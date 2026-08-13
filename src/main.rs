// shell-switcher — runtime switch between desktop shells on Wayland compositors.
//
// 设计:
//   每个 shell 是一个 systemd user service，挂 graphical-session.target 上下文，
//   但同一时刻只有一个在跑（都抢 org.freedesktop.Notifications DBus 名）。
//   set <name>: stop 全部 shell → 轮询确认都 inactive → start 目标 → 写 current 标记。
//   boot:      读 current 标记启动对应 shell（compositor autostart / shell-starter 入口）。
//   防呆:      非受支持 compositor 会话拒绝切换；启动失败回退默认 shell（config.toml 的 `default`）。
//
//   通用性：本工具不绑定任何具体 shell/compositor。默认 shell 由 config.toml 的 `default`
//   字段指定（缺省取第一个），compositor 支持列表见 detect_compositor。

#[macro_use]
extern crate rust_i18n;

i18n!("locales", fallback = "zh-CN");

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::thread::sleep;
use std::time::{Duration, Instant};

const CONFIG_REL: &str = ".config/shell-switcher/config.toml";
const CURRENT_REL: &str = ".config/shell-switcher/current";
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const START_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(serde::Deserialize, Debug)]
struct Config {
    /// 默认 shell 名（可选，缺省取第一个 [[shell]]）
    default: Option<String>,
    shell: Vec<ShellEntry>,
}

#[derive(serde::Deserialize, Clone, Debug)]
struct ShellEntry {
    name: String,
    service: String,
}

/// 按系统 locale 选语言：含 zh 用中文，否则英文。
/// 优先级 LC_ALL → LC_MESSAGES → LANG，空值视为未设置。
fn detect_locale() -> &'static str {
    let lang = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|v| env::var(v).ok())
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_lowercase();
    if lang.contains("zh") {
        "zh-CN"
    } else {
        "en"
    }
}

fn main() -> ExitCode {
    rust_i18n::set_locale(detect_locale());

    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    match cmd {
        "list" => list(),
        "current" => current(),
        "set" => set(args.get(2).map(String::as_str).unwrap_or("")),
        "boot" => boot(),
        "help" => help(),
        other => {
            eprintln!("{}", t!("unknown_command", other = other));
            help();
            ExitCode::from(2)
        }
    }
}

/// 检测当前 compositor。不在支持列表内返回 None（防呆）。
/// 新增 compositor：在此加环境变量分支即可（如 Sway 用 SWAYSOCK）。
fn detect_compositor() -> Option<&'static str> {
    if env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        Some("hyprland")
    } else if env::var("NIRI_SOCKET").is_ok() {
        Some("niri")
    } else {
        None
    }
}

fn home_dir() -> PathBuf {
    PathBuf::from(env::var("HOME").unwrap_or_else(|_| "/tmp".into()))
}

fn config_path() -> PathBuf {
    home_dir().join(CONFIG_REL)
}

fn current_path() -> PathBuf {
    home_dir().join(CURRENT_REL)
}

/// 读取并解析 config.toml；缺失/解析失败返回 None。
fn load_config() -> Option<Config> {
    let raw = fs::read_to_string(config_path()).ok()?;
    toml::from_str::<Config>(&raw).ok()
}

/// 可用 shell 列表（config.toml 的 [[shell]]）。
fn load_shells() -> Vec<ShellEntry> {
    load_config().map(|c| c.shell).unwrap_or_default()
}

/// 默认 shell：`default` 指定的 shell，未指定时取第一个；无 shell 时 None。
fn default_shell(shells: &[ShellEntry]) -> Option<&ShellEntry> {
    let cfg = load_config();
    let default = cfg.as_ref().and_then(|c| c.default.as_deref());
    shells
        .iter()
        .find(|s| Some(s.name.as_str()) == default)
        .or_else(|| shells.first())
}

/// 默认 shell 名（用于错误消息等）。
fn default_name(shells: &[ShellEntry]) -> String {
    default_shell(shells)
        .map(|s| s.name.clone())
        .unwrap_or_default()
}

fn find_shell<'a>(shells: &'a [ShellEntry], name: &str) -> Option<&'a ShellEntry> {
    shells.iter().find(|s| s.name == name)
}

fn is_active(service: &str) -> bool {
    Command::new("systemctl")
        .args(["--user", "is-active", service])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false)
}

fn stop_service(service: &str) {
    let _ = Command::new("systemctl")
        .args(["--user", "stop", service])
        .status();
}

