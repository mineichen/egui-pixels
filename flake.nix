{
  description = "Deterministic Rust + WASM + Tailwind dev shell";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
  };

  outputs = { self, nixpkgs, flake-utils, fenix, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
        };
        # ort crate 1.15.2 requires onnxruntime 1.16.0 specifically
        # Download prebuilt ONNX Runtime 1.16.0
        onnxruntime_1_16 = pkgs.stdenv.mkDerivation {
          name = "onnxruntime-1.16.0";
          src = pkgs.fetchurl {
            url = "https://github.com/microsoft/onnxruntime/releases/download/v1.16.0/onnxruntime-linux-x64-1.16.0.tgz";
            sha256 = "sha256-I9hn6yp3jdVMYBd45dK89FzrdsWoLMAUQFPIP10fAAU=";
          };
          nativeBuildInputs = [ pkgs.gnutar pkgs.gzip ];
          installPhase = ''
            mkdir -p $out/lib
            tar -xzf $src
            cp -r onnxruntime-linux-x64-1.16.0/lib/* $out/lib/
            mkdir -p $out/include
            cp -r onnxruntime-linux-x64-1.16.0/include/* $out/include/ 2>/dev/null || true
          '';
        };
        rust = with fenix.packages.${system}; combine [
          stable.toolchain
          targets.wasm32-unknown-unknown.stable.rust-std
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = [
            rust
            pkgs.trunk
            pkgs.bashInteractive
            pkgs.bash-completion
            pkgs.wasm-pack
            pkgs.wayland
            pkgs.mesa
          ];

          shellHook = ''
            echo "===================================="
            echo " Welcome to the deterministic dev shell! "
            echo "===================================="
            rustc --version
            cargo --version
            trunk --version
            
            export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
              pkgs.wayland
              pkgs.libxkbcommon
            ]}:$LD_LIBRARY_PATH
          '';
        };

        apps.default = {
          type = "app";
          program = "${pkgs.writeShellScriptBin "cursor" ''
            export DISPLAY="''${DISPLAY:-:0}"
            export WAYLAND_DISPLAY="''${WAYLAND_DISPLAY:-wayland-0}"
            export XDG_SESSION_TYPE="''${XDG_SESSION_TYPE:-wayland}"
            export XDG_CURRENT_DESKTOP="''${XDG_CURRENT_DESKTOP:-KDE}"
            exec nix develop . --command ${pkgs.lib.getExe pkgs.code-cursor} --no-sandbox "$PWD"
            
          ''}/bin/cursor";
        };

        apps.outdated = {
          type = "app";
          program = "${pkgs.writeShellScriptBin "outdated" ''
            exec ${pkgs.cargo-outdated}/bin/cargo-outdated outdated
          ''}/bin/outdated";
        };


        apps.annotationtool-web = {
          type = "app";
          program = "${pkgs.writeShellScriptBin "annotation-tool-web" ''
            set -e
            PROJECT_ROOT="$PWD"
            
            if [ ! -f "$PROJECT_ROOT/flake.nix" ]; then
              echo "Error: Not in flake root directory" >&2
              exit 1
            fi
            
            exec nix develop "$PROJECT_ROOT" --command bash -c "
              cd '$PROJECT_ROOT/annotation-tool'
              exec ${pkgs.trunk}/bin/trunk serve --release --no-default-features
            "
          ''}/bin/annotation-tool-web";
        };

        apps.annotationtool = {
          type = "app";
          program = "${pkgs.writeShellScriptBin "annotation-tool-app" ''
            set -e
            PROJECT_ROOT="$PWD"
            
            if [ ! -f "$PROJECT_ROOT/flake.nix" ]; then
              echo "Error: Not in flake root directory" >&2
              exit 1
            fi
            
            # Build if needed
            nix develop "$PROJECT_ROOT" --command bash -c "cargo build --release --features sam --bin annotation-tool-app"
            
            # Run with arguments passed through
            exec nix develop "$PROJECT_ROOT" --command bash -c "
              export LD_LIBRARY_PATH=${pkgs.lib.makeLibraryPath [
                pkgs.libGL
                pkgs.mesa
                onnxruntime_1_16
                pkgs.stdenv.cc.cc.lib
              ]}:\$LD_LIBRARY_PATH
              if [ \$# -eq 0 ]; then
                exec \"$PROJECT_ROOT/target/release/annotation-tool-app\" ~/Downloads
              else
                exec \"$PROJECT_ROOT/target/release/annotation-tool-app\" \"\$@\"
              fi
            " _ "$@"
          ''}/bin/annotation-tool-app";
        };
      });
}
