// shell-switcher — runtime switch between desktop shells on Hyprland/niri.
//
// 骨架 v0.1：CLI 结构 + compositor 检测 + 切换状态机占位。
// 完整逻辑（systemctl 启停、等待确认、防呆、崩溃回退）在打磨阶段实现。

use std::process::Command;

/// 已知 shell 及其 systemd user service 名。
/// 后续接入 DMS/caelestia/Persona 时在这里追加。
const SHELLS: &[(&str, &str)] = &[("noctalia", "noctalia.service")];

fn main() {
    let args: Vec<String> = std::env::args().collect();
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
            std::process::exit(2);
        }
    }
}

/// 检测当前 compositor。非 Hyprland/niri 时返回 None（防呆）。
fn detect_compositor() -> Option<&'static str> {
    if std::env::var("HYPRLAND_INSTANCE_SIGNATURE").is_ok() {
        Some("hyprland")
    } else if std::env::var("NIRI_SOCKET").is_ok() {
        Some("niri")
    } else {
        None
    }
}

fn list() {
    println!("可用 shell:");
    for (name, _) in SHELLS {
        println!("  {name}");
    }
}

fn current() {
    let compositor = detect_compositor();
    if compositor.is_none() {
        eprintln!("非 Hyprland/niri 会话，shell-switcher 不适用。");
        std::process::exit(1);
    }
    // 查找当前 active 的 shell service
    for (name, service) in SHELLS {
        let out = Command::new("systemctl")
            .args(["--user", "is-active", service])
            .output();
        if let Ok(o) = out {
            if String::from_utf8_lossy(&o.stdout).trim() == "active" {
                println!("{name}");
                return;
            }
        }
    }
    println!("none");
}

fn set(target: &str) {
    let compositor = match detect_compositor() {
        Some(c) => c,
        None => {
            eprintln!("非 Hyprland/niri 会话，拒绝切换。用 --force 可忽略。");
            std::process::exit(1);
        }
    };
    println!("compositor: {compositor}, target shell: {target}");

    // TODO(打磨阶段): 切换状态机
    //  1. systemctl --user stop 所有 shell service
    //  2. 轮询 systemctl --user is-active 直到全部 inactive（带超时）
    //  3. systemctl --user start 目标 service
    //  4. 确认 active；失败则回退 Noctalia
    let _ = (compositor, target);
    eprintln!("未实现：切换逻辑在打磨阶段完成。");
}

fn boot() {
    // compositor autostart 入口：读 ~/.config/shell-switcher/current 启动对应 shell。
    // TODO(打磨阶段)
    eprintln!("未实现：boot 在打磨阶段完成。");
}

fn help() {
    println!(
        "shell-switcher — 桌面 shell 运行时切换器

用法:
  shell-switcher list                列出可用 shell
  shell-switcher current             显示当前 active 的 shell
  shell-switcher set <name>          切换到指定 shell（杀旧起新）
  shell-switcher boot                compositor autostart 入口（读 current 标记启动）
  shell-switcher help                本帮助"
    );
}
