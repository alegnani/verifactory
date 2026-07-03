{
  description = "A verifier for Factorio blueprints";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs?ref=nixos-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    crane.url = "github:ipetkov/crane";
    utils.url = "github:numtide/flake-utils";
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      utils,
      advisory-db,
    }:
    utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };
        inherit (pkgs) lib;

        # =========================================
        # Rust crane build system for nix
        # =========================================
        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;
        # Crane: common arguments for building
        commonArgs = {
          inherit src;
          strictDeps = true;

          buildInputs = with pkgs; [
            z3
            llvmPackages.libclang
            wayland
            libGL
            libxkbcommon
          ];

          nativeBuildInputs = with pkgs; [
            pkg-config
          ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };

        # Build the dependencies
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        fileSetForCrate =
          crate:
          lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              (craneLib.fileset.commonCargoSources ./verifactory_lib)
              (craneLib.fileset.commonCargoSources crate)
              (crate + "/imgs")
            ];
          };
        verifactory_app = craneLib.buildPackage (
          commonArgs
          // rec {
            inherit cargoArtifacts;
            inherit (craneLib.crateNameFromCargoToml { cargoToml = ./verifactory_app/Cargo.toml; })
              pname
              version
              ;
            cargoExtraArgs = "-p verifactory_app";
            src = fileSetForCrate ./verifactory_app;

            nativeBuildInputs = with pkgs; [
              pkg-config
              makeWrapper
            ];

            dlopenDeps = with pkgs; [
              wayland
              libGL
              libxkbcommon
            ];

            postFixup = ''
              wrapProgram $out/bin/verifactory_app \
                --prefix LD_LIBRARY_PATH : ${LD_LIBRARY_PATH}
            '';
            LD_LIBRARY_PATH = "${lib.makeLibraryPath dlopenDeps}";

            # Don't run tests normally: cargo-nextest runs them
            doCheck = false;
          }
        );
      in
      rec {
        packages = {
          inherit verifactory_app;
        };

        checks = {
          clippy = craneLib.cargoClippy (
            commonArgs
            // {
              inherit cargoArtifacts;
              cargoClippyExtraArgs = "--all-targets -- --deny warnings";
            }
          );

          doc = craneLib.cargoDoc (
            commonArgs
            // {
              inherit cargoArtifacts;
              env.RUSTDOCFLAGS = "--deny warnings";
            }
          );

          # Check formatting
          fmt = craneLib.cargoFmt {
            inherit src;
          };

          toml-fmt = craneLib.taploFmt {
            src = pkgs.lib.sources.sourceFilesBySuffices src [ ".toml" ];
          };

          # Audit dependencies
          audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };

          # Audit licenses
          deny = craneLib.cargoDeny {
            inherit src;
          };

          # Run tests with cargo-nextest
          nextest = craneLib.cargoNextest (
            commonArgs
            // {
              inherit cargoArtifacts;
              partitions = 1;
              partitionType = "count";
              cargoNextestPartitionsExtraArgs = "--no-tests=pass";
            }
          );
        };

        defaultPackage = packages.verifactory_app;

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };
      }
    );
}
