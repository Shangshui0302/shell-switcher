{
  description = "shell-switcher — runtime desktop shell switcher for Hyprland/niri";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        rustPkgs = pkgs.rustPlatform;
      in {
        packages.default = rustPkgs.buildRustPackage {
          pname = "shell-switcher";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;
          nativeBuildInputs = [ pkgs.pkg-config ];
        };

        devShells.default = pkgs.mkShell {
          packages = [
            pkgs.rustc
            pkgs.cargo
            pkgs.rust-analyzer
            pkgs.clippy
          ];
        };
      });
}
