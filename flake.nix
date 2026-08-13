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
          # 安装 bash/zsh/fish 补全到标准目录（completions/ 手写脚本）
          # 用 $src 引用源（cargoInstallHook 后 cwd 不在 build 顶层，相对路径找不到）
          postInstall = ''
            install -Dm644 $src/completions/shell-switcher.bash $out/share/bash-completion/completions/shell-switcher
            install -Dm644 $src/completions/_shell-switcher $out/share/zsh/site-functions/_shell-switcher
            install -Dm644 $src/completions/shell-switcher.fish $out/share/fish/vendor_completions.d/shell-switcher.fish
          '';
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
