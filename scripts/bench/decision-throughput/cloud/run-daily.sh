#!/bin/sh
# The daily decision-throughput run (Cloud Run job decision-throughput-daily,
# docs/inference/decision-throughput.md): the cheap backends on the `daily`
# plan, each line appended to gs://$BUCKET/runs/YYYY-MM-DD.jsonl, then the
# doc's tables regenerated from every day in GCS and pushed to main.
#
# Env: TYPESAFE_API_KEY (secret), GH_TOKEN (secret; optional: no push
# without it), DT_TARGETS (default below), DT_DURATION (seconds per point).
set -u
BUCKET=${DT_BUCKET:-openagentsgemini-decision-throughput}
G=gs://$BUCKET
TARGETS=${DT_TARGETS:-"jev gateway-jev gateway-pylon vertex-gemini sealed-cpu"}
cd /app
for t in $TARGETS; do
    echo "== $t"
    python3 harness.py --target "$t" --plan "${DT_PLAN:-daily}" --corpus "$G/corpus/v1" \
        --out /tmp/daily.jsonl --gcs "$G/runs" --duration "${DT_DURATION:-120}" --note scheduled \
        || echo "$t failed ($?)"
done
[ -n "${GH_TOKEN:-}" ] || { echo "no GH_TOKEN: tables not pushed"; exit 0; }
repo=/tmp/openagents
# The token goes in a header, never in a URL, and git's messages are masked,
# so no error can print it.
tok=$(printf %s "$GH_TOKEN" | tr -d '\r\n')
auth="AUTHORIZATION: basic $(printf 'x-access-token:%s' "$tok" | base64 | tr -d '\n')"
gitq() { git -c http.extraHeader="$auth" "$@" 2>&1 | sed "s/$tok/***/g"; }
gitq clone -q --depth 1 https://github.com/OpenAgentsInc/openagents.git "$repo"
[ -d "$repo/.git" ] || { echo "clone failed"; exit 1; }
git -C "$repo" config user.name "decision-throughput bot"
git -C "$repo" config user.email "bot@openagents.com"
for attempt in 1 2 3 4 5; do
    python3 report.py --gcs "$G/runs" --doc "$repo/docs/inference/decision-throughput.md" || exit 1
    if git -C "$repo" diff --quiet; then echo "tables unchanged"; exit 0; fi
    git -C "$repo" commit -qam "decision-throughput: tables through $(date -u +%Y-%m-%d) (scheduled run)"
    if gitq -C "$repo" push -q origin HEAD:main && [ "$(git -C "$repo" rev-parse HEAD)" = "$(gitq -C "$repo" ls-remote origin refs/heads/main | cut -f1)" ]; then
        echo "pushed"; exit 0
    fi
    gitq -C "$repo" fetch -q --depth 1 origin main && git -C "$repo" reset -q --hard origin/main
done
echo "push failed"; exit 1
