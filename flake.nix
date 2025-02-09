{
  description = "The Plaixt project";
  inputs = {
    nixpkgs.url = "nixpkgs/nixos-24.11";
    flake-utils = {
      url = "github:numtide/flake-utils";
    };
    crane = {
      url = "github:ipetkov/crane";
    };
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
      };
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      rust-overlay,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        rustTarget = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;
        unstableRustTarget = pkgs.rust-bin.selectLatestNightlyWith (
          toolchain:
          toolchain.default.override {
            extensions = [
              "rust-src"
              "miri"
              "rustfmt"
            ];
          }
        );
        craneLib = (crane.mkLib pkgs).overrideToolchain rustTarget;
        unstableCraneLib = (crane.mkLib pkgs).overrideToolchain unstableRustTarget;

        tomlInfo = craneLib.crateNameFromCargoToml { cargoToml = ./Cargo.toml; };
        inherit (tomlInfo) version;

        src = ./.;

        rustfmt' = pkgs.writeShellScriptBin "rustfmt" ''
          exec "${unstableRustTarget}/bin/rustfmt" "$@"
        '';

        common = {
          src = ./.;

          buildInputs = [
            pkgs.openssl
            pkgs.pkg-config
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly (
          common
          // {
            cargoExtraArgs = "--all-features --all";
          }
        );

        plaixt = craneLib.buildPackage (
          common
          // {
            inherit cargoArtifacts version;
            cargoExtraArgs = "--all-features --all";
          }
        );

      in
      rec {
        checks = {
          inherit plaixt;

          plaixt-clippy = craneLib.cargoClippy {
            inherit cargoArtifacts src;
            cargoExtraArgs = "--all --all-features";
            cargoClippyExtraArgs = "-- --deny warnings";
          };

          plaixt-fmt = unstableCraneLib.cargoFmt {
            inherit src;
          };
        };

        packages.plaixt = plaixt;
        packages.default = packages.plaixt;

        apps.plaixt = flake-utils.lib.mkApp {
          name = "plaixt";
          drv = plaixt;
        };
        apps.default = apps.plaixt;

        devShells.default = devShells.plaixt;
        devShells.plaixt = pkgs.mkShell {
          buildInputs = [ ];

          inputsFrom = [ plaixt ];

          nativeBuildInputs = [
            rustfmt'
            rustTarget

            pkgs.cargo-msrv
            pkgs.cargo-deny
            pkgs.cargo-expand
            pkgs.cargo-bloat
            pkgs.cargo-fuzz

            pkgs.gitlint
          ];
        };
      }
    );
}
