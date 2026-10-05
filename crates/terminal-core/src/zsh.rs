//! zsh integration sourced after the user's configuration. The shell keeps its editor.

pub const HOOK: &str = r#"
if [[ -o interactive ]] && (( ! ${+_openagents_integrated} )); then
  typeset -g _openagents_integrated=1
  autoload -Uz add-zsh-hook add-zle-hook-widget
  zmodload zsh/parameter 2>/dev/null
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
    _openagents_table
    printf '\e]133;A\a'
  }
  # The command table, sent when it changes: PATH, then alias and function
  # names (completion functions excluded), bounded.
  _openagents_table() {
    local table="p:$PATH"$'\n'"a:${(j: :)${(k)aliases}}"$'\n'"f:${(j: :)${(k)functions[(I)[^_]*]}}"
    table=${table[1,8000]}
    if [[ $table != ${_openagents_table_sent-} ]]; then
      typeset -g _openagents_table_sent=$table
      # One fork when it changes; the per-character encoder is slow at this size.
      REPLY=$(print -rn -- "$table" | od -An -v -tx1 | tr -d " \n")
      printf '\e]777;openagents;table;%s\a' "$REPLY"
    fi
  }
  _openagents_preexec() {
    _openagents_hex "$1"
    printf '\e]777;openagents;command;%s\a\e]133;C\a' "$REPLY"
    typeset -g _openagents_running=1
  }
  _openagents_line_init() { printf '\e]133;B\a'; }
  _openagents_buffer() {
    if (( ${#BUFFER} <= 8192 )); then
      # The first word's kind from the shell's own tables, without a fork.
      local first=${${(z)BUFFER}[1]} kind=none
      if [[ -z $first ]]; then kind=''
      elif (( ${+aliases[$first]} )); then kind=alias
      elif (( ${+functions[$first]} )); then kind=function
      elif (( ${+builtins[$first]} )); then kind=builtin
      elif (( ${reswords[(Ie)$first]} )); then kind=reserved
      elif (( ${+commands[$first]} )); then kind=command
      fi
      _openagents_hex "$kind"
      printf '\e]777;openagents;word;%s\a' "$REPLY"
      _openagents_hex "$BUFFER"
      printf '\e]777;openagents;buffer;%s\a' "$REPLY"
    fi
  }
  # The terminal sends this key for Enter on a line it routed to a request.
  _openagents_ask() {
    if [[ -n $BUFFER ]] && (( ${#BUFFER} <= 8192 )); then
      print -s -- "$BUFFER"
      _openagents_hex "${BUFFER#\# }"
      printf '\e]777;openagents;request;%s\a' "$REPLY"
      BUFFER=''
      zle reset-prompt
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
  zle -N _openagents_ask
  bindkey -M emacs '\e[24242~' _openagents_ask
  bindkey -M viins '\e[24242~' _openagents_ask
  bindkey -M vicmd '\e[24242~' _openagents_ask
fi
"#;
