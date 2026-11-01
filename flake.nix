{
  description = "Build tool for Leptos (Rust)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib;

        craneLib = (crane.mkLib pkgs).overrideToolchain (
          p:
          p.rust-bin.nightly.latest.default.override {
            extensions = [
              "rust-src"
              "rust-analyzer"
            ];
            targets = [ "wasm32-unknown-unknown" ];
          }
        );

        cargo-leptos = craneLib.buildPackage (
          {
            src = lib.cleanSourceWith {
              src = craneLib.cleanCargoSource ./.;
              filter = path: type: !(lib.hasInfix "/examples/" path || lib.hasSuffix "/examples" path);
              name = "src";
            };
            strictDeps = true;
            doCheck = false;
            OPENSSL_NO_VENDOR = 1;
            nativeBuildInputs = [
              pkgs.perl
              pkgs.pkg-config
            ];
            buildInputs = [
              pkgs.openssl
            ];
            cargoExtraArgs = "--features no_downloads";
          }
          // lib.optionalAttrs pkgs.stdenv.hostPlatform.isDarwin {
            CRATE_CC_NO_DEFAULTS = 1;
          }
        );
      in
      {
        packages.default = cargo-leptos;

        devShells.default = craneLib.devShell {
          inputsFrom = [ cargo-leptos ];

          packages = with pkgs; [
            cargo-insta
            llvmPackages_latest.llvm
            llvmPackages_latest.bintools
            zlib.out
            llvmPackages_latest.lld
            eza
            fd
            ripgrep
          ];

          shellHook = ''
            alias ls=eza
            alias find=fd
            alias grep=ripgrep
          '';
        };
      }
    );
}
