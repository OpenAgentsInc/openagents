#!/usr/bin/env bash
# Show a project's new tasks, claims, plans and notes on openagents.com
# without a release: the data alone, in seconds.
#
#   coax/web/project/publish-data.sh [SLUG...]     # default: every project with a page
#
# The drawn project page (/projects/<slug>) already reads its relay live
# (project-live.coax): a claim, a plan step, a note, a done or a move shows
# there within seconds of `publish.ts`, with no release and no data push.
# What the page's build bakes in goes stale instead: the Markdown agents read
# (/projects/<slug>/index.md), the projects list's (/projects/index.md) and
# their sections of /llms-full.txt. This regenerates those from the relay
# (snapshot.ts, list.ts, in a clean worktree of origin/main so no checkout's
# files change) and writes just them to the staging and production buckets,
# then checks production answers the new snapshot.
#
# The page's first paint (its baked tables in project.wasm) and the commit
# history are code: they move with the next release, which reads the relay
# afresh. A release never needs to run for data. A release promoting at the
# same moment can put its own (minutes older) Markdown back; run this again
# after it if the newest events matter.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"; repo="$(cd "$here/../../.." && pwd)"
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"
buckets="openagentsgemini-openagents-web-staging openagentsgemini-openagents-web"
git -C "$repo" fetch -q origin main
tree="$(mktemp -d)/data"; out="$(mktemp -d)"
trap 'git -C "$repo" worktree remove --force "$tree" 2>/dev/null || true; rm -rf "$out"' EXIT
git -C "$repo" worktree add -q --detach "$tree" origin/main
tools="$tree/coax/web/project" site="$tree/apps/openagents-web/projects"
if [[ -d "$here/node_modules" ]]; then ln -s "$here/node_modules" "$tools/node_modules"; else (cd "$tools" && bun install --silent); fi
slugs=("$@"); (( ${#slugs[@]} )) || read -r -a slugs <<< "$(bun "$tools/baked.ts" --list | tr '\n' ' ')"
for slug in "${slugs[@]}"; do
  [[ "$slug" =~ ^[a-z0-9][a-z0-9-]{0,63}$ && -f "$site/$slug/project.json" ]] || { echo "publish-data: no project $slug" >&2; exit 2; }
  bun "$tools/snapshot.ts" --project "$slug" | sed 's/^/  /'
done
bun "$tools/list.ts" | sed 's/^/  /'

# The files, and each one's section of llms-full.txt (deploy.sh writes it: the
# public pages' Markdown, each after "Source: <its URL>").
files=(projects/index.md); for slug in "${slugs[@]}"; do files+=("projects/$slug/index.md"); done
for b in $buckets; do
  for f in "${files[@]}"; do
    gcloud storage cp "$tree/apps/openagents-web/$f" "gs://$b/$f" --content-type "text/markdown; charset=utf-8" --cache-control no-cache --quiet > /dev/null 2>&1 ||
      { echo "publish-data: couldn't write gs://$b/$f" >&2; exit 1; }
  done
  if gcloud storage cp "gs://$b/llms-full.txt" "$out/llms-full.txt" --quiet > /dev/null 2>&1; then
    bun -e '
      const [file, site, ...files] = process.argv.slice(1), sep = "\n\n---\n\nSource: "
      const text = await Bun.file(file).text(), parts = text.split(sep)
      let changed = 0
      for (let i = 1; i < parts.length; i++) {
        const url = parts[i].slice(0, parts[i].indexOf("\n")), path = url.replace("https://openagents.com/", "") + "/index.md"
        if (files.includes(path)) { parts[i] = `${url}\n\n` + await Bun.file(`${site}/${path}`).text(); changed++ }
      }
      await Bun.write(file, parts.join(sep)); console.log(changed)' "$out/llms-full.txt" "$tree/apps/openagents-web" "${files[@]}" > "$out/n"
    gcloud storage cp "$out/llms-full.txt" "gs://$b/llms-full.txt" --content-type "text/plain; charset=utf-8" --cache-control no-cache --quiet > /dev/null 2>&1 ||
      { echo "publish-data: couldn't write gs://$b/llms-full.txt" >&2; exit 1; }
  fi
  echo "  $b: ${#files[@]} files, $(cat "$out/n" 2>/dev/null || echo 0) llms-full.txt sections"
done

# Production answers what was just written.
for slug in "${slugs[@]}"; do
  want="$(grep -m1 '^Snapshot of the project' "$site/$slug/index.md")"
  got="$(curl -sf -H 'cache-control: no-cache' "https://openagents.com/projects/$slug/index.md" | grep -m1 '^Snapshot of the project' || true)"
  [[ "$got" == "$want" ]] || { echo "publish-data: https://openagents.com/projects/$slug/index.md answers \"$got\", not \"$want\"" >&2; exit 1; }
  at="${want#*taken }"
  echo "publish-data $slug: live on https://openagents.com/projects/$slug/index.md (snapshot ${at%% from*})"
done
