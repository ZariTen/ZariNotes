//! Pasted images, saved next to the note and shown in live preview.
//!
//! iced's clipboard only carries text, so an image paste is read with
//! `wl-paste` (Wayland) or `xclip` (X11). The bytes go in a hidden `.images`
//! folder beside the note. The preview draws every local image at one size.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

/// Display size of a rendered image. Not the file's pixel size.
pub const IMAGE_WIDTH: f32 = 480.0;
pub const IMAGE_HEIGHT: f32 = 320.0;

const DIR_NAME: &str = ".images";

const IMAGE_TYPES: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/bmp", "bmp"),
];

/// Bytes copied from the clipboard, or read from a copied image file.
#[derive(Debug, Clone)]
pub struct ImageBytes {
    pub bytes: Vec<u8>,
    pub extension: String,
}

/// What a paste found on the clipboard, before it is written next to a note.
#[derive(Debug, Clone)]
pub enum ClipboardOffer {
    Image(ImageBytes),
    /// No image. The caller should paste text instead.
    None,
    /// The clipboard tool for this session is not installed.
    ToolMissing,
}

/// Read an image from the system clipboard. Blocks for at most a couple of seconds.
pub fn read_clipboard() -> ClipboardOffer {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return read_with(wayland_image);
    }
    read_with(x11_image)
}

fn read_with(read: fn() -> Read) -> ClipboardOffer {
    match read() {
        Read::Image(image) => ClipboardOffer::Image(image),
        Read::None => ClipboardOffer::None,
        Read::Missing => ClipboardOffer::ToolMissing,
    }
}

enum Read {
    Image(ImageBytes),
    None,
    Missing,
}

fn wayland_image() -> Read {
    let types = match run("wl-paste", &["--list-types"]) {
        Run::Output(text) => text,
        Run::Missing => return Read::Missing,
        Run::Failed => return Read::None,
    };
    if let Some((mime, ext)) = offered_image_type(&types) {
        return bytes_from("wl-paste", &["--type", mime, "--no-newline"], ext);
    }
    if mime_listed(&types, "text/uri-list") {
        return file_from_uri_list("wl-paste", &["--type", "text/uri-list", "--no-newline"]);
    }
    Read::None
}

fn x11_image() -> Read {
    let types = match run(
        "xclip",
        &["-selection", "clipboard", "-target", "TARGETS", "-out"],
    ) {
        Run::Output(text) => text,
        Run::Missing => return Read::Missing,
        Run::Failed => return Read::None,
    };
    if let Some((mime, ext)) = offered_image_type(&types) {
        return bytes_from(
            "xclip",
            &["-selection", "clipboard", "-target", mime, "-out"],
            ext,
        );
    }
    if mime_listed(&types, "text/uri-list") {
        return file_from_uri_list(
            "xclip",
            &[
                "-selection",
                "clipboard",
                "-target",
                "text/uri-list",
                "-out",
            ],
        );
    }
    Read::None
}

fn bytes_from(program: &str, args: &[&str], extension: &str) -> Read {
    match run_bytes(program, args) {
        RunBytes::Bytes(bytes) if !bytes.is_empty() => Read::Image(ImageBytes {
            bytes,
            extension: extension.to_owned(),
        }),
        RunBytes::Missing => Read::Missing,
        _ => Read::None,
    }
}

fn file_from_uri_list(program: &str, args: &[&str]) -> Read {
    let text = match run(program, args) {
        Run::Output(text) => text,
        Run::Missing => return Read::Missing,
        Run::Failed => return Read::None,
    };
    let Some(path) = first_local_image(&text) else {
        return Read::None;
    };
    match std::fs::read(&path) {
        Ok(bytes) if !bytes.is_empty() => Read::Image(ImageBytes {
            bytes,
            extension: extension_of(&path).unwrap_or("png").to_owned(),
        }),
        _ => Read::None,
    }
}

/// Write `image` into `note_dir/.images` and return the file name.
pub fn save_image(note_dir: &Path, image: &ImageBytes) -> io::Result<String> {
    let ext = safe_extension(&image.extension)?;
    let dir = note_dir.join(DIR_NAME);
    std::fs::create_dir_all(&dir)?;
    let name = unique_name(&dir, ext)?;
    std::fs::write(dir.join(&name), &image.bytes)?;
    Ok(name)
}

