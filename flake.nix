{
  description = "Rust overlay";

  inputs = {
    nixpkgs.url      = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url  = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, rust-overlay, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [ "rust-src" "cargo" "rustc"];
        }; 
      in
      with pkgs;
      {
        devShells.default = mkShell rec {
          buildInputs = [
            cmake
            rust
            z3
            wayland
            libGL
            libxkbcommon
          ];

          LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
          # LIBCLANG_PATH = pkgs.lib.makeLibraryPath [ pkgs.llvmPackages_latest.libclang.lib ];
          nativeBuildInputs = [ rustPlatform.bindgenHook ];

          RUST_SRC_PATH = "${rust}/lib/rustlib/src/rust/library";
        };
      }
    );
}

