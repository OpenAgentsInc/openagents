#!/usr/bin/env bash
# Build, stage, check, and pin one Aethyr engine release from one openagents
# commit (docs/verse/aethyrforge-deployment.md, #12590):
#
#   scripts/deploy/aethyr/release.sh build [COMMIT]   # default origin/main
#   scripts/deploy/aethyr/release.sh stage DIR        # images, no-traffic tags
#   scripts/deploy/aethyr/release.sh check DIR        # browser checks on the tags
#   scripts/deploy/aethyr/release.sh pin DIR          # new pins, old ones kept
#
# build checks COMMIT out into a detached worktree under DIR (default
# $AETHYR_RELEASE_ROOT/COMMIT10, AETHYR_RELEASE_ROOT defaulting to
# ~/work/aethyr-releases) and builds everything from it: the host SDK from
# crates/aethyrforge-host with its Kiln companion, kernel.wasm from
# crates/aethyr, then one Vite build per profile from apps/aethyr. Each
# bundle's SDK URL is derived from the SDK built in the same run
# (/sdk/SHA256/aethyrforge_host.js on the release's own editor origin), so
# there is no operator-pinned SDK URL to supply. render.py makes the editor
# and preview container contexts. build.json records every hash, and
# candidates/ holds the pin JSON each profile would publish. No GCP call is
# made, except to read a reused asset set's manifest (below); build runs on
# any machine with the pinned toolchains.
#
# Profiles (AETHYR_PROFILES, default all four):
#   universe  /new-lander/verse map          -> demos/universe.json
#   house     /new-lander/house              -> demos/haunted-house.json and
#                                               demos/runtimes/haunted-house.json
#   planet    personal planets               -> demos/runtimes/personal-planet.json
#   town      /demo/medieval-town            -> demos/medieval-town.json
# Every profile except universe packages its compiled scene closure from
# AETHYR_PRIVATE_SOURCE (default ~/.openagents/verse/private/aethyrforge)
# with package-assets.py. Those files stay under DIR and in the private
# bucket; never in Git.
#
# AETHYR_REUSE_ASSET_SET_HOUSE, _PLANET, and _TOWN (each an asset set
# SHA-256) skip packaging for that profile and reuse an asset set already
# uploaded to the bucket, for a build machine without the private source.
# build reads only that set's assets-manifest.json, as the automation
# account, and checks its identity; no asset file is fetched. stage then
# mounts the same assetsets/SHA256 prefix and refuses if it is missing.
#
# stage uploads each new asset set once to the bucket the live preview
# service already mounts (AETHYR_ASSET_BUCKET overrides), builds each
# context with Cloud Build, and deploys each image by digest to
# aethyrforge-editor and aethyrforge-preview with --use-http2 --no-traffic
# and the release's fresh tag. The preview mounts only its asset set,
# read-only, at /runtime. It refuses a tag that either service already has.
#
# check runs scripts/check-haunted-house.mjs, scripts/check-medieval-town.mjs
# and the scripts/check-verse-*.mjs entry checks in candidate mode against
# the tag URLs, each in its own browser through `openagents browser run`.
# AETHYR_CHECK_REF=REF takes the website consumer and the check scripts from
# a later commit (a detached worktree at DIR/check-src) instead of the
# release's own, for checking these engine tags against the website that
# will serve them; checks.json records that commit.
# AETHYR_CHECK_CDP=http://127.0.0.1:PORT runs them in an existing
# WebGPU browser instead (a machine whose own Chrome has no GPU), clearing
# its cookies, cache, and release-origin storage before each check.
# It builds the native website consumer from DIR's worktree and serves the
# candidate pins with it, unless AETHYRFORGE_HOST_RESPONSE_URL names a
# website candidate already serving them.
#
# pin copies the current pins to crates/openagents-web/content/demos/rollback/
# and writes the candidates in their place in this checkout, then writes the
# qualification entry docs/verse/aethyr-releases/COMMIT10.json. It commits
# nothing. Ship the pins with scripts/deploy/web.sh like any website change.
#
# Reads and writes run as the automation account (CLOUDSDK_CONFIG, default
# ~/work/.secrets/gcloud-sa-config), with no fallback to a person's gcloud
# login (docs/deployment/automation-account.md). DRY_RUN=1 prints every
# gcloud write instead of making it; reads still run, and a refused read is
# replaced by a placeholder.
set -euo pipefail

PROJECT=openagentsgemini
REGION=us-central1
REPO=us-central1-docker.pkg.dev/$PROJECT/openagents
RUN_HASH=ezxz4mgdsq-uc
HOST_ORIGIN=https://openagents.com
BUILD_SA=projects/$PROJECT/serviceAccounts/oa-mvp-automation@$PROJECT.iam.gserviceaccount.com
SA_CONFIG=${CLOUDSDK_CONFIG:-$HOME/work/.secrets/gcloud-sa-config}
PROTOCOL_VERSION=8
ROOT=$(git rev-parse --show-toplevel)
HERE=$ROOT/scripts/deploy/aethyr

say() { printf '%s\n' "$*" >&2; }
die() { say "$*"; exit 1; }
sha() { sha256sum "$1" | awk '{print $1}'; }

g() { CLOUDSDK_CONFIG=$SA_CONFIG gcloud "$@"; }

# A write, as the automation account only (#12058).
apply() {
    if [ -n "${DRY_RUN:-}" ]; then say "  DRY_RUN: gcloud $*"; return 0; fi
    local log
    log=$(mktemp)
    if g "$@" > "$log" 2>&1; then cat "$log" >&2; rm -f "$log"; return 0; fi
    cat "$log" >&2
    if grep -qi 'actAs\|PERMISSION_DENIED\|does not have permission' "$log"; then
        say "The automation account was refused (above). Grant it the missing role; see docs/deployment/automation-account.md."
    fi
    rm -f "$log"
    return 1
}

