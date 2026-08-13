# shell-switcher

运行时切换桌面 shell（顶栏面板）的小工具，面向 Hyprland / niri。

多个桌面 shell 都抢占 `org.freedesktop.Notifications` DBus、都画顶栏，不能同时跑。shell-switcher 负责同一时刻只让一个 shell 运行：切走时把旧的干净停掉、再启动目标，失败自动回退默认 shell，不用手工 `systemctl --user stop/start`。

> **关于本项目**：一个 AI vibe 的小脚本——单文件 Rust、功能克制、个人自用顺手写的，不是严肃的正式工具。它存在的意义就是"够用就行"，所以代码结构很简单、取舍很直接（见「限制」）。想改就 fork，欢迎 PR。

## 特性

- **一条命令切换**：`shell-switcher set <name>`，内部 stop-all → 等待退出 → start 目标
- **声明式注册 shell**：改 `config.toml` 即可添加 shell，无需改代码、无需重编译
- **防呆**：非 Hyprland/niri 会话拒绝切换；切换失败自动回退默认 shell（config.toml 的 `default`，缺省取第一个）
- **幂等**：目标已在运行且无其他 shell 在跑时直接 no-op
- **启动恢复**：`boot` 入口读 `current` 标记，用于 compositor autostart 恢复上次选择的 shell
- **Shell 补全**：bash / zsh / fish 补全随 Nix 包自动安装，cargo/源码安装手动放置（见「安装」）
- **国际化**：中文 / 英文按系统 locale 自动切换（locale 含 `zh` → 中文，其他 → 英文）

## 工作原理

每个 shell 是一个 **systemd user service**（挂 `graphical-session.target` 上下文）。`config.toml` 声明 `name → service` 映射，切换器只做启停编排，不管理 shell 的安装。

`set <name>` 的内部流程：

1. 检测 compositor（非 Hyprland/niri 拒绝）
2. 幂等短路：目标已 active 且无其他 shell active → 只更新标记
3. `systemctl --user stop` 所有 shell，轮询确认全部 inactive（**10s 超时**）
4. `systemctl --user start` 目标，轮询进入 active（**15s 超时**）
5. 写 `current` 标记

任一步失败自动回退默认 shell。被 SIGTERM 停掉的 shell（如 DMS 退出码 143）应在其 service 里配 `SuccessExitStatus=143`，这样 systemd 不把它标为 failed。

## 安装

shell-switcher 是普通 Rust 二进制，不依赖 Nix。要求 Rust 工具链（edition 2021），依赖仅 `serde` + `toml`，任何有 systemd user session + Hyprland/niri 的发行版都能用。

### Cargo 安装（推荐，任意发行版）

```bash
cargo install --path .         # 从本地源码目录安装
# 或直接从 git 安装
cargo install --git https://github.com/Shangshui0302/shell-switcher
```

### 源码构建（通用）

```bash
git clone https://github.com/Shangshui0302/shell-switcher
cd shell-switcher
cargo build --release
install -Dm755 target/release/shell-switcher ~/.local/bin/
```

cargo 安装 / 源码构建不会自动放置 shell 补全，需要时手动复制到对应目录：

```bash
install -Dm644 completions/shell-switcher.bash ~/.local/share/bash-completion/completions/shell-switcher
install -Dm644 completions/_shell-switcher ~/.local/share/zsh/site-functions/_shell-switcher
install -Dm644 completions/shell-switcher.fish ~/.local/share/fish/vendor_completions.d/shell-switcher.fish
```

装完后：

1. 写 `~/.config/shell-switcher/config.toml`（格式见「配置」）。
2. 用发行版自己的方式声明各 shell 的 **systemd user service**（`/etc/systemd/user/` 或 `~/.config/systemd/user/`），并确保可切换的 shell 互斥（`wantedBy` 不自动起，由切换器启停）。

前提：必须运行在 Hyprland/niri 会话内，否则 `set`/`current` 会拒绝执行。

### Nix flake（NixOS / Nix 用户可选）

flake 里用 `rustPlatform.buildRustPackage` 打包，产物在 `packages.default`：

```bash
nix build .#default          # 构建出二进制（result/bin/shell-switcher）
nix develop                  # 开发环境（rustc/cargo/rust-analyzer/clippy）
```

作为 flake input 接入 NixOS/Home Manager：