fn safe_extension(ext: &str) -> io::Result<&str> {
    if IMAGE_TYPES.iter().any(|(_, known)| *known == ext) || ext == "jpeg" {
        return Ok(if ext == "jpeg" { "jpg" } else { ext });
    }
    Err(io::Error::new(
        ErrorKind::InvalidInput,
        "clipboard image has an unsupported type",
    ))
}

fn unique_name(dir: &Path, ext: &str) -> io::Result<String> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    for n in 0..1000 {
        let name = if n == 0 {
            format!("paste-{stamp}.{ext}")
        } else {
            format!("paste-{stamp}-{n}.{ext}")
        };
        if !dir.join(&name).exists() {
            return Ok(name);
        }
    }
    Err(io::Error::other("could not pick a free image name"))
}

/// Markdown inserted at the cursor. A trailing newline leaves the image line
/// so live preview renders it instead of showing the raw link.
pub fn paste_snippet(column: usize, filename: &str) -> String {
    let image = format!("![]({DIR_NAME}/{filename})");
    if column == 0 {
        format!("{image}\n")
    } else {
        format!("\n{image}\n")
    }
}

/// Local file for a markdown image URL, relative to the note's folder.
pub fn image_path(note_dir: &Path, url: &str) -> Option<PathBuf> {
    if url.contains("://") || url.starts_with("mailto:") {
        return None;
    }
    let decoded = percent_decode(url);
    let path = decoded.split(['?', '#']).next().unwrap_or("");
    if path.is_empty() {
        return None;
    }
    let file = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        note_dir.join(path)
    };
    if !is_image_ext(&file) || !file.is_file() {
        return None;
    }
    Some(file)
}

/// A clipboard string that is one local image file (`file://` or an absolute path).
pub fn copied_image_file(text: &str) -> Option<PathBuf> {
    let text = text.trim();
    if text.lines().count() != 1 {
        return None;
    }
    let path = file_uri(text)?;
    if is_image_ext(&path) && path.is_file() {
        return Some(path);
    }
    None
}

fn first_local_image(uri_list: &str) -> Option<PathBuf> {
    for line in uri_list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let path = file_uri(line)?;
        if !is_image_ext(&path) {
            return None;
        }
        if path.is_file() {
            return Some(path);
        }
        return None;
    }
    None
}

fn file_uri(text: &str) -> Option<PathBuf> {
    if let Some(rest) = text.strip_prefix("file://") {
        let path = percent_decode(rest);
        if path.starts_with('/') {
            return Some(PathBuf::from(path));
        }
        return None;
    }
    let path = Path::new(text);
    if path.is_absolute() {
        return Some(path.to_path_buf());
    }
    None
}

fn offered_image_type(types: &str) -> Option<(&'static str, &'static str)> {
    IMAGE_TYPES
        .iter()
        .copied()
        .find(|(mime, _)| mime_listed(types, mime))
}

fn mime_listed(types: &str, mime: &str) -> bool {
    types.lines().any(|line| {
        line.split(';')
            .next()
            .unwrap_or(line)
            .trim()
            .eq_ignore_ascii_case(mime)
    })
}

fn is_image_ext(path: &Path) -> bool {
    extension_of(path).is_some_and(|ext| {
        ext == "jpg" || ext == "jpeg" || IMAGE_TYPES.iter().any(|(_, known)| *known == ext)
    })
}

fn extension_of(path: &Path) -> Option<&str> {
    path.extension().and_then(|ext| ext.to_str())
}

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((hi << 4) | lo);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

enum Run {
    Output(String),
    Missing,
    Failed,
}

enum RunBytes {
    Bytes(Vec<u8>),
    Missing,
    Failed,
}

fn run(program: &str, args: &[&str]) -> Run {
    match run_bytes(program, args) {
        RunBytes::Bytes(bytes) => Run::Output(String::from_utf8_lossy(&bytes).into_owned()),
        RunBytes::Missing => Run::Missing,
        RunBytes::Failed => Run::Failed,
    }
}

fn run_bytes(program: &str, args: &[&str]) -> RunBytes {
    let mut command = Command::new("timeout");
    command
        .args(["--signal=KILL", "2", program])
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    match command.output() {
        Ok(output) => classify(output.status, output.stdout),
        Err(error) if error.kind() == ErrorKind::NotFound => run_bytes_direct(program, args),
        Err(_) => RunBytes::Failed,
    }
}

fn run_bytes_direct(program: &str, args: &[&str]) -> RunBytes {
    match Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        Ok(output) => classify(output.status, output.stdout),
        Err(error) if error.kind() == ErrorKind::NotFound => RunBytes::Missing,
        Err(_) => RunBytes::Failed,
    }
}

