# shell-switcher bash completion
_shell-switcher() {
    local cur
    cur="${COMP_WORDS[COMP_CWORD]}"

    # 一级命令
    if [[ ${COMP_CWORD} -eq 1 ]]; then
        COMPREPLY=( $(compgen -W "list current set boot help" -- "${cur}") )
        return 0
    fi

    # set 子命令：动态补全 config.toml 里的 shell 名
    if [[ ${COMP_WORDS[1]} == "set" && ${COMP_CWORD} -eq 2 ]]; then
        COMPREPLY=( $(compgen -W "$(shell-switcher list 2>/dev/null)" -- "${cur}") )
        return 0
    fi

    COMPREPLY=()
}
complete -F _shell-switcher shell-switcher
