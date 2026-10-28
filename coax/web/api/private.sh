#!/usr/bin/env bash
# A private project from this machine, for its members and their agents
# (docs/private-projects.md). Every request is NIP-98 signed with a Nostr key
# kept in a file (64 hex digits, chmod 600; sign.c; the key never leaves it).
#
#   private.sh push FILE                         a member pushes the project's document (openagents.private-project.v1)
#   private.sh get SLUG                          the state as Markdown: goals, today, checklist, activity, messages
#   private.sh note SLUG TEXT --by NAME          a note on the page's AGENTS panel
#   private.sh plan SLUG FILE --by NAME          a plan (the file's text)
#   private.sh status SLUG ID todo|doing|done [--note TEXT] --by NAME   a milestone's or checklist item's status
#   private.sh inbox SLUG                        members' messages nobody has answered
#   private.sh reply SLUG ID TEXT --by NAME      answer one
#   private.sh message SLUG TEXT --by NAME       a new message on the thread
#   private.sh invite SLUG PUBKEY                a one-time sign-in link for that member (24 hours)
#   private.sh agent SLUG PUBKEY NAME [--remove] register (or remove) an agent key
#   private.sh open SLUG                         sign yourself in and open the page in the default browser
#   private.sh pubkey                            the key's public key
#   private.sh new-key FILE                      a new key in FILE (chmod 600); prints only its public key
#
# --staging talks to staging.openagents.com. --key FILE (or OA_PRIVATE_KEY)
# picks the key: members' commands (push, invite, agent, open) default to the
# owner's member key, the rest to the agent key.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
origin=https://openagents.com key="${OA_PRIVATE_KEY:-}" by="" note="" remove=false
args=()
while (($#)); do
  case "$1" in
    --staging) origin=https://staging.openagents.com; shift ;;
    --key) key="$2"; shift 2 ;;
    --by) by="$2"; shift 2 ;;
    --note) note="$2"; shift 2 ;;
    --remove) remove=true; shift ;;
    *) args+=("$1"); shift ;;
  esac
done
set -- ${args[@]+"${args[@]}"}
cmd="${1:-}"; shift || true
case "$cmd" in
  push|invite|agent|open) key="${key:-$HOME/work/.secrets/openagents-analytics-admin.key}" ;;
  *) key="${key:-$HOME/work/.secrets/openagents-agent-private.key}" ;;
esac
sign="${XDG_CACHE_HOME:-$HOME/.cache}/oa-admin/sign"
if [[ ! -x "$sign" || "$here/sign.c" -nt "$sign" ]]; then
  mkdir -p "$(dirname "$sign")"
  secp="$(brew --prefix secp256k1)"; ssl="$(brew --prefix openssl@3)"
  /usr/bin/clang -std=c11 -O2 -Wall -Wextra -Werror -D_DEFAULT_SOURCE -I"$secp/include" -I"$ssl/include" \
    -o "$sign" "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto
fi
if [[ "$cmd" == new-key ]]; then "$sign" --new "${1:?new-key FILE}"; exit; fi
[[ -f "$key" ]] || { echo "private.sh: no key at $key" >&2; exit 1; }
tmp="$(mktemp -d)"; trap 'rm -rf "$tmp"' EXIT

# call METHOD SLUG OP [JSON]: the answer's body on stdout; a refusal on stderr and exit 1.
call() {
  local url="$origin/api/private/$2/$3" auth status
  if [[ $# -ge 4 ]]; then
    printf '%s' "$4" > "$tmp/body"
    auth="$("$sign" --key "$key" "$1" "$url" "$tmp/body")"
    status="$(curl -sS -o "$tmp/out" -w '%{http_code}' -X "$1" -H "Authorization: Nostr $auth" -H 'Content-Type: application/json' --data-binary @"$tmp/body" "$url")"
  else
    auth="$("$sign" --key "$key" "$1" "$url")"
    status="$(curl -sS -o "$tmp/out" -w '%{http_code}' -H "Authorization: Nostr $auth" "$url")"
  fi
  if [[ "$status" != 200 ]]; then
    if [[ "$status" == 404 ]]; then echo "private.sh: refused: no such project, or this key isn't one of its members or agents" >&2
    else echo "private.sh: $status $(cat "$tmp/out")" >&2; fi
    return 1
  fi
  cat "$tmp/out"
}
json() { python3 -I -c 'import json, sys; print(json.dumps(dict(zip(sys.argv[1::2], sys.argv[2::2]))))' "$@"; }
need_by() { [[ -n "$by" ]] || { echo "private.sh: say who you are with --by NAME" >&2; exit 2; }; }
link() {  # SLUG PUBKEY: a sign-in link; the token goes in the fragment, which never reaches a server or a log
  call POST "$1" invite "$(json pubkey "$2")" > "$tmp/ans" || exit 1
  python3 -I -c '
import json, sys
a = json.load(sys.stdin)
print(f"{sys.argv[1]}/projects/{sys.argv[2]}#invite={a["token"]}")
print(f"good once, until {a["expires"]}", file=sys.stderr)' "$origin" "$1" < "$tmp/ans"
}

case "$cmd" in
  pubkey) "$sign" --key "$key" --pubkey ;;
  push)
    file="${1:?push FILE}"
    slug="$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["slug"])' "$file")"
    base=0
    if call GET "$slug" state > "$tmp/ans" 2>/dev/null; then base="$(python3 -I -c 'import json, sys; print(json.load(sys.stdin)["version"])' < "$tmp/ans")"; fi
    python3 -I -c 'import json, sys; print(json.dumps({"base": int(sys.argv[2]), "doc": json.load(open(sys.argv[1]))}))' "$file" "$base" > "$tmp/push"
    call POST "$slug" push "$(cat "$tmp/push")" > "$tmp/ans" || exit 1
    python3 -I -c 'import json, sys; print("pushed: version", json.load(sys.stdin)["version"])' < "$tmp/ans" ;;
  get)
    call GET "${1:?get SLUG}" state > "$tmp/ans" || exit 1
    python3 -I -c '
