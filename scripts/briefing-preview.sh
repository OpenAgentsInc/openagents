#!/usr/bin/env bash
# Fetch issue metadata, then run the separate read-only briefing experiment.
set -euo pipefail
usage() {
  cat <<'HELP'
Usage: briefing-preview.sh --repo LOCAL_PATH --rev COMMIT_OR_REF
       (--issue-file JSON | --issue URL_OR_NUMBER [--github-repo OWNER/REPO])
       [--output-dir OUTSIDE_REPO] [--index INDEX_JSON]
       [--no-lexical] [--no-symbols] [--no-history] [--syntax]
       [--execution --manifest PACKAGE/Cargo.toml --environment-id LABEL]
       [--require-tool NAME] [--require-file PATH] [--attempt-dir RUN_DIR]

Set BRIEFING_LAB_BIN to a built briefing-lab binary, or put it on PATH.
Build and indexing are separate from warm preview time. No AI calls are made.
HELP
}
local_repo= revision= issue_file= issue= github_repo= output_dir= index_file=
components=()
index_options=()
while (($#)); do
  case "$1" in
    --help|-h) usage; exit 0 ;;
    --repo|--rev|--issue-file|--issue|--github-repo|--output-dir|--index)
      (($# >= 2)) || { echo "Missing value for $1" >&2; exit 2; }
      case "$1" in
        --repo) local_repo=$2 ;; --rev) revision=$2 ;; --issue-file) issue_file=$2 ;;
        --issue) issue=$2 ;; --github-repo) github_repo=$2 ;;
        --output-dir) output_dir=$2 ;; --index) index_file=$2 ;;
      esac
      shift 2 ;;
    --syntax) components+=("$1"); index_options+=("$1"); shift ;;
    --execution) components+=("$1"); shift ;;
    --manifest|--environment-id|--require-tool|--require-file|--attempt-dir)
      (($# >= 2)) || { echo "Missing value for $1" >&2; exit 2; }
      components+=("$1" "$2"); shift 2 ;;
    --no-lexical|--no-symbols|--no-history) components+=("$1"); shift ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done
[[ -n "$local_repo" && -n "$revision" ]] || { usage >&2; exit 2; }
[[ -z "$issue_file" || -z "$issue" ]] || { echo "Choose --issue-file or --issue." >&2; exit 2; }
[[ -n "$issue_file" || -n "$issue" ]] || { usage >&2; exit 2; }
briefing_binary=${BRIEFING_LAB_BIN:-briefing-lab}
command -v "$briefing_binary" >/dev/null 2>&1 || {
  echo "Build briefing-lab first: cargo build -p briefing-lab --release" >&2
  echo "Set BRIEFING_LAB_BIN to the resulting binary. Build time is separate." >&2
  exit 2
}
[[ -n "$output_dir" ]] || output_dir=$(mktemp -d "${TMPDIR:-/tmp}/briefing-preview.XXXXXXXX")
# Index construction validates its artifact destination. Preview validates both outputs.
[[ -n "$index_file" ]] || index_file="$output_dir/index.json"
if [[ ! -f "$index_file" ]]; then
  "$briefing_binary" index --repo "$local_repo" --rev "$revision" --output "$index_file" ${index_options[@]+"${index_options[@]}"}
fi
if [[ -z "$issue_file" ]]; then
  issue_file=$(mktemp "${TMPDIR:-/tmp}/briefing-issue.XXXXXXXX")
  trap 'rm -f "$issue_file"' EXIT
  echo "Fetching issue metadata through gh; network time is separate from local assembly." >&2
  TIMEFORMAT="GitHub fetch wall: %3R seconds"
  if [[ "$issue" =~ ^[0-9]+$ ]]; then
    [[ "$github_repo" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || { echo "Issue numbers require --github-repo OWNER/REPO." >&2; exit 2; }
    time gh issue view "$issue" --repo "$github_repo" --json number,title,body,url > "$issue_file"
  elif [[ "$issue" =~ ^https://github.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/issues/[0-9]+/?$ ]]; then
    time gh issue view "$issue" --json number,title,body,url > "$issue_file"
  else
    echo "Expected a GitHub issue URL or a positive issue number." >&2; exit 2
  fi
fi
"$briefing_binary" preview --repo "$local_repo" --rev "$revision" --index "$index_file" \
  --issue-file "$issue_file" --output-dir "$output_dir" ${components[@]+"${components[@]}"}
