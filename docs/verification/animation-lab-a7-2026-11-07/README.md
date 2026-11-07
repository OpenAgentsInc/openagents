# Animation Lab A7 verification

Issue: [#12972](https://github.com/OpenAgentsInc/openagents/issues/12972).
Scope: typed Decisions transport, shared editing, CLI versions and realm
directing. No production deployment. See the
[implemented profile](../../verse/anim-directing-v1.md).

Coax 0.52.0, compiler SHA-256
`23dfc150fa0d21dd199b2da382b9c2dd0762e8183e3f576657fe1341d327117e`.
All builds and checks ran in the asset-library worktree.

## Gates

All seven gates kill 1,724 mutants with zero survivors. Each authority mutation command uses `--root-only`, retaining dependency
checks. No mutant was marked equivalent. Red tests preceded the speed
serialization, camera screening and lowercase/Escape keyboard fixes.

| Authority | Killed | Survived |
| --- | ---: | ---: |
| Shared editing core | 724 | 0 |
| JSON primitives | 271 | 0 |
| Typed direction wire | 101 | 0 |
| Decisions selector | 234 | 0 |
| Session history | 122 | 0 |
| Scene screening | 78 | 0 |
| CLI arguments | 194 | 0 |

The moved project loader, sampler, checker and their checks have identical
bodies to A5 at `e25377c906`, after excluding `#load` paths. A5's existing
701-mutant qualification is retained for those unchanged bodies. JSON/name/
number extraction is independently mutation-qualified above. The writer's
0.01 binary32 minimum speed now emits canonical JSON `0.01`; its round-trip
regression passes, as do exact numeric edit bounds.

## Native API and file effects

`aethyr/tests/anim_direct_oracle.py` runs the actual rebuilt CLI and API against
an owned loopback PostgreSQL database and a controlled model HTTP endpoint.
The API retains normal NIP-98 verification, origin checks and quota accounting.
Only temporary fixture principals are used; their private keys are removed.
The independent oracle reads ALP tables and JSON; it does not implement edits.

The private Guardian fixture admits five stock clips, including an original
static wand key. Speed, path, look-at, hand-to, hold, clip swap, pose key,
shot insertion, shot retiming and cut-on-event each produce exactly one
changed JSON component and preserve all other bundle bytes. Each result
passes `labs anim check`, exposes full confidence, has a full diff and undoes
to the exact original bundle. Source files remain untouched. Shot removal is additionally qualified at the first, middle, last and 128-shot boundaries; removing the only shot is refused. The rebuilt CLI/API also checks shot removal separately under `~/.openagents/verse/private-build/anim-a7-remove/`.

Additional cases cover unmatched and ambiguous replies, absent clips,
malformed model output, hero framing failure and a source change during the
request. None publishes a version. Equal-content edits with different words
or confidence retain distinct immutable forks. Corrupted current receipts,
payloads and parent receipts refuse without moving the head.

Requests, snapshots, receipts and reports remain under
`~/.openagents/verse/private-build/anim-a7-qualified/`; none is committed.
The private source bundle hash is
`66c1116a259f489d6a7a82585b1079517a86ccae937a191aa5a5bacb6ba46d08`.

## Browser

`coax/web/anim-direct-check.coax` uses muted headless Chrome, actual Wasm,
the C effect bridge and browser HTTP host. Only the model/API reply is
controlled for this isolated browser test; the real API is qualified above.
One typed message creates one version with confidence 950, lowercase D
copies the full diff, and lowercase U restores speed 1.000 and the original
parent. The screenshot contains the visible WebGPU scene and controls, and
the browser reports no console errors. Unit checks also cover explicit voice
activation, duplicate final-transcript suppression, cancellation, response
ownership and the host's Escape code. No hardware microphone was opened.

The Wasm is 388,523 bytes, below its existing 420,000-byte budget. Captures
remain private in `~/.openagents/verse/private-build/anim-a7-browser/`.

## Limit

The live-provider mode was attempted, but `gcloud auth print-access-token`
could not issue a token. Model interpretation quality against the live
provider is therefore unqualified; the ten operation checks use controlled
schema-valid responses through the real authenticated API. No live-provider
success or production deployment is claimed.

Reproduce the private operation checks with:

```sh
python3 aethyr/tests/anim_direct_oracle.py --project PRIVATE_SOURCE.alp \
  --out ~/.openagents/verse/private-build/NEW_PRIVATE_DIR \
  --cli apps/openagents-cli/build/openagents \
  --api-bin OWNED_API_BINARY
```

Add `--live` only with an authorized provider credential. Every command is
bounded below fifteen minutes; full unrelated compiler/model suites are not
part of this change.
