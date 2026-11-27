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

      blenderMcp = pkgs.python3Packages.buildPythonPackage {
        pname = "mcp-for-blender";
        version = "2.3.3";
        pyproject = true;
        src = pkgs.fetchPypi {
          pname = "mcp_for_blender";
          version = "2.3.3";
          hash = "sha256-CUyq3roNmk0XC5ON37M3t0rjnKDdKq24K/tvuTyL7Cc=";
        };
        build-system = [ pkgs.python3Packages.setuptools ];
        dependencies = [ pkgs.python3Packages.mcp_1 pkgs.python3Packages.httpx ];
        pythonImportsCheck = [ "blender_mcp.server" ];
      };

      blenderMcpServer = pkgs.writeShellScriptBin "blender-mcp-server" ''
        exec ${blenderMcp}/bin/mcp-for-blender "$@"
      '';

      blenderWithMcpPlugin = pkgs.writeShellScriptBin "blender-with-mcp-plugin" ''
        exec ${pkgs.blender}/bin/blender --factory-startup --python ${pkgs.writeText "enable-blender-mcp.py" ''
          import importlib.util

          specification = importlib.util.spec_from_file_location("blender_mcp_addon", "${blenderMcp}/${pkgs.python3.sitePackages}/blender_mcp/bundled/addon.py")
          addon = importlib.util.module_from_spec(specification)
          specification.loader.exec_module(addon)
          addon.register()
        ''} "$@"
      '';

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
          blenderMcpServer
          blenderWithMcpPlugin
          pkgs.python3
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