fn start_service(service: &str) -> bool {
    Command::new("systemctl")
        .args(["--user", "start", service])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// stop 全部 shell service 并轮询等待都 inactive（带超时）。返回是否全部干净退出。
fn stop_all_shells(shells: &[ShellEntry]) -> bool {
    for s in shells {
        stop_service(&s.service);
    }
    let deadline = Instant::now() + STOP_TIMEOUT;
    loop {
        let any_active = shells.iter().any(|s| is_active(&s.service));
        if !any_active {
            return true;
        }
        if Instant::now() >= deadline {
            let still = shells
                .iter()
                .filter(|s| is_active(&s.service))
                .map(|s| s.service.clone())
                .collect::<Vec<_>>();
            eprintln!("{}", t!("stop_timeout", still = format!("{:?}", still)));
            return false;
        }
        sleep(Duration::from_millis(200));
    }
}

fn write_current(name: &str) {
    let p = current_path();
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(p, format!("{name}\n"));
}

fn read_current() -> Option<String> {
    fs::read_to_string(current_path())
        .ok()
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

fn list() -> ExitCode {
    for s in load_shells() {
        println!("  {}", s.name);
    }
    ExitCode::SUCCESS
}

fn current() -> ExitCode {
    if detect_compositor().is_none() {
        eprintln!("{}", t!("not_compositor"));
        return ExitCode::from(1);
    }
    let shells = load_shells();
    for s in &shells {
        if is_active(&s.service) {
            println!("{}", s.name);
            return ExitCode::SUCCESS;
        }
    }
    println!("{}", t!("current_none"));
    ExitCode::SUCCESS
}

fn set(target: &str) -> ExitCode {
    match detect_compositor() {
        Some(c) => println!("{}", t!("compositor_label", compositor = c)),
        None => {
            eprintln!(
                "{}",
                t!(
                    "set_denied",
                    desktop = env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
                )
            );
            return ExitCode::from(1);
        }
    }

    let shells = load_shells();
    if shells.is_empty() {
        eprintln!("{}", t!("no_shells", config = config_path().display()));
        return ExitCode::from(1);
    }

    let target_entry = match find_shell(&shells, target) {
        Some(e) => e.clone(),
        None => {
            eprintln!("{}", t!("unknown_shell", target = target));
            return ExitCode::from(2);
        }
    };

    // 目标 active 且其他 shell 均 inactive → 幂等 no-op。
    // 注意：即使目标 active，若有其他 shell 在跑（如 graphical-session.target 把默认拉起），
    // 仍需 stop_all 清理，确保同一时刻只有一个 shell（双顶栏/DBus 冲突）。
    let others_active = shells
        .iter()
        .any(|s| s.name != target && is_active(&s.service));
    if is_active(&target_entry.service) && !others_active {
        println!("{}", t!("already_running", target = target));
        write_current(target);
        return ExitCode::SUCCESS;
    }

    println!("{}", t!("stopping_all"));
    if !stop_all_shells(&shells) {
        eprintln!("{}", t!("stop_failed", default = default_name(&shells)));
        if let Some(d) = default_shell(&shells) {
            let _ = start_service(&d.service);
        }
        return ExitCode::from(1);
    }

    println!("{}", t!("starting", target = target));
    if !start_service(&target_entry.service) {
        eprintln!(
            "{}",
            t!("start_failed", target = target, default = default_name(&shells))
        );
        if let Some(d) = default_shell(&shells).cloned() {
            let _ = start_service(&d.service);
            write_current(&d.name);
        }
        return ExitCode::from(1);
    }

    // 确认真的起来了（systemd 可能还在 activating）
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if is_active(&target_entry.service) {
            println!("{}", t!("switched", target = target));
            write_current(target);
            return ExitCode::SUCCESS;
        }
        if Instant::now() >= deadline {
            eprintln!("{}", t!("start_timeout", target = target));
            return ExitCode::from(1);
        }
        sleep(Duration::from_millis(200));
    }
}

/// compositor autostart / shell-starter 入口：读 current 标记启动对应 shell。
/// current 标记缺失或无效时，启动默认 shell（config.toml 的 `default` 或第一个）。
fn boot() -> ExitCode {
    if detect_compositor().is_none() {
        eprintln!("{}", t!("boot_skip"));
        return ExitCode::from(0);
    }
    let shells = load_shells();
    if shells.is_empty() {
        eprintln!("{}", t!("no_shells", config = config_path().display()));
        return ExitCode::from(1);
    }

    let fallback = default_shell(&shells).map(|s| s.name.clone());
    let name = read_current()
        .filter(|n| find_shell(&shells, n).is_some())
        .or(fallback);

    let entry = match name.as_deref().and_then(|n| find_shell(&shells, n)) {
        Some(e) => e.clone(),
        None => {
            eprintln!("{}", t!("no_shells"));
            return ExitCode::from(1);
        }
    };
    println!(
        "{}",
        t!("boot_starting", name = entry.name, service = entry.service)
    );
    let ok = start_service(&entry.service);
    ExitCode::from(if ok { 0 } else { 1 })
}

fn help() -> ExitCode {
    let shells = load_shells();
    let default = default_shell(&shells)
        .map(|s| s.name.as_str())
        .unwrap_or("-");
    println!(
        "{}",
        t!("help_text", config = config_path().display(), default = default)
    );
    ExitCode::SUCCESS
}
