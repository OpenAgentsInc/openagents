//! Pushing from a Claude Code run (#11226): GitHub access for one run, from
//! the signed-in person's own GitHub connection.
//!
//! The web asks the account service for a broker ticket for the run's one
//! repository (`oa_auth::repos::broker`) and hands it here as [`Push`]. The
//! run's computer gets the ticket as the `OPENAGENTS_GIT_BROKER` credential
//! (with the run's Claude credential, released for the run only) and never
//! a GitHub token:
//!
//! - Git in the working directory asks the broker through the helper
//!   [`coder_environment_setup::git_auth_env`] describes, for an
//!   installation token scoped to that one repository, on every fetch or
//!   push. The helper in `.git/config` names the variable, not its value.
//! - `open-pull-request` (in `~/.local/bin`) gets a token the same way and
//!   opens a pull request with `curl`; the token stays inside that script
//!   and is never printed.
//!
//! The computer is deleted when the run ends, and nothing here is written
//! into a saved image.

use coder_environment_setup::{GIT_BROKER, git_auth_env};

/// Where pull requests are opened.
pub const GITHUB_API: &str = "https://api.github.com";
/// The script a run opens a pull request with.
pub const PULL_REQUEST: &str = "open-pull-request";

/// GitHub access for one run: the repository (`owner/name`), the branch
/// pull requests target, the person's GitHub login (for commits), and the
/// broker credential (`<broker URL> <ticket>`). The credential never
/// prints, and its bytes are zeroed when it drops.
pub struct Push {
    repository: String,
    base: String,
    login: Option<String>,
    credential: String,
}

impl Push {
    /// # Errors
    /// When the repository, branch, login, or credential can't be used.
    pub fn new(
        repository: &str,
        base: &str,
        login: Option<&str>,
        credential: String,
    ) -> Result<Self, String> {
        let push = Self {
            repository: repository.to_owned(),
            base: base.to_owned(),
            login: login.map(str::to_owned),
            credential,
        };
        let plain = |text: &str, slash: bool| {
            !text.is_empty()
                && text.len() <= 200
                && !text.starts_with(['-', '.', '/'])
                && text.bytes().all(|b| {
                    b.is_ascii_alphanumeric()
                        || matches!(b, b'.' | b'_' | b'-')
                        || (slash && b == b'/')
                })
        };
        let repository_ok = plain(repository, true) && repository.matches('/').count() == 1;
        let (url, ticket) = push.credential.split_once(' ').unwrap_or_default();
        if !repository_ok
            || !plain(base, true)
            || push.login.as_deref().is_some_and(|l| !plain(l, false))
            || !(url.starts_with("https://")
                || url.starts_with("http://127.0.0.1:")
                || url.starts_with("http://localhost:"))
            || ticket.is_empty()
            || ticket.contains(char::is_whitespace)
        {
            return Err("GitHub access for this run can't be used.".into());
        }
        Ok(push)
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }

    /// The credential's name on the computer.
    pub fn name(&self) -> &'static str {
        GIT_BROKER
    }

    pub(super) fn value(&self) -> String {
        self.credential.clone()
    }
}

impl Drop for Push {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.credential).into_bytes();
        bytes.fill(0);
    }
}

impl std::fmt::Debug for Push {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Push({}, redacted)", self.repository)
    }
}

/// What Claude Code is told about GitHub, after the task: how to push and
/// open a pull request, or that this run can't.
pub fn task_note(push: Option<&Push>) -> String {
    match push {
        Some(push) => format!(
            "\n\nYou can push to https://github.com/{repo} as the person: commit on a new branch, push it with `git push origin HEAD:refs/heads/BRANCH`, then open a pull request with `{PULL_REQUEST} --title TITLE --body-file FILE` (it targets {base} and prints the link). Push to {base} itself only when the person asked for that.",
            repo = push.repository,
            base = push.base,
        ),
        None => "\n\nThis run has no GitHub access, so it can't push or open a pull request. Leave your change committed here and say what you changed.".into(),
    }
}