import json, sys
s = json.load(sys.stdin); d = s["doc"]
print(f"# {d["title"]} (version {s["version"]}, {s["updated_at"]} Chicago by {s["updated_by"]})\n")
print(f"You: {s["you"]["name"]} ({s["you"]["role"]}). Today in Chicago: {s["today"]}.\n")
if d.get("rules"):
    print("## Rules\n"); [print(f"- {r}") for r in d["rules"]]; print()
print("## Goals\n")
for g in d.get("goals", []):
    who = "agreed by " + ", ".join(g.get("agreed_by", [])) if g["status"] == "agreed" else "PROPOSED"
    print(f"- `{g["id"]}` {g["text"]} ({who})")
days = sorted(d.get("days", []), key=lambda x: x["date"])
today = next((x for x in days if x["date"] == s["today"]), None) or next((x for x in days if x["date"] > s["today"]), days[-1] if days else None)
for day in days:
    mark = " (today)" if day["date"] == s["today"] else ""
    if day is not today and day["date"] < s["today"] and all(m["status"] == "done" for m in day["milestones"]): continue
    print(f"\n## Milestones {day["date"]}{mark}\n")
    for m in day["milestones"]:
        print(f"- [{ {"done": "x", "doing": "~", "todo": " "}[m["status"]] }] `{m["id"]}` {m["text"]} ({m.get("owner", "")}, {m["status"]})" + (f" -- {m["note"]}" if m.get("note") else ""))
for c in d.get("checklists", []):
    print(f"\n## Checklist: {c["title"]}\n")
    for i in c["items"]:
        print(f"- [{ {"done": "x", "doing": "~", "todo": " "}[i["status"]] }] `{i["id"]}` {i["text"]} ({i["status"]})" + (f" -- {i["evidence"]}" if i.get("evidence") else ""))
print("\n## Analytics (today / last 7 days)\n")
for r in s["analytics"]: print(f"- {r["label"]}: {r["today"]} / {r["week"]}")
print("\n## Activity (newest first)\n")
for a in s["activity"][:15]: print(f"- {a["at"]} {a["by"]} ({a["kind"]}): " + a["text"].replace("\n", " / "))
print("\n## Messages\n")
for m in s["messages"]: print(f"- #{m["id"]} {m["at"]} {m["from"]}" + (f" (re #{m["reply_to"]})" if m["reply_to"] else "") + f": {m["text"]}")
if d.get("links"):
    print("\n## Links\n"); [print(f"- [{l["label"]}]({l["url"]})") for l in d["links"]]' < "$tmp/ans" ;;
  note) need_by; call POST "${1:?note SLUG TEXT}" note "$(json kind note text "${2:?TEXT}" by "$by")" >/dev/null && echo "noted" ;;
  plan) need_by; call POST "${1:?plan SLUG FILE}" note "$(json kind plan text "$(cat "${2:?FILE}")" by "$by")" >/dev/null && echo "plan posted" ;;
  status)
    need_by
    body="$(python3 -I -c 'import json, sys; d = {"id": sys.argv[1], "status": sys.argv[2], "by": sys.argv[3]}; d.update({"note": sys.argv[4]} if sys.argv[4] else {}); print(json.dumps(d))' "${2:?ID}" "${3:?STATUS}" "$by" "$note")"
    call POST "${1:?status SLUG ID STATUS}" status "$body" > "$tmp/ans" || exit 1
    python3 -I -c 'import json, sys; print("version", json.load(sys.stdin)["version"])' < "$tmp/ans" ;;
  inbox)
    call GET "${1:?inbox SLUG}" inbox > "$tmp/ans" || exit 1
    python3 -I -c '
import json, sys
m = json.load(sys.stdin)
print("nothing unanswered" if not m else "\n".join(f"#{x["id"]} {x["at"]} {x["from"]}: {x["text"]}" for x in m))' < "$tmp/ans" ;;
  reply)
    need_by
    body="$(python3 -I -c 'import json, sys; print(json.dumps({"reply_to": sys.argv[1], "text": sys.argv[2], "by": sys.argv[3]}))' "${2:?ID}" "${3:?TEXT}" "$by")"
    call POST "${1:?reply SLUG ID TEXT}" message "$body" >/dev/null && echo "replied" ;;
  message) need_by; call POST "${1:?message SLUG TEXT}" message "$(json text "${2:?TEXT}" by "$by")" >/dev/null && echo "sent" ;;
  invite) link "${1:?invite SLUG PUBKEY}" "${2:?PUBKEY}" ;;
  agent)
    role=agent; $remove && role=none
    call POST "${1:?agent SLUG PUBKEY NAME}" agent "$(json pubkey "${2:?PUBKEY}" name "${3:?NAME}" role "$role")" >/dev/null && echo "agent $role: ${3}" ;;
  open) open "$(link "${1:?open SLUG}" "$("$sign" --key "$key" --pubkey)" 2>/dev/null)" ;;
  *) sed -n '2,23p' "$0"; exit 2 ;;
esac
