//! bash integration (#10678): a startup file bash reads through `--rcfile`
//! in place of the user's, which sources the user's own files first and then
//! adds the hooks. Nothing is written to the user's dotfiles, and readline
//! keeps its Emacs or vi editing, history, and completion.
//!
//! Supported: bash 4.4 and later, which have `bind -x` with `READLINE_LINE`
//! and `PS0`. An older bash, such as the 3.2 that ships with macOS, reads the
//! user's files and gets no hooks: an ordinary shell, where a line starting
//! with `# ` is only a comment.
//!
//! bash has no hook that runs on every keystroke, so it reports no buffer
//! and Enter always reaches the shell. A line starting with `# ` is the
//! explicit request: Enter hands it to OpenAgents instead of running it. A
//! command's text is the line Enter accepted; a command continued over
//! several lines records its first.

/// Sources the user's startup files as bash would have without `--rcfile`:
/// a login shell's profile when `OPENAGENTS_BASH_LOGIN` is `1`, otherwise
/// `~/.bashrc`.
pub const STARTUP: &str = r#"
if [[ ${OPENAGENTS_BASH_LOGIN-} == 1 ]]; then
  unset OPENAGENTS_BASH_LOGIN
  if [[ -r /etc/profile ]]; then source /etc/profile; fi
  for _openagents_file in "$HOME/.bash_profile" "$HOME/.bash_login" "$HOME/.profile"; do
    if [[ -r $_openagents_file ]]; then
      source "$_openagents_file"
      break
    fi
  done
  unset _openagents_file
else
  if [[ -r /etc/bash.bashrc ]]; then source /etc/bash.bashrc; fi
  if [[ -r $HOME/.bashrc ]]; then source "$HOME/.bashrc"; fi
fi
"#;

/// The hooks, after the user's files. They write the same OSC 133, OSC 7,
/// and OSC 777 marks the zsh hooks write ([`crate::zsh::HOOK`]).
pub const HOOK: &str = r#"
if [[ $- == *i* && -z ${_openagents_integrated-} ]] \
  && (( BASH_VERSINFO[0] > 4 || (BASH_VERSINFO[0] == 4 && BASH_VERSINFO[1] >= 4) )); then
  _openagents_integrated=1
  _openagents_running=0
  _openagents_output=''
  _openagents_hex() {
    REPLY=$(printf '%s' "$1" | LC_ALL=C od -An -v -tx1 | tr -d ' \n')
  }
  _openagents_directory() {
    local hex pair uri='' index
    _openagents_hex "$PWD"
    hex=$REPLY
    for (( index=0; index < ${#hex}; index+=2 )); do
      pair=${hex:index:2}
      if [[ $pair == 2f ]]; then uri+=/; else uri+=%$pair; fi
    done
    printf '\e]7;file://localhost%s\a' "$uri"
  }
  # The command table, sent when it changes: PATH, then alias and function
  # names (those starting with `_` excluded), bounded.
  _openagents_table() {
    local table names='' name
    for name in "${!BASH_ALIASES[@]}"; do names+="$name "; done
    table="p:$PATH"$'\n'"a:${names% }"
    names=''
    while read -r name; do
      if [[ $name != _* ]]; then names+="$name "; fi
    done < <(compgen -A function)
    table+=$'\n'"f:${names% }"
    table=${table:0:8000}
    if [[ $table != "${_openagents_table_sent-}" ]]; then
      _openagents_table_sent=$table
      _openagents_hex "$table"
      printf '\e]777;openagents;table;%s\a' "$REPLY"
    fi
  }
  _openagents_precmd() {
    local code=$?
    if [[ $_openagents_running == 1 ]]; then
      printf '\e]133;D;%d\a' "$code"
    fi
    _openagents_running=0
    _openagents_output=''
    if [[ $PWD != "${_openagents_pwd_sent-}" ]]; then
      _openagents_pwd_sent=$PWD
      _openagents_directory
    fi
    _openagents_table
    printf '\e]133;A\a'
    return "$code"
  }
  # Enter runs this before accept-line: a `# ` line becomes a request, and
  # any other line that is not blank is the command about to run. `PS0`
  # marks where its output starts.
  _openagents_accept() {
    local line=$READLINE_LINE
    if [[ $line == '# '* ]] && (( ${#line} <= 8192 )); then
      _openagents_hex "${line#\# }"
      printf '\e]777;openagents;request;%s\a' "$REPLY"
      READLINE_LINE=''
      READLINE_POINT=0
    elif [[ -n ${line//[[:space:]]/} && $_openagents_running != 1 ]] && (( ${#line} <= 8192 )); then
      _openagents_hex "$line"
      printf '\e]777;openagents;command;%s\a' "$REPLY"
      _openagents_running=1
      _openagents_output=$'\e]133;C\a'
    fi
  }
  # The terminal sends this key for Enter on a line it routed to a request.
  _openagents_ask() {
    local line=$READLINE_LINE
    if [[ -n $line ]] && (( ${#line} <= 8192 )); then
      history -s "$line"
      _openagents_hex "${line#\# }"
      printf '\e]777;openagents;request;%s\a' "$REPLY"
      READLINE_LINE=''
      READLINE_POINT=0
    fi
  }
  if [[ $(declare -p PROMPT_COMMAND 2>/dev/null) == 'declare -a'* ]]; then
    PROMPT_COMMAND=(_openagents_precmd "${PROMPT_COMMAND[@]}")
  else
    PROMPT_COMMAND="_openagents_precmd"$'\n'"${PROMPT_COMMAND-}"
  fi
  PS0='${_openagents_output}'"${PS0-}"
  for _openagents_map in emacs vi-insert vi-command; do
    bind -m "$_openagents_map" -x '"\e[24243~": _openagents_accept' 2>/dev/null
    bind -m "$_openagents_map" '"\e[24244~": accept-line' 2>/dev/null
    bind -m "$_openagents_map" '"\C-m": "\e[24243~\e[24244~"' 2>/dev/null
    bind -m "$_openagents_map" '"\C-j": "\e[24243~\e[24244~"' 2>/dev/null
    bind -m "$_openagents_map" -x '"\e[24242~": _openagents_ask' 2>/dev/null
  done
  unset _openagents_map
fi
"#;

/// The whole startup file: the user's files, then the hooks.
#[must_use]
pub fn rcfile() -> String {
    format!("{STARTUP}\n{HOOK}")
}
