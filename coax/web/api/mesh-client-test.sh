#!/usr/bin/env bash
# Independent command/credential-lifetime test; no network or real key.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
work=$(mktemp -d); trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin" "$work/scratch"
secp=$(brew --prefix secp256k1); ssl=$(brew --prefix openssl@3)
/usr/bin/clang -std=c11 -O1 -g -Wall -Wextra -Werror -D_DEFAULT_SOURCE -fsanitize=address,undefined \
  -I"$secp/include" -I"$ssl/include" "$here/sign.c" -L"$secp/lib" -lsecp256k1 -L"$ssl/lib" -lcrypto -o "$work/sign"
export OA_MESH_SIGNER="$work/sign" TMPDIR="$work/scratch/"
cat > "$work/bin/curl" <<'PY'
#!/usr/bin/env python3
import base64, hashlib, json, os, pathlib, sys
args = sys.argv[1:]
assert args[-1] == os.environ["EXPECTED_ORIGIN"] + "/api/v1/systemone"
assert args[args.index("--proto") + 1] == "=https"
assert "--location" not in args
headers = [args[i + 1] for i, arg in enumerate(args) if arg == "--header"]
auth = next(value for value in headers if value.startswith("Authorization: Nostr "))
event = json.loads(base64.b64decode(auth.split(" ", 2)[2]))
body = pathlib.Path(args[args.index("--data-binary") + 1][1:]).read_bytes()
tags = dict(event["tags"])
assert tags["u"] == args[-1] and tags["method"] == "POST"
assert tags["payload"] == hashlib.sha256(body).hexdigest()
assert event["kind"] == 27235 and len(event["sig"]) == 128
for key in pathlib.Path(os.environ["TMPDIR"]).rglob("*.key"):
    assert key.stat().st_mode & 0o077 == 0
pathlib.Path(args[args.index("--output") + 1]).write_text('{"test":true}')
print("200", end="")
PY
chmod +x "$work/bin/curl"
export PATH="$work/bin:$PATH"
printf '%s' '{"model":"elephant-1"}' > "$work/body"
export EXPECTED_ORIGIN=https://openagents.com
"$here/mesh-client.sh" --new-demo-key "$work/body" > "$work/result"
[[ $(cat "$work/result") == '{"test":true}' ]]
[[ -z $(find "$work/scratch" -type f -name '*.key') ]]
export EXPECTED_ORIGIN=https://staging.openagents.com
"$here/mesh-client.sh" --staging --new-demo-key "$work/body" >/dev/null
"$work/sign" --new "$work/existing.key" >/dev/null
"$here/mesh-client.sh" --staging --key "$work/existing.key" "$work/body" >/dev/null
[[ -f "$work/existing.key" ]]
if "$here/mesh-client.sh" --new-demo-key --key "$work/existing.key" "$work/body" >/dev/null 2>&1; then exit 1; fi
if "$here/mesh-client.sh" --url https://evil.invalid --new-demo-key "$work/body" >/dev/null 2>&1; then exit 1; fi
if "$here/mesh-client.sh" "$work/body" >/dev/null 2>&1; then exit 1; fi
echo 'mesh client: fixed origins, existing NIP-98 format, body binding, key lifetime and bad arguments passed'
