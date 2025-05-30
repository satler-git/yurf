{
  description = "A very basic flake";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";

    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";

    crane.url = "github:ipetkov/crane";

    flake-parts.url = "github:hercules-ci/flake-parts";
  };

  nixConfig = {
    extra-substituters = [
      "https://satler.cachix.org"
    ];

    extra-trusted-public-keys = [
      "satler.cachix.org-1:qPh5WfP6bZGq/dGK4DsoYRfZqbDAxPwO9l8v+x+ET04="
    ];
  };

  outputs =
    inputs@{
      crane,
      flake-parts,
      self,
      ...
    }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = [
        "x86_64-linux"
      ];

      flake = {
        homeManagerModules = rec {
          default = yurf;

          yurf = import ./modules/home-manager/default.nix self;
        };
      };

      perSystem =
        {
          system,
          pkgs,
          self',
          ...
        }:
        let
          rust-bin = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

          craneLib = (crane.mkLib pkgs).overrideToolchain rust-bin;
          src = craneLib.cleanCargoSource ./.;

          commonArgs = {
            inherit src;
            strictDeps = true;

            nativeBuildInputs = with pkgs; [
              pkg-config

              m4
              gmp
              mpfr
              libmpc
              gmp.dev
            ];
          };

          cargoArtifacts = craneLib.buildDepsOnly commonArgs;

          yurf = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );
        in
        {

          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              inputs.rust-overlay.overlays.default
            ];
          };

          packages = {
            default = yurf;
          };

          devShells.default = pkgs.mkShell {
            inputsFrom = [
              self'.packages.default
            ];

            name = "ltrait";

            buildInputs = with pkgs; [
              rust-bin

              cargo-nextest
            ];
          };
        };
    };
}
