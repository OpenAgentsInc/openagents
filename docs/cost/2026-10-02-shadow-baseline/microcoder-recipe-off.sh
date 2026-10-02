#!/run/current-system/sw/bin/bash
# Study shim for #10209: the shipped launchers clear OPENAGENTS_DELEGATE_RECIPE,
# so this controller starts the same owner process the --detach launch would,
# with the variable set. Only for the recipe-off arm.
G=""; S=""
while [ $# -gt 0 ]; do case "$1" in --grant) G="$2"; shift 2;; --store) S="$2"; shift 2;; *) shift;; esac; done
id=repository-launch-shadow-$$-$(/run/current-system/sw/bin/date +%s%N)
/run/current-system/sw/bin/cp "$G" "$S/$id.grant.json"
OPENAGENTS_DELEGATE_RECIPE=off /run/current-system/sw/bin/setsid /home/christopherdavid/coder-runner/microcoder repository --store "$S" --grant "$S/$id.grant.json" </dev/null >"$S/$id.jsonl" 2>&1 &
pid=$!
dig=$(/run/current-system/sw/bin/sha256sum "$G" | /run/current-system/sw/bin/cut -d" " -f1)
echo "{\"owner_process\":$pid,\"admission\":\"pending\",\"grant_digest\":\"sha256:$dig\",\"diagnostic_path\":\"$S/$id.jsonl\"}"