/// `timeout` exits 127 when the program is not installed.
fn classify(status: std::process::ExitStatus, stdout: Vec<u8>) -> RunBytes {
    if status.success() {
        return RunBytes::Bytes(stdout);
    }
    if status.code() == Some(127) {
        return RunBytes::Missing;
    }
    RunBytes::Failed
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use iced::widget::markdown;

    use super::{
        ImageBytes, classify, copied_image_file, image_path, offered_image_type, paste_snippet,
        save_image,
    };

    #[test]
    fn paste_snippet_is_its_own_line() {
        assert_eq!(
            paste_snippet(0, "paste-1.png"),
            "![](.images/paste-1.png)\n"
        );
        assert_eq!(
            paste_snippet(4, "paste-1.png"),
            "\n![](.images/paste-1.png)\n"
        );
    }

    #[test]
    fn offered_image_type_prefers_png() {
        let types = "text/plain\nimage/jpeg\nimage/png\n";
        assert_eq!(offered_image_type(types), Some(("image/png", "png")));
        assert_eq!(offered_image_type("text/plain\n"), None);
    }

    #[test]
    fn classify_treats_127_as_a_missing_tool() {
        let ok = std::process::Command::new("true").status().unwrap();
        let missing = std::process::Command::new("sh")
            .args(["-c", "exit 127"])
            .status()
            .unwrap();
        let failed = std::process::Command::new("false").status().unwrap();
        assert!(
            matches!(classify(ok, b"png".to_vec()), super::RunBytes::Bytes(bytes) if bytes == b"png")
        );
        assert!(matches!(
            classify(missing, Vec::new()),
            super::RunBytes::Missing
        ));
        assert!(matches!(
            classify(failed, Vec::new()),
            super::RunBytes::Failed
        ));
    }

    #[test]
    fn image_path_resolves_next_to_the_note_and_skips_remote_urls() {
        let dir = temp_dir("resolve");
        let file = dir.join(".images/shot.png");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, b"png").unwrap();

        assert_eq!(image_path(&dir, ".images/shot.png"), Some(file.clone()));
        assert_eq!(image_path(&dir, ".images/my%20shot.png"), None);
        std::fs::write(dir.join(".images/my shot.png"), b"png").unwrap();
        assert_eq!(
            image_path(&dir, ".images/my%20shot.png").as_deref(),
            Some(dir.join(".images/my shot.png").as_path())
        );
        assert_eq!(image_path(&dir, "https://example.com/a.png"), None);
        assert_eq!(image_path(&dir, ".images/missing.png"), None);
        assert_eq!(image_path(&dir, "notes.md"), None);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_image_writes_into_dot_images_without_overwriting() {
        let dir = temp_dir("save");
        std::fs::create_dir_all(&dir).unwrap();
        let first = save_image(
            &dir,
            &ImageBytes {
                bytes: b"one".to_vec(),
                extension: "png".into(),
            },
        )
        .unwrap();
        let second = save_image(
            &dir,
            &ImageBytes {
                bytes: b"two".to_vec(),
                extension: "png".into(),
            },
        )
        .unwrap();
        assert_ne!(first, second);
        let folder = dir.join(".images");
        assert_eq!(std::fs::read(folder.join(&first)).unwrap(), b"one");
        assert_eq!(std::fs::read(folder.join(&second)).unwrap(), b"two");
        assert!(
            save_image(
                &dir,
                &ImageBytes {
                    bytes: b"no".to_vec(),
                    extension: "exe".into(),
                },
            )
            .is_err()
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn copied_image_file_accepts_one_local_image() {
        let dir = temp_dir("uri");
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("shot.png");
        std::fs::write(&file, b"png").unwrap();
        let uri = format!("file://{}", file.display());
        assert_eq!(copied_image_file(&uri), Some(file.clone()));
        assert_eq!(copied_image_file(&file.display().to_string()), Some(file));
        assert_eq!(copied_image_file("hello"), None);
        assert_eq!(
            copied_image_file("file:///tmp/nope.png\nfile:///tmp/b.png"),
            None
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_lone_image_line_parses_as_an_image() {
        let items: Vec<_> = markdown::parse("![](.images/paste-1.png)").collect();
        assert!(matches!(
            items.as_slice(),
            [markdown::Item::Image { url, .. }] if url == ".images/paste-1.png"
        ));
    }

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("zarinotes-images-{name}-{}", std::process::id()))
    }
}
