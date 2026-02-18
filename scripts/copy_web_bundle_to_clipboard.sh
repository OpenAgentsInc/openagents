#!/usr/bin/env bash
set -euo pipefail

# Copies apps/openagents.com/ and apps/website/ (minus gitignored and long/noisy files) to the
# system clipboard with file path dividers. Target: under 400 KB so an AI can
# understand both projects well enough to give advice.

repo_root=$(
  cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd
)
cd "$repo_root"

tmp_file="$(mktemp)"
trap 'rm -f "$tmp_file"' EXIT

append_file() {
  local abs_path="$1"
  local rel_path="${abs_path#"$repo_root"/}"

  {
    printf "\n===== FILE: %s =====\n\n" "$rel_path"
    cat "$abs_path"
    printf "\n\n===== END FILE: %s =====\n" "$rel_path"
  } >>"$tmp_file"
}

# Skip paths we explicitly exclude. Input is repo-relative path.
# Goal: keep app structure, config, source, key docs; drop content blobs and generated.
should_skip() {
  local rel="$1"
  local base
  base=$(basename "$rel")

  # Lockfiles and large generated/config blobs
  case "$base" in
    package-lock.json|pnpm-lock.yaml|yarn.lock|bun.lockb) return 0 ;;
    *.lock) return 0 ;;
    worker-configuration.d.ts) return 0 ;;  # Cloudflare generated, ~400KB
    esac

  # Convex generated (regeneratable)
  case "$rel" in
    *"/convex/_generated/"*) return 0 ;;
    esac

  # Dirs we don't need for "understand and advise"
  case "$rel" in
    *"/.cursor/"*) return 0 ;;
    *"/.git/"*) return 0 ;;
    *"/.wrangler/"*) return 0 ;;
    *"/.vscode/"*) return 0 ;;
    esac

  # Website: static assets and content (fonts, blog, kb) — structure is in pages/layouts
  case "$rel" in
    apps/website/public/*) return 0 ;;
    apps/website/src/content/*) return 0 ;;
    esac

  # Web app: public assets (favicon etc.) — not needed for advice
  case "$rel" in
    apps/openagents.com/public/*) return 0 ;;
    esac

  # Optional: skip long dev/log docs
  case "$rel" in
    apps/website/docs/init-log.md) return 0 ;;
    esac

  return 1
}

# Use tracked files only (implies gitignore is respected).
git ls-files apps/web apps/website | LC_ALL=C sort | while IFS= read -r rel; do
  [[ -z "$rel" ]] && continue
  should_skip "$rel" && continue
  append_file "$repo_root/$rel"
done

# Copy to clipboard (macOS pbcopy; fall back to common linux tools).
if command -v pbcopy >/dev/null 2>&1; then
  pbcopy <"$tmp_file"
elif command -v wl-copy >/dev/null 2>&1; then
  wl-copy <"$tmp_file"
elif command -v xclip >/dev/null 2>&1; then
  xclip -selection clipboard <"$tmp_file"
elif command -v xsel >/dev/null 2>&1; then
  xsel --clipboard --input <"$tmp_file"
else
  echo "No clipboard tool found (pbcopy, wl-copy, xclip, xsel)." >&2
  exit 1
fi

bytes="$(wc -c <"$tmp_file" | tr -d ' ')"
echo "Copied openagents.com + website bundle to clipboard (${bytes} bytes)."
