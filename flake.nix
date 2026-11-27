{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
    rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
  };

  outputs = {
    nixpkgs,
    flake-utils,
    rust-overlay,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [(import rust-overlay)];
      };

      cargoToml = builtins.fromTOML (builtins.readFile ./Cargo.toml);

      package = pkgs.rustPlatform.buildRustPackage {
        pname = "shodan";
        src = ./.;
        version = cargoToml.package.version;
        nativeBuildInputs = [ pkgs.pkg-config ];
        buildInputs = [ pkgs.oniguruma pkgs.aws-lc ];
        PKG_CONFIG_PATH = "${pkgs.oniguruma}/lib/pkgconfig:${pkgs.aws-lc}/lib/pkgconfig";
        doCheck = false;
        cargoLock.lockFile = ./Cargo.lock;
      };
    in {
      packages.default = package;
      apps.default = {
        type = "app";
        program = "${package}/bin/shodan";
      };

      devShells.default = pkgs.mkShell {
        packages = [
          (pkgs.rust-bin.nightly.latest.default.override {
            extensions = ["rust-src" "rust-analyzer"];
          })
          (pkgs.writeShellScriptBin "ci" ''
            set -euo pipefail
            cargo fmt --all -- --check --color always
            cargo clippy --all-features --workspace --all-targets -- -D warnings
            cargo test --all-features --workspace
          '')
        ];
      };
    });
}
