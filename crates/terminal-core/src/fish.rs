//! fish integration (#10679): a script fish sources through `--init-command`
//! after it reads the user's configuration, so nothing is written to the
//! user's files. The marks come from event handlers; the one key bound is
//! Enter, which runs fish's own `execute` for every line but a request, and
//! only when the user has not bound Enter. fish keeps its abbreviations,
//! autosuggestions, completion, history, multiline input, and the user's
//! other bindings.
//!
//! Supported: fish 3.3 and later (tested with 4.9). An older fish starts
//! without hooks.
//!
//! fish reports no line as you type, so the terminal sends Enter to fish
//! for every line. A line starting with `# ` is the explicit request. A
//! command's text is the first line fish ran.

/// The hooks. They write the same OSC 133, OSC 7, and OSC 777 marks the zsh
/// hooks write ([`crate::zsh::HOOK`]).
pub const HOOK: &str = r#"
if status is-interactive; and not set -q _openagents_integrated
    and string match -qr '^(3\.([3-9]|[1-9][0-9])|[4-9]|[1-9][0-9])' -- $version
    set -g _openagents_integrated 1
    set -g _openagents_running 0
    function _openagents_hex
        set -g _openagents_reply (printf '%s' $argv[1] | od -An -v -tx1 | string replace -a ' ' '' | string join '')
    end
    # The command table, sent when it changes: PATH, then abbreviations
    # (fish's aliases are functions) and function names (those starting
    # with `_` excluded), bounded.
    function _openagents_table
        set -l path (string join : -- $PATH)
        set -l abbreviations (abbr --list | string join ' ')
        set -l names
        for name in (functions --names)
            string match -q '_*' -- $name; or set -a names $name
        end
        set -l functions (string join ' ' -- $names)
        set -l table (string sub -l 8000 -- "p:$path"\n"a:$abbreviations"\n"f:$functions" | string collect)
        if test "$table" != "$_openagents_table_sent"
            set -g _openagents_table_sent $table
            _openagents_hex $table
            printf '\e]777;openagents;table;%s\a' $_openagents_reply
        end
    end
    function _openagents_prompt --on-event fish_prompt
        if test "$PWD" != "$_openagents_pwd_sent"
            set -g _openagents_pwd_sent $PWD
            printf '\e]7;file://localhost%s\a' (string escape --style=url -- $PWD)
        end
        _openagents_table
        printf '\e]133;A\a'
    end
    function _openagents_preexec --on-event fish_preexec
        set -l line (string split -m1 \n -- $argv[1])[1]
        if test (string length -- "$line") -gt 8192
            set line ''
        end
        _openagents_hex $line
        printf '\e]777;openagents;command;%s\a\e]133;C\a' $_openagents_reply
        set -g _openagents_running 1
    end
    # Enter on a `# ` line hands it to OpenAgents instead of running it;
    # any other line runs as fish's own Enter runs it.
    function _openagents_accept
        set -l line (commandline)
        if test (count $line) -eq 1; and string match -q '# *' -- $line
            and test (string length -- $line) -le 8192
            _openagents_hex (string sub -s 3 -- $line)
            printf '\e]777;openagents;request;%s\a' $_openagents_reply
            commandline -r ''
            commandline -f repaint
        else
            commandline -f execute
        end
    end
    # A binding of Enter the user made is theirs: the hooks leave it, and a
    # `# ` line is then only a comment.
    if not bind --user \r >/dev/null 2>&1
        bind \r _openagents_accept
        bind -M insert \r _openagents_accept
    end
    function _openagents_postexec --on-event fish_postexec
        set -l code $status
        if test $_openagents_running = 1
            printf '\e]133;D;%d\a' $code
        end
        set -g _openagents_running 0
    end
end
"#;
