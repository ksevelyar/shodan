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
    in {
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
