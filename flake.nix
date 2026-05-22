{
  description = "libobsbot - clean-room, cross-platform Rust SDK for OBSBOT cameras";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };

        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        platformInputs = with pkgs;
          lib.optionals stdenv.isLinux [ libusb1 udev ];

        nativeBuildInputs = with pkgs; [ pkg-config rustToolchain ];
        buildInputs = platformInputs;

        cargoArtifacts = {
          inherit nativeBuildInputs buildInputs;
          src = pkgs.lib.cleanSourceWith {
            src = ./.;
            filter = path: type:
              let base = baseNameOf path; in
              !(base == "target" || base == ".direnv" || base == ".devenv");
          };
          cargoLock.lockFile = ./Cargo.lock;
        };

        # The main package - lib, cdylib, staticlib, generated header.
        libobsbot = pkgs.rustPlatform.buildRustPackage (cargoArtifacts // {
          pname = "libobsbot";
          version = "0.0.0";

          # cbindgen runs at build time when LIBOBSBOT_GEN_HEADER is set.
          LIBOBSBOT_GEN_HEADER = "1";

          # buildRustPackage installs liblibobsbot.{so,dylib,a} by default.
          # Strip the redundant prefix and add the generated header.
          postInstall = ''
            mkdir -p $out/include
            cp include/libobsbot.h $out/include/
            for f in $out/lib/liblibobsbot.*; do
              [ -f "$f" ] || continue
              mv "$f" "$out/lib/$(basename "$f" | sed 's/^liblib/lib/')"
            done
          '';

          doCheck = true;

          meta = with pkgs.lib; {
            description = "Clean-room, cross-platform Rust SDK for OBSBOT cameras";
            homepage = "https://github.com/ananthb/libobsbot";
            license = licenses.gpl3Only;
            platforms = platforms.unix;
            maintainers = [ ];
          };
        });
      in
      {
        packages = {
          default = libobsbot;
          libobsbot = libobsbot;
        };

        devShells.default = pkgs.mkShell {
          inherit nativeBuildInputs;
          buildInputs = buildInputs ++ (with pkgs; [
            cargo-edit
            cargo-outdated
            cargo-audit
            rust-cbindgen
          ] ++ lib.optionals stdenv.isLinux [
            wireshark
            usbutils
          ]);

          # Make rust-analyzer happy.
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";

          shellHook = ''
            echo "libobsbot devshell"
            echo "  $(cargo --version)"
            echo "  $(rustc --version)"
          '';
        };

        checks = {
          # Full package build, including tests via `doCheck = true`.
          build = libobsbot;

          # Format check.
          fmt = pkgs.runCommand "libobsbot-fmt" {
            inherit nativeBuildInputs;
            src = cargoArtifacts.src;
          } ''
            cp -r $src/* .
            chmod -R u+w .
            cargo fmt --all -- --check
            touch $out
          '';

          # Clippy lints with -D warnings.
          clippy = pkgs.rustPlatform.buildRustPackage (cargoArtifacts // {
            pname = "libobsbot-clippy";
            version = "0.0.0";
            buildPhase = ''
              cargo clippy --workspace --all-targets -- -D warnings
            '';
            installPhase = ''
              mkdir -p $out
            '';
            doCheck = false;
          });

          # Header-drift: regenerate libobsbot.h and assert it matches the
          # committed copy.
          header-drift = pkgs.rustPlatform.buildRustPackage (cargoArtifacts // {
            pname = "libobsbot-header-drift";
            version = "0.0.0";
            LIBOBSBOT_GEN_HEADER = "1";
            buildPhase = ''
              cargo build --release -p libobsbot-ffi
            '';
            installPhase = ''
              diff -u ${./include/libobsbot.h} include/libobsbot.h
              mkdir -p $out
            '';
            doCheck = false;
          });
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
