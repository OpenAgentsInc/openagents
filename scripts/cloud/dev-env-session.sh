#!/usr/bin/env bash
# Sign a cloud development environment in for OpenAgents work, with no key
# file copied onto it (docs/cloud/dogfood-dev-on-prod.md).
#
#   eval "$(scripts/cloud/dev-env-session.sh)"      # in the shell that works
#   scripts/cloud/dev-env-session.sh --check        # say what is signed in
#
# Every credential comes from Secret Manager (project openagentsgemini)
# through the VM's own service account (the metadata server), at the moment
# a session starts. They live in this shell's environment and in a mode-600
# file, ~/.openagents/dev-env.env, on this VM's own disk, and are never
# printed. (Not /dev/shm: systemd-logind's RemoveIPC deletes a user's
# /dev/shm files when their last ssh session ends.)
#
# | Variable                 | Secret                        |
# | GH_TOKEN                 | coder-pool-git-token (repo, project scopes) |
# | CLAUDE_CODE_OAUTH_TOKEN  | dev-claude-code-oauth-token (claude setup-token) |
# | TYPESAFE_API_KEY         | dev-typesafe-api-key (Jev)    |
# | OPENROUTER_API_KEY       | openagents-openrouter-api-key |
#
# Google Cloud itself (deploys, Cloud Run, logs) needs nothing: gcloud on
# the VM already acts as the attached service account.
set -euo pipefail

project=${OA_PROJECT:-openagentsgemini}
md=http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default
file=$HOME/.openagents/dev-env.env
mkdir -p "$HOME/.openagents"

token() {
  curl -fsS -H 'Metadata-Flavor: Google' "$md/token" | jq -r .access_token
}

secret() { # name -> value on stdout, or nothing
  curl -fsS -H "Authorization: Bearer $access" \
    "https://secretmanager.googleapis.com/v1/projects/$project/secrets/$1/versions/latest:access" 2>/dev/null \
    | jq -r '.payload.data // empty' | base64 -d 2>/dev/null || true
}

if [[ ${1:-} == --check ]]; then
  [[ -r $file ]] && set -a && . "$file" && set +a
  for v in GH_TOKEN CLAUDE_CODE_OAUTH_TOKEN TYPESAFE_API_KEY OPENROUTER_API_KEY; do
    if [[ -n ${!v:-} ]]; then echo "$v: set"; else echo "$v: missing"; fi
  done
  gh api user --jq '"github: " + .login' 2>/dev/null || echo "github: not signed in"
  echo "gcloud: $(curl -fsS -H 'Metadata-Flavor: Google' "$md/email")"
  exit 0
fi

access=$(token)
umask 077
tmp=$(mktemp "$file.XXXXXX")
missing=()
for pair in GH_TOKEN=coder-pool-git-token \
            CLAUDE_CODE_OAUTH_TOKEN=dev-claude-code-oauth-token \
            TYPESAFE_API_KEY=dev-typesafe-api-key \
            OPENROUTER_API_KEY=openagents-openrouter-api-key; do
  name=${pair%%=*}
  value=$(secret "${pair#*=}")
  if [[ -n $value ]]; then
    printf 'export %s=%q\n' "$name" "$value" >>"$tmp"
  else
    missing+=("${pair#*=}")
  fi
done
mv -f "$tmp" "$file"
set -a; . "$file"; set +a

# Git as the token's account, over HTTPS with gh as the helper.
if [[ -n ${GH_TOKEN:-} ]]; then
  gh auth setup-git >/dev/null 2>&1 || true
  user=$(gh api user 2>/dev/null || true)
  name=$(jq -r '.name // .login // empty' <<<"$user")
  email=$(jq -r '.email // empty' <<<"$user")
  [[ -z ${name:-} ]] || git config --global user.name "$name"
  [[ -z ${email:-} ]] || git config --global user.email "$email"
fi
# Claude Code's own login file from the subscription token. Coder's engines
# start from the login shell's environment less every *_TOKEN variable, so
# CLAUDE_CODE_OAUTH_TOKEN alone does not reach them; the login file does,
# as `claude` and log in would write it. Mode 600, on this VM's disk only.
if [[ -n ${CLAUDE_CODE_OAUTH_TOKEN:-} ]]; then
  install -d -m 700 "$HOME/.claude"
  jq -n --arg t "$CLAUDE_CODE_OAUTH_TOKEN" \
    '{claudeAiOauth: {accessToken: $t, refreshToken: null,
      expiresAt: ((now + 300 * 86400) * 1000 | floor),
      scopes: ["user:inference", "user:profile"]}}' >"$HOME/.claude/.credentials.json.tmp"
  mv -f "$HOME/.claude/.credentials.json.tmp" "$HOME/.claude/.credentials.json"
fi
# Every login shell picks the session up.
for f in "$HOME/.profile" "$HOME/.bashrc"; do
  grep -qF "$file" "$f" 2>/dev/null || printf '[ -r %s ] && { set -a; . %s; set +a; }\n' "$file" "$file" >>"$f"
done

((${#missing[@]} == 0)) || echo "echo 'dev-env: not found in Secret Manager: ${missing[*]}' >&2"
echo "set -a; . $file; set +a"
