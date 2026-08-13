// shell-switcher — runtime switch between desktop shells on Hyprland/niri.
//
// 设计:
//   每个 shell 是一个 systemd user service，挂 graphical-session.target 上下文，
//   但同一时刻只有一个在跑（都抢 org.freedesktop.Notifications DBus 名）。
//   set <name>: stop 全部 shell → 轮询确认都 inactive → start 目标 → 写 current 标记。
//   boot:      读 current 标记启动对应 shell（compositor autostart / shell-starter 入口）。
//   防呆:      非 Hyprland/niri 会话拒绝切换；启动失败回退默认 shell（noctalia）。

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};
use std::thread::sleep;
use std::time::{Duration, Instant};

const DEFAULT_SHELL: &str = "noctalia";
const CONFIG_REL: &str = ".config/shell-switcher/config.toml";
const CURRENT_REL: &str = ".config/shell-switcher/current";
const STOP_TIMEOUT: Duration = Duration::from_secs(10);
const START_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(serde::Deserialize, Debug)]
struct Config {
    shell: Vec<ShellEntry>,
}

#[derive(serde::Deserialize, Clone, Debug)]
struct ShellEntry {
    name: String,
    service: String,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let cmd = args.get(1).map(String::as_str).unwrap_or("help");

    match cmd {
        "list" => list(),
        "current" => current(),
        "set" => set(args.get(2).map(String::as_str).unwrap_or("")),
        "boot" => boot(),
        "help" => help(),
        other => {
            eprintln!("未知命令: {other}");
            help();
            ExitCode::from(2)
        }
    }
}

/// 检测当前 compositor。非 Hyprland/niri 返回 None（防呆）。
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

/// 读 config.toml；缺失/解析失败回退默认 [noctalia]。
fn load_shells() -> Vec<ShellEntry> {
    match fs::read_to_string(config_path()) {
        Ok(raw) => match toml::from_str::<Config>(&raw) {
            Ok(cfg) if !cfg.shell.is_empty() => cfg.shell,
            _ => default_shells(),
        },
        Err(_) => default_shells(),
    }
}

fn default_shells() -> Vec<ShellEntry> {
    vec![ShellEntry {
        name: DEFAULT_SHELL.into(),
        service: format!("{DEFAULT_SHELL}.service"),
    }]
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
            eprintln!("超时: 以下 service 仍未退出: {still:?}");
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
        eprintln!("非 Hyprland/niri 会话，shell-switcher 不适用。");
        return ExitCode::from(1);
    }
    let shells = load_shells();
    for s in &shells {
        if is_active(&s.service) {
            println!("{}", s.name);
            return ExitCode::SUCCESS;
        }
    }
    println!("none");
    ExitCode::SUCCESS
}

fn set(target: &str) -> ExitCode {
    match detect_compositor() {
        Some(c) => println!("compositor: {c}"),
        None => {
            eprintln!(
                "非 Hyprland/niri 会话（XDG_CURRENT_DESKTOP={}），拒绝切换。",
                env::var("XDG_CURRENT_DESKTOP").unwrap_or_default()
            );
            return ExitCode::from(1);
        }
    }

    let shells = load_shells();
    let target_entry = match find_shell(&shells, target) {
        Some(e) => e.clone(),
        None => {
            eprintln!("未知 shell: {target}（shell-switcher list 查看）");
            return ExitCode::from(2);
        }
    };

    // 已是目标且 active → 幂等 no-op
    if is_active(&target_entry.service) {
        println!("{target} 已在运行。");
        write_current(target);
        return ExitCode::SUCCESS;
    }

    println!("停止所有 shell…");
    if !stop_all_shells(&shells) {
        eprintln!("有 shell 未能停止，放弃切换，尝试回退 {DEFAULT_SHELL}。");
        let default = find_shell(&shells, DEFAULT_SHELL).cloned();
        if let Some(d) = default {
            let _ = start_service(&d.service);
        }
        return ExitCode::from(1);
    }

    println!("启动 {target}…");
    if !start_service(&target_entry.service) {
        eprintln!("{target} 启动失败，回退 {DEFAULT_SHELL}。");
        let default = find_shell(&shells, DEFAULT_SHELL).cloned();
        if let Some(d) = default {
            let _ = start_service(&d.service);
            write_current(DEFAULT_SHELL);
        }
        return ExitCode::from(1);
    }

    // 确认真的起来了（systemd 可能还在 activating）
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        if is_active(&target_entry.service) {
            println!("已切换到 {target}。");
            write_current(target);
            return ExitCode::SUCCESS;
        }
        if Instant::now() >= deadline {
            eprintln!("{target} 未在超时内进入 active，可能起不来。");
            return ExitCode::from(1);
        }
        sleep(Duration::from_millis(200));
    }
}

/// compositor autostart / shell-starter 入口：读 current 标记启动对应 shell。
fn boot() -> ExitCode {
    if detect_compositor().is_none() {
        eprintln!("非 Hyprland/niri 会话，跳过 shell 启动。");
        return ExitCode::from(0);
    }
    let shells = load_shells();
    let name = read_current().filter(|n| find_shell(&shells, n).is_some()).unwrap_or_else(|| DEFAULT_SHELL.into());
    let entry = find_shell(&shells, &name).expect("DEFAULT_SHELL 必在 config");
    println!("boot: 启动 {name}（{service}）", service = entry.service);
    let ok = start_service(&entry.service);
    ExitCode::from(if ok { 0 } else { 1 })
}

fn help() -> ExitCode {
    println!(
        "shell-switcher — 桌面 shell 运行时切换器

用法:
  shell-switcher list                列出可用 shell
  shell-switcher current             显示当前 active 的 shell
  shell-switcher set <name>          切换到指定 shell（stop-all → await → start）
  shell-switcher boot                shell-starter 入口（读 current 标记启动）
  shell-switcher help                本帮助

配置: {config}
  shell-switcher 只做切换，shell 的 systemd service 由 NixOS 仓库声明。
防呆: 非 Hyprland/niri 会话拒绝切换；切换失败自动回退 {default}。",
        config = config_path().display(),
        default = DEFAULT_SHELL
    );
    ExitCode::SUCCESS
}