```nix
# flake.nix
inputs.shell-switcher = {
  url = "github:Shangshui0302/shell-switcher";
  inputs.nixpkgs.follows = "nixpkgs";
};

# 某模块里
home.packages = [
  inputs.shell-switcher.packages.${pkgs.system}.default
];
```

## 配置

配置文件：`~/.config/shell-switcher/config.toml`。缺失或解析失败时命令会明确报错（提示配置文件路径），不静默使用内置假设。

```toml
default = "noctalia"        # 默认 shell（可选，缺省取第一个 [[shell]]）

[[shell]]
name = "noctalia"
service = "noctalia.service"

[[shell]]
name = "dms"
service = "dms.service"
```

字段：

| 字段 | 说明 |
|------|------|
| `default` | 默认 shell 名（可选）：`boot` 无 current 标记时、切换失败回退时使用；缺省取第一个 `[[shell]]` |
| `name` | 切换器里的 shell 标识（`set <name>` 用这个名字） |
| `service` | 对应的 systemd user service 名（必须含 `.service` 后缀） |

`current` 标记文件：`~/.config/shell-switcher/current`（`set` 写入、`boot` 读取，内容是当前 shell 名）。

## 使用

```bash
shell-switcher list               # 列出 config.toml 里注册的 shell
shell-switcher current            # 显示当前 active 的 shell（无则 none）
shell-switcher set <name>         # 切到指定 shell
shell-switcher boot               # 读 current 标记启动对应 shell（shell-starter 入口）
shell-switcher help               # 帮助
```

典型场景：

```bash
# 从默认 shell 切到另一个
shell-switcher set dms

# 切回
shell-switcher set noctalia

# 作为 compositor autostart / 桌面启动脚本的一部分：
# 只在标记了非默认 shell 时才额外启动它（默认 shell 由 systemd WantedBy 自动起）
shell-switcher boot
```

## 添加一个 shell（注册方式）

shell 的注册**完全声明式**，加一行 `[[shell]]` 即可，无需改代码：

1. 确保该 shell 的 systemd user service 已存在：unit 文件放 `/etc/systemd/user/`（系统级）或 `~/.config/systemd/user/`（用户级），再 `systemctl --user daemon-reload`；NixOS/Home Manager 用户也可用 `systemd.user.services` 声明。
2. 在 `config.toml` 加映射：

```toml
[[shell]]
name = "my-shell"
service = "my-shell.service"
```

3. `shell-switcher list` 确认出现，`shell-switcher set my-shell` 切换。

实践要点：

- 所有 shell 应互相**互斥**（同一时刻只能一个在跑）：可切换的 shell 的 service `wantedBy` 要置空，**不自动起**，由切换器启停；默认 shell 用 `WantedBy=graphical-session.target` 自动起。
- service 要能干净退出：配 `KillMode=control-group`（连 QML 子进程一起终止）、必要时 `SuccessExitStatus=143`（被 SIGTERM 停不标 failed）。

## 添加一个 compositor（目前需改代码）

compositor 检测目前是**硬编码**的，不支持配置。`detect_compositor()` 只认两个环境变量：

| Compositor | 环境变量 |
|------------|----------|
| Hyprland | `HYPRLAND_INSTANCE_SIGNATURE` |
| niri | `NIRI_SOCKET` |

要支持新合成器（例如 Sway，用 `SWAYSOCK`），修改 `src/main.rs` 里的 `detect_compositor()`，加一个分支即可：

```rust
fn detect_compositor() -> Option<&'static str> {
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        Some("hyprland")
    } else if env::var("NIRI_SOCKET").is_ok() {
        Some("niri")
    } else if env::var("SWAYSOCK").is_ok() {
        Some("sway")
    } else {
        None
    }
}
```

后续如果希望 compositor 列表也配置化，可考虑把检测条件挪进 `config.toml`（当前未实现）。

## 限制

- 只支持 Hyprland / niri（compositor 检测硬编码，见上节）
- 假定所有 shell service 都挂在 `graphical-session.target` 上下文下
- 停止/启动超时固定（10s / 15s），未做配置化
- 切换是"stop 全部 → start 目标"，不是"目标先起再停旧的"，切换期间有短暂空窗

## License

项目尚未声明 License（`Cargo.toml` 无 `license` 字段）。托管前建议补充，例如在 `Cargo.toml` 加 `license = "MIT"` 并放置对应 LICENSE 文件。