# A read whose answer a dry run can stand in for.
read_or() { # PLACEHOLDER gcloud-args...
    local placeholder=$1
    shift
    if g "$@" 2> /dev/null; then return 0; fi
    if [ -n "${DRY_RUN:-}" ]; then printf '%s\n' "$placeholder"; return 0; fi
    die "gcloud $* failed as the automation account."
}

require_automation_account() {
    if [ -n "${DRY_RUN:-}" ]; then return 0; fi
    [ -d "$SA_CONFIG" ] || die "No automation gcloud configuration at $SA_CONFIG (set CLOUDSDK_CONFIG)."
    local account
    account=$(g auth list --filter=status:ACTIVE --format='value(account)' 2> /dev/null || true)
    case "$account" in
        *.iam.gserviceaccount.com) say "  gcloud account: $account" ;;
        *) die "CLOUDSDK_CONFIG=$SA_CONFIG is not signed in as a service account (got '${account:-nothing}'). No personal login is used." ;;
    esac
}

profiles() { printf '%s\n' "${AETHYR_PROFILES:-universe house planet town}" | tr ' ' '\n' | grep .; }
letter() { case "$1" in universe) echo u ;; house) echo h ;; planet) echo p ;; town) echo m ;; *) die "Unknown profile: $1" ;; esac; }
asset_profile() { case "$1" in universe) echo universe ;; house) echo haunted-house ;; planet) echo personal-planet ;; town) echo medieval-town ;; esac; }
editor_url() { echo "https://$1---aethyrforge-editor-$RUN_HASH.a.run.app"; }
preview_url() { echo "https://$1---aethyrforge-preview-$RUN_HASH.a.run.app"; }

need_dir() {
    [ -n "${1:-}" ] && [ -f "$1/build.json" ] || die "Name a release directory that build wrote (it holds build.json)."
    DIR=$(cd "$1" && pwd -P)
    SRC=$DIR/src
}

