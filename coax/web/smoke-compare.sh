# Smoke comparison for release.sh (sourced). Only what a release breaks may
# stop it: each failure is compared with the baseline (what is live), and a
# failure is only real if it survives a retry. A check that fails is run again
# after a short wait, up to twice (SMOKE_RETRY_WAITS, seconds); one that
# passes on any retry was transient, e.g. a link-preview card briefly empty
# right after the bucket switch (2026-10-31), and never rolls a release back.
fails_of() { sed -n 's/^  FAIL  //p' | sed 's/ ([^)]*)$//' | sort -u; }
new_fails() { comm -23 <(printf '%s\n' "$1" | sed '/^$/d' | sort -u) <(printf '%s\n' "$2" | sed '/^$/d' | sort -u); }
# confirm_fails FAILS SMOKE ARGS...: of FAILS, the checks that fail again on
# every retry. smoke.sh runs as one script; only the named checks are judged.
confirm_fails() {
  local fails="$1" wait again; shift
  for wait in ${SMOKE_RETRY_WAITS:-10 20}; do
    [[ -n "$fails" ]] || break
    echo "  smoke: $(wc -l <<< "$fails" | tr -d ' ') check(s) failed; running again in ${wait} s" >&2
    sleep "$wait"
    again=$("$@" 2>&1 | fails_of) || true
    fails=$(comm -12 <(printf '%s\n' "$fails" | sed '/^$/d' | sort -u) <(printf '%s\n' "$again" | sed '/^$/d' | sort -u))
  done
  printf '%s' "$fails"
}
# broken_by_release OUTPUT BASELINE_FAILS SMOKE ARGS...: the failures this
# release causes, new against the baseline and confirmed by retries.
broken_by_release() {
  local out="$1" base="$2" broke; shift 2
  broke=$(new_fails "$(fails_of <<< "$out")" "$base")
  [[ -z "$broke" ]] || broke=$(confirm_fails "$broke" "$@")
  printf '%s' "$broke"
}
# passes_on_retry CMD...: a whole check that failed (the production --starts
# walk) runs again after the same waits; 0 when any retry passes.
passes_on_retry() {
  local wait
  for wait in ${SMOKE_RETRY_WAITS:-10 20}; do
    echo "  the check failed; running it again in ${wait} s: $*" >&2
    sleep "$wait"
    "$@" && return 0
  done
  return 1
}