/// The command that readies `workdir` for pushing: Git asks the broker
/// (and no other helper) for `github.com`, `origin` is the repository's
/// plain address, commits carry the person's login when Git has no name,
/// and `open-pull-request` is installed. `api` is where pull requests are
/// opened ([`GITHUB_API`]).
pub fn setup_script(push: &Push, workdir: &str, api: &str) -> String {
    let env = git_auth_env(GIT_BROKER);
    let helper = &env["GIT_CONFIG_VALUE_1"];
    let q = boat::shell_quote;
    let url = format!("https://github.com/{}.git", push.repository);
    let identity = push
        .login
        .as_deref()
        .map(|login| {
            format!(
                "git config user.name >/dev/null 2>&1 || git config user.name {name}\ngit config user.email >/dev/null 2>&1 || git config user.email {email}\n",
                name = q(login),
                email = q(&format!("{login}@users.noreply.github.com")),
            )
        })
        .unwrap_or_default();
    format!(
        r#"set -eu
cd {workdir}
git rev-parse --git-dir >/dev/null 2>&1 || git init -q
git config --unset-all credential.helper 2>/dev/null || true
git config --add credential.helper ''
git config --add credential.helper {helper}
git config credential.https://github.com.useHttpPath true
if git remote get-url origin >/dev/null 2>&1; then git remote set-url origin {url}; else git remote add origin {url}; fi
{identity}mkdir -p "$HOME/.local/bin"
cat > "$HOME/.local/bin/{PULL_REQUEST}" <<'OA_PULL_REQUEST'
{script}OA_PULL_REQUEST
chmod 755 "$HOME/.local/bin/{PULL_REQUEST}"
"#,
        workdir = q(workdir),
        helper = q(helper),
        url = q(&url),
        script = pull_request_script(&push.repository, &push.base, workdir, api),
    )
}

/// `open-pull-request --title TEXT [--body TEXT | --body-file FILE]
/// [--head BRANCH] [--base BRANCH]`: opens a pull request on `repository`
/// from a pushed branch (the current one by default) into `base`, with a
/// token Git's helper gets from the broker. The token is passed to `curl`
/// in a private file, never as an argument, and never printed.
fn pull_request_script(repository: &str, base: &str, workdir: &str, api: &str) -> String {
    let q = boat::shell_quote;
    format!(
        r#"#!/bin/sh
set -u
repo={repo}
base={base}
title=
body=
head=
usage() {{ echo "usage: {PULL_REQUEST} --title TEXT [--body TEXT | --body-file FILE] [--head BRANCH] [--base BRANCH]" >&2; exit 2; }}
while [ $# -gt 0 ]; do
  case "$1" in
    --title) [ $# -ge 2 ] || usage; title=$2; shift 2 ;;
    --body) [ $# -ge 2 ] || usage; body=$2; shift 2 ;;
    --body-file) [ $# -ge 2 ] || usage; body=$(cat "$2") || exit 1; shift 2 ;;
    --head) [ $# -ge 2 ] || usage; head=$2; shift 2 ;;
    --base) [ $# -ge 2 ] || usage; base=$2; shift 2 ;;
    *) usage ;;
  esac
done
[ -n "$title" ] || usage
cd {workdir} || exit 1
[ -n "$head" ] || head=$(git rev-parse --abbrev-ref HEAD)
[ "$head" != HEAD ] || {{ echo "Check out the branch you pushed, or name it with --head." >&2; exit 2; }}
esc() {{ printf '%s' "$1" | tr '\011' ' ' | tr -d '\000-\010\013-\037' | sed -e 's/\\/\\\\/g' -e 's/"/\\"/g' | awk 'BEGIN {{ ORS = "" }} {{ if (NR > 1) print "\\n"; print }}'; }}
umask 077
t=$(mktemp -d) || exit 1
trap 'rm -rf "$t"' EXIT
printf 'protocol=https\nhost=github.com\npath=%s.git\n\n' "$repo" | GIT_TERMINAL_PROMPT=0 git credential fill 2>/dev/null | sed -n 's/^password=//p' > "$t/token"
if [ ! -s "$t/token" ]; then echo "GitHub access isn't available for this run." >&2; exit 1; fi
printf 'Authorization: Bearer %s\n' "$(cat "$t/token")" > "$t/headers"
rm -f "$t/token"
printf '{{"title":"%s","head":"%s","base":"%s","body":"%s"}}' "$(esc "$title")" "$(esc "$head")" "$(esc "$base")" "$(esc "$body")" > "$t/body.json"
code=$(curl -sS --max-time 30 -o "$t/answer" -w '%{{http_code}}' -X POST -H @"$t/headers" -H 'Accept: application/vnd.github+json' -H 'X-GitHub-Api-Version: 2022-11-28' -H 'Content-Type: application/json' --data-binary @"$t/body.json" {api}/repos/"$repo"/pulls) || {{ echo "GitHub didn't answer." >&2; exit 1; }}
link=$(tr ',' '\n' < "$t/answer" | sed -n 's/^ *{{*"html_url": *"\(https:[^"]*\/pull\/[0-9][0-9]*\)".*/\1/p' | head -n 1)
if [ "$code" = 201 ] && [ -n "$link" ]; then echo "$link"; exit 0; fi
message=$(tr ',' '\n' < "$t/answer" | sed -n 's/^ *{{*"message": *"\([^"]*\)".*/\1/p' | head -n 1)
echo "GitHub didn't open the pull request ($code): ${{message:-no reason given}}" >&2
exit 1
"#,
        repo = q(repository),
        base = q(base),
        workdir = q(workdir),
        api = q(api.trim_end_matches('/')),
    )
}

#[cfg(test)]
#[path = "push_tests.rs"]
mod tests;
