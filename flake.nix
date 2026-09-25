{
  description = "ZariNotes - a minimal Markdown notes app (Rust + iced)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAll = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});

      # winit/wgpu dlopen these; linking them is not enough.
      runtimeLibs =
        pkgs: with pkgs; [
          wayland
          libxkbcommon
          vulkan-loader
          libGL
          libx11
          libxcursor
          libxi
          libxrandr
          libxrender
          libxinerama
          libxcb
          libxext
        ];

      package =
        pkgs:
        let
          inherit (pkgs) lib rustPlatform makeWrapper;
          libs = runtimeLibs pkgs;
          cargo = builtins.fromTOML (builtins.readFile ./Cargo.toml);
        in
        rustPlatform.buildRustPackage {
          pname = cargo.package.name;
          version = cargo.package.version;

          src = lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./src
              ./assets
            ];
          };

          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = [ makeWrapper ];

          # Same install as ./install.sh. Nix only adds the runtime-library wrapper.
          postInstall = ''
            wrapProgram $out/bin/zarinotes \
              --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath libs}
            SKIP_BUILD=1 PREFIX=$out ROOT=$src sh ${./install.sh}
          '';

          doInstallCheck = true;
          installCheckPhase = ''
            runHook preInstallCheck
            test -x $out/bin/zarinotes
            cmp $src/assets/zarinotes.desktop $out/share/applications/zarinotes.desktop
            cmp $src/assets/icon.svg $out/share/icons/hicolor/scalable/apps/zarinotes.svg
            runHook postInstallCheck
          '';

          meta = {
            description = "A minimal Markdown notes app";
            homepage = "https://github.com/ZariTen/ZariNotes";
            license = lib.licenses.gpl3Plus;
            mainProgram = "zarinotes";
            platforms = lib.platforms.linux;
          };
        };
    in
    {
      packages = forAll (pkgs: rec {
        zarinotes = package pkgs;
        default = zarinotes;
      });

      apps = forAll (pkgs: {
        default = {
          type = "app";
          program = "${self.packages.${pkgs.stdenv.hostPlatform.system}.default}/bin/zarinotes";
        };
      });

      overlays.default = final: _prev: {
        zarinotes = self.packages.${final.stdenv.hostPlatform.system}.default;
      };

      devShells = forAll (pkgs: {
        default = pkgs.mkShell {
          packages = with pkgs; [
            cargo
            rustc
            rustfmt
            clippy
            rust-analyzer
            pkg-config
          ];
          buildInputs = runtimeLibs pkgs;
          LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (runtimeLibs pkgs);
        };
      });
    };
}
