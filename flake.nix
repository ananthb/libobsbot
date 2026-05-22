{
  description = "libobsbot - cross-platform Rust SDK for OBSBOT cameras";

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

        # MSRV pinned to the value in workspace.package.rust-version. Bumped
        # above 1.75 when a transitive dep (getrandom-0.4 → tempfile →
        # cbindgen) started requiring edition 2024.
        rustMsrv = pkgs.rust-bin.stable."1.85.0".default;
        rustPlatformMsrv = pkgs.makeRustPlatform {
          rustc = rustMsrv;
          cargo = rustMsrv;
        };

        # Nightly + miri + rust-src for the `miri` devShell. Not used in
        # `checks` - building the miri sysroot inside nix's offline sandbox
        # would need the rust-lang std deps (rustc-demangle, etc.) vendored
        # alongside ours, which we don't pull in for normal builds. Run
        # miri locally instead: `nix develop .#miri -c cargo miri test
        # --workspace -- --skip transport::`.
        rustNightlyMiri = pkgs.rust-bin.selectLatestNightlyWith (toolchain:
          toolchain.default.override {
            extensions = [ "miri-preview" "rust-src" ];
          });

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
            description = "Cross-platform Rust SDK for OBSBOT cameras";
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

        devShells = {
          default = pkgs.mkShell {
            inherit nativeBuildInputs;
            buildInputs = buildInputs ++ (with pkgs; [
              cargo-edit
              cargo-outdated
              cargo-audit
              cargo-deny
              rust-cbindgen
              valgrind
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

          # `nix develop .#miri` - nightly toolchain with miri + rust-src
          # pre-installed, ready to `cargo miri test`. Network-on, so the
          # sysroot can build the first time.
          miri = pkgs.mkShell {
            nativeBuildInputs = [ pkgs.pkg-config rustNightlyMiri ];
            inherit buildInputs;
            shellHook = ''
              echo "libobsbot miri shell"
              echo "  $(cargo --version)"
              echo "  $(rustc --version)"
              echo "  run: cargo miri test --workspace -- --skip transport::"
            '';
          };
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

          # cargo-deny: license + bans + sources. The advisories check needs
          # the RustSec advisory-db git tree fetched at evaluation time and
          # is left to a separate workflow / `cargo deny check advisories`
          # invocation in the devShell.
          cargo-deny = pkgs.rustPlatform.buildRustPackage (cargoArtifacts // {
            pname = "libobsbot-cargo-deny";
            version = "0.0.0";
            nativeBuildInputs = nativeBuildInputs ++ [ pkgs.cargo-deny ];
            buildPhase = ''
              cargo deny --offline check licenses bans sources
            '';
            installPhase = ''
              mkdir -p $out
            '';
            doCheck = false;
          });

          # Rustdoc: refuse broken intra-doc links so public API docs don't
          # rot silently.
          rustdoc-links = pkgs.rustPlatform.buildRustPackage (cargoArtifacts // {
            pname = "libobsbot-rustdoc-links";
            version = "0.0.0";
            RUSTDOCFLAGS = "-D rustdoc::broken_intra_doc_links -D rustdoc::private_intra_doc_links";
            buildPhase = ''
              cargo doc --workspace --no-deps
            '';
            installPhase = ''
              mkdir -p $out
            '';
            doCheck = false;
          });

          # MSRV: verify the workspace builds against the rust-version
          # claimed in `workspace.package.rust-version` (currently 1.85).
          msrv = rustPlatformMsrv.buildRustPackage (cargoArtifacts // {
            pname = "libobsbot-msrv";
            version = "0.0.0";
            nativeBuildInputs = [ pkgs.pkg-config rustMsrv ];
            buildPhase = ''
              cargo check --workspace --all-targets
            '';
            installPhase = ''
              mkdir -p $out
            '';
            doCheck = false;
          });

          # Valgrind over the C smoke - the FFI boundary is exactly where
          # C-side leaks / UAFs would hide.
          valgrind-c-smoke = pkgs.runCommand "libobsbot-valgrind-c-smoke" {
            nativeBuildInputs = with pkgs; [ gcc valgrind ];
          } ''
            cp ${./crates/libobsbot-ffi/examples/c_smoke.c} c_smoke.c
            gcc -O2 -Wall -Wextra \
              -I ${libobsbot}/include \
              -L ${libobsbot}/lib \
              -Wl,-rpath,${libobsbot}/lib \
              c_smoke.c -lobsbot -o c_smoke
            valgrind \
              --error-exitcode=1 \
              --leak-check=full \
              --errors-for-leak-kinds=definite \
              --show-leak-kinds=definite,possible \
              ./c_smoke
            touch $out
          '';
        };

        formatter = pkgs.nixpkgs-fmt;
      });
}
