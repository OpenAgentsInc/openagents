//! zsh integration sourced after the user's configuration. The shell keeps its editor.

pub const HOOK: &str = r#"
if [[ -o interactive ]] && (( ! ${+_openagents_integrated} )); then
  typeset -g _openagents_integrated=1
  autoload -Uz add-zsh-hook add-zle-hook-widget
  _openagents_hex() {
    local LC_ALL=C word="$1" hex='' char
    local -i index
    for (( index=1; index <= ${#word}; ++index )); do
      printf -v char '%02x' "'$word[index]"
      hex+=$char
    done
    REPLY=$hex
  }
  _openagents_directory() {
    local LC_ALL=C word="$PWD" uri='' char
    local -i index
    for (( index=1; index <= ${#word}; ++index )); do
      char=$word[index]
      case $char in
        [a-zA-Z0-9/._~-]) uri+=$char ;;
        *) printf -v char '%%%02X' "'$char"; uri+=$char ;;
      esac
    done
    printf '\e]7;file://localhost%s\a' "$uri"
  }
  _openagents_precmd() {
    local code=$?
    if (( ${_openagents_running:-0} )); then
      printf '\e]133;D;%d\a' "$code"
    fi
    typeset -g _openagents_running=0
    _openagents_directory
    printf '\e]133;A\a'
  }
  _openagents_preexec() {
    _openagents_hex "$1"
    printf '\e]777;openagents;command;%s\a\e]133;C\a' "$REPLY"
    typeset -g _openagents_running=1
  }
  _openagents_line_init() { printf '\e]133;B\a'; }
  _openagents_buffer() {
    if (( ${#BUFFER} <= 8192 )); then
      _openagents_hex "$BUFFER"
      printf '\e]777;openagents;buffer;%s\a' "$REPLY"
    fi
  }
  _openagents_accept() {
    if [[ $BUFFER == '# '* ]] && (( ${#BUFFER} <= 8192 )); then
      _openagents_hex "${BUFFER#\# }"
      printf '\e]777;openagents;request;%s\a' "$REPLY"
      BUFFER=''
      zle reset-prompt
    else
      zle _openagents_original_accept
    fi
  }
  add-zsh-hook precmd _openagents_precmd
  add-zsh-hook preexec _openagents_preexec
  add-zle-hook-widget line-init _openagents_line_init
  add-zle-hook-widget line-pre-redraw _openagents_buffer
  zle -A accept-line _openagents_original_accept
  zle -N accept-line _openagents_accept
fi
"#;
