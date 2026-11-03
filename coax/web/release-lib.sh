# Firm reason: 5 (a thin wrapper: the release lock as one gcloud object, and the compiler cache around coax/provenance.sh)
# Shared by release.sh, after-release.sh and pin.sh (sourced).
#
# The release lock: one object in the production bucket, gs://BUCKET/release.lock,
# created only if absent (--if-generation-match=0). One writer of the production
# bucket at a time across every machine and agent, not only this Mac. It names
# its holder (host, pid, who, what, when). A holder whose process is gone on this
# host, or whose lock is older than 90 minutes, is stale and taken over.
prod_bucket=openagentsgemini-openagents-web
lock_url="gs://$prod_bucket/release.lock"
lock_gen=""
export CLOUDSDK_CONFIG="${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}"

lock_age() {   # seconds since the lock object was written
  python3 -I -c 'import sys, datetime as d; t = d.datetime.fromisoformat(sys.argv[1].replace("Z", "+00:00")); print(int((d.datetime.now(d.timezone.utc) - t).total_seconds()))' "$1"
}

take_lock() {   # take_lock WHAT
  local me holder meta gen when host pid said=""
  me="$(hostname -s) $$ $(whoami) $1 $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  while :; do
    if printf '%s\n' "$me" | gcloud storage cp - "$lock_url" --if-generation-match=0 --quiet > /dev/null 2>&1; then
      lock_gen="$(gcloud storage objects describe "$lock_url" --format='value(generation)' 2>/dev/null)"
      return 0
    fi
    meta="$(gcloud storage objects describe "$lock_url" --format='value(generation,update_time)' 2>/dev/null)" || continue
    gen="$(awk '{print $1}' <<< "$meta")"; when="$(awk '{print $2}' <<< "$meta")"
    holder="$(gcloud storage cat "$lock_url" 2>/dev/null || true)"
    host="$(awk '{print $1}' <<< "$holder")"; pid="$(awk '{print $2}' <<< "$holder")"
    if { [[ "$host" == "$(hostname -s)" && -n "$pid" ]] && ! kill -0 "$pid" 2>/dev/null; } || (( $(lock_age "$when") > 5400 )); then
      echo "release lock: taking over a stale lock ($holder)" >&2
      gcloud storage rm "$lock_url" --if-generation-match="$gen" --quiet > /dev/null 2>&1 || true
      continue
    fi
    [[ "$said" == "$holder" ]] || { echo "release lock: waiting for $holder" >&2; said="$holder"; }
    sleep 20
  done
}

drop_lock() {
  [[ -n "$lock_gen" ]] || return 0
  gcloud storage rm "$lock_url" --if-generation-match="$lock_gen" --quiet > /dev/null 2>&1 || true
  lock_gen=""
}

# The checked compiler for a tree, cached by its compiler inputs' fingerprint
# (~/.cache/coax-release-fast/FINGERPRINT); bootstrapped without mutation gates
# when no checked build exists.
compiler_env() {
  env -u COAX_HOST_TARGET -u SDKROOT -u MACOSX_DEPLOYMENT_TARGET -u IPHONEOS_DEPLOYMENT_TARGET -u DEVELOPER_DIR \
    -u CPATH -u C_INCLUDE_PATH -u CPLUS_INCLUDE_PATH -u LIBRARY_PATH \
    CC="${COAX_RELEASE_CC:-cc}" CXX="${COAX_RELEASE_CXX:-c++}" "$@"
}
cache_root="${COAX_RELEASE_CACHE:-$HOME/.cache/coax-release-fast}"
compiler() {   # compiler TREE: installs TREE/coax/bin/coax
  local root="$1" fingerprint cache into checkout
  checkout="${repo:-}"   # the checkout release.sh runs from: its own build may already be checked
  fingerprint="$(compiler_env "$root/coax/provenance.sh" fingerprint "$root")" || return 1
  cache="$cache_root/$fingerprint"
  install_from() { [[ -d "$1" ]] && compiler_env "$root/coax/provenance.sh" install "$root" "$1" 2> /dev/null; }
  keep() {
    into="$(mktemp -d "$cache_root/.keep.XXXXXXXX")"
    cp "$1/coax" "$1/provenance-check" "$1/provenance.record" "$into/"
    for n in lexer.c lexer.h parser.c parser.h checker.c checker.h sources.c sources.h proto_runtime.inc proto_importer.c proto_importer.h; do
      [[ ! -f "$1/$n" ]] || cp "$1/$n" "$into/"
    done
    compiler_env "$root/coax/provenance.sh" verify "$root" "$into/coax" "$into/provenance-check" "$into/provenance.record" || { rm -rf "$into"; return 1; }
    rm -rf "$cache"; mv "$into" "$cache"
  }
  mkdir -p "$cache_root"
  if install_from "$cache"; then echo "  compiler   cached build ${fingerprint:0:12}"; return; fi
  if [[ -n "$checkout" ]] && install_from "$checkout/coax/bin"; then keep "$root/coax/bin"; echo "  compiler   the checkout's build ${fingerprint:0:12}, now cached"; return; fi
  echo "  compiler   no checked build for ${fingerprint:0:12}; bootstrapping one (coax/build.sh, no mutation gates, a few minutes)"
  if ! compiler_env COAX_RELEASE_CACHE="$cache_root" COAX_MUTATION=0 "$root/coax/build.sh" > "$cache_root/$fingerprint.log" 2>&1 ||
     ! { install_from "$cache" || { keep "$root/coax/bin" && install_from "$cache"; }; }; then
    echo "release: coax/build.sh failed (log: $cache_root/$fingerprint.log)" >&2
    return 1
  fi
}
