# shell-switcher fish completion
complete -c shell-switcher -f -n '__fish_use_subcommand' -a 'list' -d '列出可用 shell'
complete -c shell-switcher -f -n '__fish_use_subcommand' -a 'current' -d '显示当前 active 的 shell'
complete -c shell-switcher -f -n '__fish_use_subcommand' -a 'set' -d '切换到指定 shell'
complete -c shell-switcher -f -n '__fish_use_subcommand' -a 'boot' -d '读 current 标记启动（shell-starter 入口）'
complete -c shell-switcher -f -n '__fish_use_subcommand' -a 'help' -d '帮助'
# set 子命令：动态补全 config.toml 里的 shell 名
complete -c shell-switcher -f -n '__fish_seen_subcommand_from set' -a '(shell-switcher list 2>/dev/null)'