field() { python3 -c 'import json,sys
v=json.load(open(sys.argv[1]))
for k in sys.argv[2].split("."): v=v[k]
print(v)' "$1" "$2"; }

# Hash of a directory: SHA-256 over its sorted "sha256  path" lines.
tree_hash() { (cd "$1" && find . -type f -print0 | LC_ALL=C sort -z | xargs -0 sha256sum) | sha256sum | awk '{print $1}'; }

# --- build -----------------------------------------------------------------

check_tools() {
    command -v node > /dev/null || die "Install Node 22."
    command -v npm > /dev/null || die "Install npm."
    rustup toolchain list | grep -q '^1\.98\.1' || die "Install Rust 1.98.1 with the wasm32-unknown-unknown target (crates/aethyr/rust-toolchain.toml)."
    local locked installed
    locked=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/version = |"/, ""); print; exit }' "$SRC/Cargo.lock")
    installed=$(wasm-bindgen --version 2> /dev/null | awk '{print $2}' || true)
    [ "$installed" = "$locked" ] || die "Install wasm-bindgen-cli $locked to match Cargo.lock."
    "${AETHYRFORGE_WASM_OPT:-wasm-opt}" --version 2> /dev/null | grep -q '^wasm-opt version 132\b' || die "Put Binaryen 132's wasm-opt on PATH or set AETHYRFORGE_WASM_OPT."
    if [ -z "${CC_wasm32_unknown_unknown:-}" ]; then
        local candidate
        for candidate in /opt/homebrew/opt/llvm/bin/clang /usr/local/opt/llvm/bin/clang "$(command -v clang || true)"; do
            if [ -x "$candidate" ] && "$candidate" --print-targets 2> /dev/null | grep -q wasm32; then
                CC_wasm32_unknown_unknown=$candidate
                break
            fi
        done
        [ -n "${CC_wasm32_unknown_unknown:-}" ] || die "Kiln needs a clang with the WebAssembly target; install LLVM or set CC_wasm32_unknown_unknown."
        AR_wasm32_unknown_unknown=$(dirname "$CC_wasm32_unknown_unknown")/llvm-ar
        [ -x "$AR_wasm32_unknown_unknown" ] || AR_wasm32_unknown_unknown=$(command -v llvm-ar)
        export CC_wasm32_unknown_unknown AR_wasm32_unknown_unknown
    fi
}

# The asset set a profile reuses from the bucket instead of packaging, if any.
reused_asset_set() { # PROFILE
    local name value
    name=AETHYR_REUSE_ASSET_SET_$(printf '%s' "$1" | tr '[:lower:]' '[:upper:]')
    value=${!name:-}
    [ -z "$value" ] || [ "$1" != universe ] || die "The universe has no asset set to reuse."
    [ -z "$value" ] || [[ $value =~ ^[0-9a-f]{64}$ ]] || die "$name must be an asset set SHA-256."
    printf '%s\n' "$value"
}

# Reads an uploaded asset set's manifest (read-only) and checks that it names
# that set. Only the manifest is fetched; the asset files stay in the bucket.
fetch_asset_manifest() { # SHA256 ASSETS_DIR
    local set=$1 assets=$2 bucket
    bucket=$(asset_bucket)
    mkdir -p "$assets/private-assets"
    g storage cat "gs://$bucket/assetsets/$set/assets-manifest.json" > "$assets/assets-manifest.json" ||
        die "No asset set $set in gs://$bucket (read as the automation account)."
    python3 - "$assets/assets-manifest.json" "$set" <<'PY' || die "gs://$bucket/assetsets/$set/assets-manifest.json does not describe $set."
import hashlib, json, sys
m = json.load(open(sys.argv[1]))
body = json.dumps(m['files'], sort_keys=True, separators=(',', ':')).encode()
ok = m.get('schema') == 'aethyrforge.browser-assets.v1' and m['sha256'] == sys.argv[2] == hashlib.sha256(body).hexdigest()
sys.exit(not ok)
PY
    printf 'gs://%s/assetsets/%s\n' "$bucket" "$set" > "$assets/reused-from"
    say "    reusing asset set $set from gs://$bucket ($(python3 -c 'import json,sys; print(len(json.load(open(sys.argv[1]))["files"]))' "$assets/assets-manifest.json") files)"
}

build() {
    local ref=${1:-origin/main} commit short
    commit=$(git -C "$ROOT" rev-parse --verify "$ref^{commit}") || die "Unknown commit: $ref"
    short=${commit:0:10}
    if [ -z "$(git -C "$ROOT" branch -r --contains "$commit" 2> /dev/null)" ]; then
        die "Push $short first: a release is built only from a pushed commit."
    fi
    if ! git -C "$ROOT" merge-base --is-ancestor "$commit" origin/main 2> /dev/null; then
        [ "${AETHYR_ALLOW_BRANCH:-}" = 1 ] || die "$short is not on origin/main. AETHYR_ALLOW_BRANCH=1 builds a pushed branch commit for a candidate that is never pinned."
        say "  $short is not on origin/main; building a branch candidate."
    fi
    DIR=${AETHYR_RELEASE_DIR:-${AETHYR_RELEASE_ROOT:-$HOME/work/aethyr-releases}/$short${AETHYR_RELEASE_SUFFIX:-}}
    SRC=$DIR/src
    mkdir -p "$DIR"
    DIR=$(cd "$DIR" && pwd -P)
    SRC=$DIR/src
    case "$DIR/" in "$ROOT"/*) die "Keep the release directory outside the checkout: $DIR" ;; esac
    if [ -d "$SRC" ]; then
        [ "$(git -C "$SRC" rev-parse HEAD)" = "$commit" ] || die "$SRC holds another commit; use a new AETHYR_RELEASE_DIR."
        [ -z "$(git -C "$SRC" status --porcelain --ignored --untracked-files=all -- . ':!apps/aethyr/node_modules' ':!apps/aethyr/public/kernel.wasm')" ] ||
            die "$SRC has changes; remove it to rebuild from a clean checkout."
    else
        git -C "$ROOT" worktree add --quiet --detach "$SRC" "$commit"
    fi
    check_tools
    say "build $short in $DIR"

    # The host SDK, from crates/aethyrforge-host at this commit.
    : "${CARGO_TARGET_DIR:=$HOME/work/openagents-target-1}"
    export CARGO_TARGET_DIR
    rm -rf "$DIR/sdk"
    AETHYRFORGE_BUILD_KILN=1 sh "$SRC/scripts/build-aethyrforge-host.sh" "$DIR/sdk" >&2
    local sdk_sha
    sdk_sha=$(sha "$DIR/sdk/aethyrforge_host_bg.wasm")
    say "  host SDK $sdk_sha"

    # The kernel, from crates/aethyr, with its own toolchain and target.
    (
        cd "$SRC/apps/aethyr"
        unset RUSTC CARGO_BUILD_BUILD_DIR
        npm ci --no-audit --no-fund >&2
        # crates/aethyr is its own workspace with its own shared target.
        CARGO_TARGET_DIR=${AETHYR_CARGO_TARGET_DIR:-$HOME/work/aethyr-target} bash tools/build-kernel.sh >&2
        npx tsc --noEmit >&2
    )
    local kernel_sha
    kernel_sha=$(sha "$SRC/apps/aethyr/public/kernel.wasm")
    say "  kernel.wasm $kernel_sha"

    local source=${AETHYR_PRIVATE_SOURCE:-$HOME/.openagents/verse/private/aethyrforge}
    local p release editor preview assets asset_set reuse
    rm -rf "$DIR/candidates"
    mkdir -p "$DIR/candidates"
    for p in $(profiles); do
        release=a$(letter "$p")$short${AETHYR_RELEASE_SUFFIX:-}
        editor=$(editor_url "$release")
        preview=$(preview_url "$release")
        say "  $p: release $release"
        rm -rf "${DIR:?}/$p"
        mkdir -p "$DIR/$p"
        assets=$DIR/$p/assets
        reuse=$(reused_asset_set "$p")
        if [ "$p" = universe ]; then
            mkdir -p "$DIR/$p/empty-source"
            python3 "$HERE/package-assets.py" "$DIR/$p/empty-source" "$assets/private-assets" universe > /dev/null
        elif [ -n "$reuse" ]; then
            fetch_asset_manifest "$reuse" "$assets"
        else
            [ -d "$source" ] || die "No private asset source at $source (set AETHYR_PRIVATE_SOURCE)."
            python3 "$HERE/package-assets.py" "$source" "$assets/private-assets" "$(asset_profile "$p")" > /dev/null
        fi
        asset_set=$(field "$assets/assets-manifest.json" sha256)
        (
            cd "$SRC/apps/aethyr"
            export VITE_PUBLIC_DEMO=1
            export VITE_PREVIEW_URL=$preview/preview.html
            export VITE_EDITOR_ORIGIN=$editor
            export VITE_OPENAGENTS_HOST_ORIGIN=$HOST_ORIGIN
            export VITE_OPENAGENTS_ENGINE_RELEASE=$release
            export VITE_OPENAGENTS_ASSET_SET=$asset_set
            # Linked at build time: the SDK built above, on this release's own origin.
            export VITE_OPENAGENTS_PROTOCOL_URL=$editor/sdk/$sdk_sha/aethyrforge_host.js
            case "$p" in
                house) export VITE_WORLD_PROFILE=haunted-house ;;
                planet) export VITE_WORLD_PROFILE=personal-planet ;;
                *) unset VITE_WORLD_PROFILE ;;
            esac
            npx vite build --outDir "$DIR/$p/dist" --emptyOutDir >&2
        )
        grep -rqF "$editor/sdk/$sdk_sha/aethyrforge_host.js" "$DIR/$p/dist/assets" ||
            die "The $p bundle does not name the SDK built with it."
        local render_profile=world
        [ "$p" != universe ] || render_profile=universe
        (cd "$SRC/apps/aethyr" && python3 "$SRC/scripts/deploy/aethyr/render.py" \
            --release "$release" --editor "$editor" --preview "$preview" --host "$HOST_ORIGIN" \
            --sdk "$sdk_sha" --dist "$DIR/$p/dist" --sdk-directory "$DIR/sdk" \
            --assets-manifest "$assets/assets-manifest.json" --output "$DIR/$p/context" \
            --protocol-version "$PROTOCOL_VERSION" --profile "$render_profile" > /dev/null)
        cp "$SRC/scripts/deploy/aethyr/cloudbuild.yaml" "$DIR/$p/context/editor/"
        cp "$SRC/scripts/deploy/aethyr/cloudbuild.yaml" "$DIR/$p/context/preview/"
    done

    python3 - "$DIR" "$commit" "$sdk_sha" "$kernel_sha" "$PROTOCOL_VERSION" "$(profiles | tr '\n' ' ')" <<'PY'
import hashlib, json, os, platform, subprocess, sys
from pathlib import Path
d, commit, sdk_sha, kernel_sha, protocol, names = sys.argv[1:]
d = Path(d)
src = d / 'src'
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def tree_of(root, files):
    lines = ''.join(f'{sha(f)}  ./{f.relative_to(root)}\n' for f in sorted(files, key=lambda f: str(f.relative_to(root))))
    return hashlib.sha256(lines.encode()).hexdigest()
def tree(root): return tree_of(root, [f for f in root.rglob('*') if f.is_file()])
def run(*cmd, cwd=None):
    try: return subprocess.check_output(cmd, cwd=cwd, text=True, stderr=subprocess.DEVNULL).strip()
    except Exception: return None
demos = src / 'crates/openagents-web/content/demos'
pin_files = {'universe': ['universe.json'], 'house': ['haunted-house.json', 'runtimes/haunted-house.json'],
             'planet': ['runtimes/personal-planet.json'], 'town': ['medieval-town.json']}
record = {
    'schema': 'openagents.aethyr.release-build.v1',
    'commit': commit,
    'protocol_version': int(protocol),
    'toolchain': {
        'node': run('node', '--version'), 'npm': run('npm', '--version'),
        'rustc_root': run('rustc', '--version', cwd=src),
        'rustc_aethyr': run('rustc', '--version', cwd=src / 'crates/aethyr'),
        'wasm_bindgen': run('wasm-bindgen', '--version'),
        'wasm_opt': run(os.environ.get('AETHYRFORGE_WASM_OPT', 'wasm-opt'), '--version'),
        'platform': platform.platform(),
    },
    'sdk': {'source': 'crates/aethyrforge-host', 'wasm_sha256': sdk_sha,
            'files': {f.name: sha(f) for f in sorted((d / 'sdk').iterdir()) if f.is_file() and f.suffix in ('.js', '.wasm')},
            # The files render.py ships at /sdk/SHA256/, not the test pages beside them.
            'closure_sha256': tree_of(d / 'sdk', [f for f in (d / 'sdk').rglob('*') if f.is_file() and (f.suffix in ('.js', '.wasm'))])},
    'kernel': {'path': 'apps/aethyr/public/kernel.wasm', 'sha256': kernel_sha,
               'bytes': (src / 'apps/aethyr/public/kernel.wasm').stat().st_size},
    'profiles': {},
}
for p in names.split():
    pd = d / p
    release_json = pd / 'context/release.json'
    manifest = json.loads(release_json.read_text())
    assets = json.loads((pd / 'assets/assets-manifest.json').read_text())
    release_sha = sha(release_json)
    record['profiles'][p] = {
        'release': manifest['release'], 'tag': manifest['release'],
        'editor': manifest['editor_url'].rstrip('/'), 'preview': manifest['preview_url'].removesuffix('/preview.html'),
        'asset_set_sha256': assets['sha256'], 'asset_files': len(assets['files']),
        'asset_bytes': sum(f['bytes'] for f in assets['files']),
        'largest_asset': max(assets['files'], key=lambda f: f['bytes'])['path'] if assets['files'] else None,
        'asset_set_source': (pd / 'assets/reused-from').read_text().strip() if (pd / 'assets/reused-from').exists() else 'packaged',
        'release_json_sha256': release_sha,
        'context_sha256': {role: tree(pd / 'context' / role) for role in ('editor', 'preview')},
        'site_files': {role: len(manifest['files'][role]) for role in ('editor', 'preview')},
    }
    # The pins this profile would publish, from the commit's current ones.
    editor = record['profiles'][p]['editor']
    sdk_url = f'{editor}/sdk/{sdk_sha}/aethyrforge_host.js'
    for name in pin_files[p]:
        pin = json.loads((demos / name).read_text())
        out = d / 'candidates' / name
        out.parent.mkdir(parents=True, exist_ok=True)
        if name.startswith('runtimes/'):
            previous_set = pin['asset_set_sha256']
            pin.update(engine_url=editor + '/', sdk_url=sdk_url, sdk_sha256=sdk_sha,
                       engine_release=manifest['release'], engine_artifact_sha256=release_sha,
                       protocol_version=int(protocol), asset_set_sha256=assets['sha256'])
            # scene_sha256, artifacts, and material_ids describe the asset set.
            # They carry over only when every listed artifact is in the new set.
            files = {'/private-assets/' + f['path']: f for f in assets['files']}
            carried = all(a['path'] in files and files[a['path']]['sha256'] == a['resource']['sha256']
                          and files[a['path']]['bytes'] == a['resource']['bytes'] for a in pin['artifacts'])
            record['profiles'][p]['runtime_carried_over'] = carried and previous_set == assets['sha256']
        else:
            pin.update(engine_url=editor + ('/universe.html' if p == 'universe' else '/'), sdk_url=sdk_url,
                       release=manifest['release'])
        out.write_text(json.dumps(pin, indent=2) + '\n')
(d / 'build.json').write_text(json.dumps(record, indent=2) + '\n')
print(json.dumps({'commit': commit, 'sdk': sdk_sha, 'kernel': kernel_sha,
                  'profiles': {p: {k: v[k] for k in ('release', 'asset_set_sha256', 'release_json_sha256')} for p, v in record['profiles'].items()}}, indent=2))
PY
    say "built $DIR (build.json, candidates/)"
}

# --- stage -----------------------------------------------------------------

existing_tags() { # SERVICE
    read_or "" run services describe "$1" --region "$REGION" --project "$PROJECT" \
        --format='value(spec.traffic[].tag)' | tr ';,' '\n' | grep . || true
}

asset_bucket() {
    if [ -n "${AETHYR_ASSET_BUCKET:-}" ]; then echo "$AETHYR_ASSET_BUCKET"; return; fi
    read_or '{}' run services describe aethyrforge-preview --region "$REGION" --project "$PROJECT" --format=json |
        python3 -c 'import json,sys
spec=json.load(sys.stdin).get("spec",{}).get("template",{}).get("spec",{})
names=[v.get("csi",{}).get("volumeAttributes",{}).get("bucketName") for v in spec.get("volumes",[])]
names=[n for n in names if n]
print(names[0] if names else "BUCKET-FROM-LIVE-PREVIEW")'
}

stage() {
    need_dir "${1:-}"
    require_automation_account
    local commit
    commit=$(field "$DIR/build.json" commit)
    [ "$(git -C "$SRC" rev-parse HEAD)" = "$commit" ] || die "$SRC no longer holds $commit."
    python3 - "$DIR" <<'PY' || die "A context changed since build; rebuild."
import hashlib, json, sys
from pathlib import Path
d = Path(sys.argv[1])
def tree(root):
    lines = ''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  ./{f.relative_to(root)}\n' for f in sorted(root.rglob('*'), key=lambda f: str(f.relative_to(root))) if f.is_file())
    return hashlib.sha256(lines.encode()).hexdigest()
for p, v in json.load(open(d / 'build.json'))['profiles'].items():
    for role, want in v['context_sha256'].items():
        if tree(d / p / 'context' / role) != want: sys.exit(f'{p}/{role} changed')
PY
    local bucket editor_tags preview_tags
    bucket=$(asset_bucket)
    say "stage $(field "$DIR/build.json" commit | cut -c1-10) as the automation account; assets in gs://$bucket"
    editor_tags=$(existing_tags aethyrforge-editor)
    preview_tags=$(existing_tags aethyrforge-preview)
    local p release asset_set prefix role image digest
    for p in $(python3 -c 'import json,sys; print(" ".join(json.load(open(sys.argv[1]))["profiles"]))' "$DIR/build.json"); do
        release=$(field "$DIR/build.json" "profiles.$p.release")
        if printf '%s\n%s\n' "$editor_tags" "$preview_tags" | grep -qx "$release"; then
            die "Tag $release is already in use; release tags are never reused. Rebuild with a new AETHYR_RELEASE_SUFFIX."
        fi
    done
    : > "$DIR/stage.tsv"
    for p in $(python3 -c 'import json,sys; print(" ".join(json.load(open(sys.argv[1]))["profiles"]))' "$DIR/build.json"); do
        release=$(field "$DIR/build.json" "profiles.$p.release")
        asset_set=$(field "$DIR/build.json" "profiles.$p.asset_set_sha256")
        prefix=gs://$bucket/assetsets/$asset_set
        say "  $p: $release"
        # The asset set: written once, never overwritten; its manifest goes last.
        if g storage ls "$prefix/assets-manifest.json" > /dev/null 2>&1; then
            say "    asset set $asset_set is already uploaded"
        elif [ -f "$DIR/$p/assets/reused-from" ]; then
            [ -n "${DRY_RUN:-}" ] || die "The reused asset set $prefix is missing; build $p from the private source instead."
            say "    DRY_RUN: would mount the reused asset set $prefix"
        else
            if [ -n "$(ls -A "$DIR/$p/assets/private-assets")" ]; then
                apply storage cp --recursive --no-clobber "$DIR/$p/assets/private-assets" "$prefix/"
            fi
            apply storage cp --no-clobber "$DIR/$p/assets/assets-manifest.json" "$prefix/assets-manifest.json"
        fi
        for role in editor preview; do
            image=$REPO/aethyrforge-$role:$release
            apply builds submit "$DIR/$p/context/$role" --project "$PROJECT" --service-account "$BUILD_SA" \
                --config "$DIR/$p/context/$role/cloudbuild.yaml" --substitutions "_IMAGE=$image"
            digest=$(read_or "sha256:DRY-RUN-$role-$release" artifacts docker images describe "$image" \
                --format='value(image_summary.digest)')
            case "$digest" in sha256:*) ;; *) die "No digest for $image." ;; esac
            if [ "$role" = editor ]; then
                apply run deploy aethyrforge-editor --image "$REPO/aethyrforge-editor@$digest" \
                    --region "$REGION" --project "$PROJECT" --use-http2 --no-traffic --tag "$release"
            else
                apply run deploy aethyrforge-preview --image "$REPO/aethyrforge-preview@$digest" \
                    --region "$REGION" --project "$PROJECT" --use-http2 --no-traffic --tag "$release" \
                    --add-volume "name=runtime,type=cloud-storage,bucket=$bucket,readonly=true,mount-options=only-dir=assetsets/$asset_set" \
                    --add-volume-mount volume=runtime,mount-path=/runtime
            fi
            local revision
            revision=$(read_or "DRY-RUN-$role-revision" run services describe "aethyrforge-$role" --region "$REGION" \
                --project "$PROJECT" --format=json | python3 -c 'import json,sys
v=sys.stdin.read()
try: t=json.loads(v)["status"]["traffic"]
except Exception: print(v.strip()); sys.exit()
print(next((e["revisionName"] for e in t if e.get("tag")==sys.argv[1]),""))' "$release")
            printf '%s\t%s\t%s\t%s\t%s\n' "$p" "$role" "$release" "$digest" "$revision" >> "$DIR/stage.tsv"
        done
        smoke "$p"
    done
    python3 - "$DIR" "$bucket" "${DRY_RUN:-}" <<'PY'
import json, sys
from pathlib import Path
d, bucket, dry = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
out = {'schema': 'openagents.aethyr.release-stage.v1', 'dry_run': bool(dry), 'asset_bucket': bucket, 'profiles': {}}
for line in (d / 'stage.tsv').read_text().splitlines():
    p, role, release, digest, revision = line.split('\t')
    out['profiles'].setdefault(p, {'tag': release})[role] = {'image_digest': digest, 'revision': revision}
(d / 'stage.json').write_text(json.dumps(out, indent=2) + '\n')
PY
    say "staged; next: $0 check $DIR"
}

# The tag answers, names its release, and serves its largest runtime file.
smoke() {
    local p=$1 release editor preview largest
    release=$(field "$DIR/build.json" "profiles.$p.release")
    editor=$(field "$DIR/build.json" "profiles.$p.editor")
    preview=$(field "$DIR/build.json" "profiles.$p.preview")
    largest=$(field "$DIR/build.json" "profiles.$p.largest_asset")
    if [ -n "${DRY_RUN:-}" ]; then say "    DRY_RUN: would check $editor/health, $preview/health, and the largest runtime file ($largest)"; return 0; fi
    local url
    for url in "$editor" "$preview"; do
        curl -fsS --http2 -o /dev/null "$url/health" || die "$url/health did not answer."
        curl -fsSI --http2 "$url/health" | tr -d '\r' | grep -qi "^x-aethyrforge-release: $release$" || die "$url does not serve $release."
    done
    if [ "$largest" != None ]; then
        curl -fsS --http2 -H "Origin: $HOST_ORIGIN" -o "$DIR/$p/largest.bin" "$preview/private-assets/$largest" ||
            die "$preview did not serve its largest runtime file."
        # Against the manifest, which a reused asset set has without its files.
        [ "$(sha "$DIR/$p/largest.bin")" = "$(python3 -c 'import json,sys
print(next(f["sha256"] for f in json.load(open(sys.argv[1]))["files"] if f["path"]==sys.argv[2]))' "$DIR/$p/assets/assets-manifest.json" "$largest")" ] ||
            die "The largest runtime file changed in transit."
        rm -f "$DIR/$p/largest.bin"
    fi
    if [ "$largest" != None ] && curl -fsS -o /dev/null "$editor/private-assets/$largest" 2> /dev/null; then
        die "$editor serves private assets."
    fi
    say "    tags answer and serve $release"
}

# --- check -----------------------------------------------------------------

# AETHYRFORGE_RELEASE_CONFIG for one profile, as the browser checks read it.
check_config() { # PROFILE OUT
    python3 - "$DIR" "$1" "$2" <<'PY'
import json, sys
from pathlib import Path
d, p, out = Path(sys.argv[1]), sys.argv[2], sys.argv[3]
b = json.load(open(d / 'build.json'))
v = b['profiles'][p]
cfg = {'release': v['release'], 'editor': v['editor'], 'preview': v['preview'],
       'sdk_sha256': b['sdk']['wasm_sha256'], 'protocol_version': b['protocol_version']}
cand = d / 'candidates'
def current(name):
    f = cand / name
    return json.loads((f if f.exists() else d / 'src/crates/openagents-web/content/demos' / name).read_text())
cfg['publish_runtime'] = current('runtimes/haunted-house.json')
cfg['personal_runtime'] = current('runtimes/personal-planet.json')
if 'planet' in b['profiles']: cfg['personal_preview'] = b['profiles']['planet']['preview']
Path(out).write_text(json.dumps(cfg, indent=2) + '\n')
PY
}

pin_env() { # NAME FILE -> compact JSON in NAME for the native consumer
    export "$1=$(python3 -c 'import json,sys; print(json.dumps(json.load(open(sys.argv[1])),separators=(",",":")))' "$2")"
}

# A changed asset set needs a reviewed publishing runtime (scene, artifacts,
# materials); AETHYR_RUNTIME_HOUSE and AETHYR_RUNTIME_PLANET name it.
runtime_overrides() {
    python3 - "$DIR" <<'PY'
import hashlib, json, os, sys
from pathlib import Path
d = Path(sys.argv[1])
b = json.load(open(d / 'build.json'))
done = {}
for p, name in (('house', 'runtimes/haunted-house.json'), ('planet', 'runtimes/personal-planet.json')):
    v = b['profiles'].get(p)
    if not v or v.get('runtime_carried_over'): continue
    override = os.environ.get('AETHYR_RUNTIME_' + p.upper())
    if not override:
        sys.exit(f'The {p} asset set changed: set AETHYR_RUNTIME_{p.upper()} to the reviewed publishing runtime JSON.')
    text = Path(override).read_bytes()
    runtime = json.loads(text)
    want = json.loads((d / 'candidates' / name).read_text())
    for k in ('engine_url', 'sdk_url', 'sdk_sha256', 'engine_release', 'asset_set_sha256', 'engine_artifact_sha256', 'protocol_version'):
        if runtime.get(k) != want[k]: sys.exit(f'{override}: {k} does not match the release ({want[k]})')
    files = {'/private-assets/' + f['path']: f for f in json.loads((d / p / 'assets/assets-manifest.json').read_text())['files']}
    for a in runtime['artifacts']:
        f = files.get(a['path'])
        if not f or f['sha256'] != a['resource']['sha256'] or f['bytes'] != a['resource']['bytes']:
            sys.exit(f'{override}: {a["path"]} is not in the asset set')
    (d / 'candidates' / name).write_bytes(text)
    done[p] = hashlib.sha256(text).hexdigest()
if done: (d / 'runtime-overrides.json').write_text(json.dumps(done, indent=2) + '\n')
PY
}

# Runs one check in its own browser, or in the AETHYR_CHECK_CDP browser after
# clearing what an earlier check left there.
check_browser() {
    if [ -z "${AETHYR_CHECK_CDP:-}" ]; then
        openagents browser run -- "$@"
        return
    fi
    local ws
    ws=$(curl -fsS "$AETHYR_CHECK_CDP/json/version" | python3 -c 'import json,sys; print(json.load(sys.stdin)["webSocketDebuggerUrl"])') || return 1
    node --input-type=module - "$ws" "$AETHYRFORGE_RELEASE_CONFIG" <<'JS' || return 1
import fs from "node:fs";
import { createRequire } from "node:module";
const { chromium } = createRequire(process.cwd() + "/")("playwright");
const [ws, configPath] = process.argv.slice(2);
const config = JSON.parse(fs.readFileSync(configPath));
const browser = await chromium.connectOverCDP(ws);
const context = browser.contexts()[0];
const [first, ...rest] = context.pages();
for (const page of rest) await page.close();
const page = first ?? (await context.newPage());
await page.goto("about:blank");
await context.clearCookies();
await context.unrouteAll({ behavior: "ignoreErrors" });
// Chromium refuses Storage.clearDataForOrigin on a browser-level session.
const cdp = await context.newCDPSession(page);
await cdp.send("Network.clearBrowserCache");
const origins = ["https://openagents.com", config.editor, config.preview, config.personal_preview].filter(Boolean);
for (const origin of new Set(origins.map((url) => new URL(url).origin)))
  await cdp.send("Storage.clearDataForOrigin", { origin, storageTypes: "all" });
await browser.close();
JS
    OPENAGENTS_CHROME_WS=$ws "$@"
}

check() {
    need_dir "${1:-}"
    [ -f "$DIR/stage.json" ] || die "Stage $DIR first."
    [ "$(field "$DIR/stage.json" dry_run)" = False ] || die "$DIR was staged as a dry run; there are no tags to check."
    if [ -n "${AETHYR_CHECK_CDP:-}" ]; then
        mkdir -p "$DIR/checks"
        curl -fsS "$AETHYR_CHECK_CDP/json/version" > "$DIR/checks/browser.json" || die "No DevTools endpoint at $AETHYR_CHECK_CDP."
    else
        command -v openagents > /dev/null || die "Install the openagents CLI: every check runs in its own browser through 'openagents browser run'."
    fi
    # The checks load Playwright; apps/aethyr installs it.
    [ ! -d "$SRC/apps/aethyr/node_modules/playwright" ] || export NODE_PATH=$SRC/apps/aethyr/node_modules${NODE_PATH:+:$NODE_PATH}
    : "${CARGO_TARGET_DIR:=$HOME/work/openagents-target-1}"
    export CARGO_TARGET_DIR
    CHECK_SRC=$SRC
    if [ -n "${AETHYR_CHECK_REF:-}" ]; then
        local ref
        ref=$(git -C "$ROOT" rev-parse --verify "$AETHYR_CHECK_REF^{commit}") || die "Unknown AETHYR_CHECK_REF: $AETHYR_CHECK_REF"
        CHECK_SRC=$DIR/check-src
        if [ -d "$CHECK_SRC" ]; then
            git -C "$CHECK_SRC" checkout --quiet --detach "$ref"
        else
            git -C "$ROOT" worktree add --quiet --detach "$CHECK_SRC" "$ref"
        fi
        say "  website and checks from $ref"
    fi
    # shellcheck source=/dev/null
    (cd "$CHECK_SRC" && . scripts/dev/cargo-build-env.sh && cargo build --locked -p openagents-web-site --bin openagents-web >&2)
    export OPENAGENTS_TEST_WEB_BINARY=$CARGO_TARGET_DIR/debug/openagents-web
    runtime_overrides || exit 1
    local c=$DIR/candidates
    [ ! -f "$c/universe.json" ] || pin_env OPENAGENTS_UNIVERSE_RELEASE "$c/universe.json"
    [ ! -f "$c/haunted-house.json" ] || pin_env OPENAGENTS_HAUNT_RELEASE "$c/haunted-house.json"
    [ ! -f "$c/medieval-town.json" ] || pin_env OPENAGENTS_MEDIEVAL_RELEASE "$c/medieval-town.json"
    [ ! -f "$c/runtimes/haunted-house.json" ] || pin_env OPENAGENTS_VERSE_PUBLISH_RUNTIME "$c/runtimes/haunted-house.json"
    [ ! -f "$c/runtimes/personal-planet.json" ] || pin_env OPENAGENTS_VERSE_PERSONAL_RUNTIME "$c/runtimes/personal-planet.json"

    # The website consumer that serves the candidate pins at the host origin.
    if [ -z "${AETHYRFORGE_HOST_RESPONSE_URL:-}" ]; then
        mkdir -p "$DIR/checks/host-home"
        HOME=$DIR/checks/host-home "$OPENAGENTS_TEST_WEB_BINARY" --listen 127.0.0.1:0 \
            --store "$DIR/checks/host-tasks" --chat-store "$DIR/checks/host-chats" \
            --pylons-relay off --profile-relay off > "$DIR/checks/host.log" 2>&1 &
        HOST_PID=$!
        trap 'kill "$HOST_PID" 2> /dev/null || true' EXIT
        for _ in $(seq 1 120); do
            AETHYRFORGE_HOST_RESPONSE_URL=$(grep -o 'listening on http://[^ ]*' "$DIR/checks/host.log" | head -1 | cut -d' ' -f3 || true)
            [ -z "$AETHYRFORGE_HOST_RESPONSE_URL" ] || break
            sleep 0.5
        done
        [ -n "$AETHYRFORGE_HOST_RESPONSE_URL" ] || die "The native website did not start; see $DIR/checks/host.log."
    fi
    export AETHYRFORGE_HOST_RESPONSE_URL AETHYRFORGE_CHECK_MODE=candidate
    say "check against host responses from $AETHYRFORGE_HOST_RESPONSE_URL"

    local profile script failed=0
    : > "$DIR/checks/results.tsv"
    while read -r profile script; do
        python3 -c 'import json,sys; sys.exit(sys.argv[2] not in json.load(open(sys.argv[1]))["profiles"])' "$DIR/build.json" "$profile" || continue
        local name
        name=$(basename "$script" .mjs)
        # Each check starts from an empty scratch folder: the checks keep their
        # native project store (chats/, tasks/, native-home/) there, and their
        # synthetic account service numbers projects from prj_..01 each run, so
        # a store left by an earlier run on this release directory answers the
        # first save with 409 newer_version.
        rm -rf "${DIR:?}/checks/$name"
        mkdir -p "$DIR/checks/$name"
        check_config "$profile" "$DIR/checks/$name/config.json"
        say "  $name ($profile)"
        if AETHYRFORGE_DEPLOY_SCRATCH=$DIR/checks/$name AETHYRFORGE_RELEASE_CONFIG=$DIR/checks/$name/config.json \
            check_browser node "$CHECK_SRC/$script" < /dev/null > "$DIR/checks/$name/output.log" 2>&1; then
            printf '%s\t%s\tpass\n' "$profile" "$name" >> "$DIR/checks/results.tsv"
        else
            printf '%s\t%s\tfail\n' "$profile" "$name" >> "$DIR/checks/results.tsv"
            say "    failed; see $DIR/checks/$name/output.log"
            failed=1
        fi
    done <<'CHECKS'
town scripts/check-medieval-town.mjs
house scripts/check-haunted-house.mjs
house scripts/check-verse-project.mjs
house scripts/check-verse-publish.mjs
house scripts/check-verse-material.mjs
house scripts/check-verse-access.mjs
house scripts/check-verse-voice.mjs
house scripts/check-verse-workspace.mjs
universe scripts/check-verse-universe.mjs
CHECKS
    python3 - "$DIR" "$AETHYRFORGE_HOST_RESPONSE_URL" "$(git -C "$CHECK_SRC" rev-parse HEAD)" <<'PY'
import json, sys
from pathlib import Path
d = Path(sys.argv[1])
rows = [l.split('\t') for l in (d / 'checks/results.tsv').read_text().splitlines()]
out = {'schema': 'openagents.aethyr.release-checks.v1', 'mode': 'candidate', 'host_responses': sys.argv[2], 'checks_commit': sys.argv[3],
       'browser': json.loads((d / 'checks/browser.json').read_text()).get('Browser') if (d / 'checks/browser.json').exists() else 'openagents browser run',
       'checks': [{'profile': p, 'check': n, 'result': r} for p, n, r in rows]}
out['passed'] = bool(rows) and all(r == 'pass' for _, _, r in rows)
(d / 'checks.json').write_text(json.dumps(out, indent=2) + '\n')
PY
    [ "$failed" = 0 ] || die "A candidate check failed; do not pin."
    say "every candidate check passed; next: $0 pin $DIR"
}

# --- pin -------------------------------------------------------------------

pin() {
    need_dir "${1:-}"
    [ -f "$DIR/checks.json" ] && [ "$(field "$DIR/checks.json" passed)" = True ] || die "Pin only a release whose candidate checks passed."
    local commit
    commit=$(field "$DIR/build.json" commit)
    git -C "$ROOT" merge-base --is-ancestor "$commit" origin/main || die "Pin only a release built from origin/main."
    python3 - "$DIR" "$ROOT" <<'PY'
import json, os, shutil, sys
from pathlib import Path
d, root = Path(sys.argv[1]), Path(sys.argv[2])
b = json.load(open(d / 'build.json'))
s = json.load(open(d / 'stage.json'))
c = json.load(open(d / 'checks.json'))
demos = root / 'crates/openagents-web/content/demos'
rollback = demos / 'rollback'
overrides = json.loads((d / 'runtime-overrides.json').read_text()) if (d / 'runtime-overrides.json').exists() else {}
for p, v in b['profiles'].items():
    if p in ('house', 'planet') and not v.get('runtime_carried_over') and p not in overrides:
        sys.exit(f'The {p} asset set changed and no reviewed publishing runtime was checked; rerun check with AETHYR_RUNTIME_{p.upper()}.')
out = root / 'docs/verse/aethyr-releases' / (b['commit'][:10] + '.json')
if out.exists(): sys.exit(f'{out} exists; qualification entries are never edited.')
written = []
for f in sorted((d / 'candidates').rglob('*.json')):
    name = f.relative_to(d / 'candidates')
    old = demos / name
    keep = rollback / name
    keep.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(old, keep)
    shutil.copyfile(f, old)
    written.append(str(name))
entry = {
    'schema': 'openagents.aethyr.release-qualification.v1',
    'issue': 12590,
    'commit': b['commit'],
    'protocol_version': b['protocol_version'],
    'toolchain': b['toolchain'],
    'sdk': {'wasm_sha256': b['sdk']['wasm_sha256'], 'closure_sha256': b['sdk']['closure_sha256']},
    'kernel_sha256': b['kernel']['sha256'],
    'profiles': {p: {**{k: v[k] for k in ('release', 'editor', 'preview', 'asset_set_sha256', 'asset_files', 'asset_bytes', 'release_json_sha256', 'context_sha256')},
                     **{role: s['profiles'][p][role] for role in ('editor', 'preview')}}
                 for p, v in b['profiles'].items()},
    'asset_bucket_prefix': 'assetsets/',
    'candidate_checks': c['checks'],
    'pins': written,
    'rollback': [f'rollback/{n}' for n in written],
    'production_checks': 'pending: run the same checks without candidate mode after the website shift',
}
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(entry, indent=2) + '\n')
print('\n'.join(['wrote ' + str(demos / n) for n in written] + ['kept ' + str(rollback / n) for n in written] + ['wrote ' + str(out)]))
PY
    say "Commit the pins, the rollback copies, and the entry; then ship them with scripts/deploy/web.sh."
}

case "${1:-}" in
    build) shift; build "$@" ;;
    stage) shift; stage "$@" ;;
    check) shift; check "$@" ;;
    pin) shift; pin "$@" ;;
    *) sed -n '2,8p' "$0" >&2; exit 2 ;;
esac
