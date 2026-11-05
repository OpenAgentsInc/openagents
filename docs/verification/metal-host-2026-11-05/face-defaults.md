# Generated 5E network defaults and native cover/resume

The 5E C face allocated `net_state` as zeroed static storage but omitted its
generated `coax_default_t_EgNet` constructor. That changed Coax defaults:
`LifeState.active=true`, `away_since=-1` and `drawing=true` became zero.
With the native monotonic clock, the first reveal was therefore treated as
a long absence and closed a healthy local room connection. The fix calls
the generated constructor alongside the other face globals. No extra
startup visibility message is added.

`coax/web/state-defaults.coax native-prepared NAME FACE.C UNIT.C...` now
checks actual native constructor storage against generated defaults using
already emitted, source-joined units. It does not emit new compiler C or
claim current browser/primary qualification. All 138 tool checks pass. The
exact 17a742c08c six-unit fixture from `current-c64-native.md` reports seven
defaulted globals: Player, Sticks, EgNet, GaSession, River, E5r and DemoWorld.
Only EgNet fails before the fix (697,112 bytes); all seven pass afterward.
The other face globals have no generated default constructor in these
headers. Existing trace reset calls remain unchanged.

The actual isolated native bundle was then tested with the matching 17a
face plus this one-line constructor fix, the admitted graphics package and
the ordinary shared presentation selector. The strict existing
`metal-stage-smoke --require-network --require-audio` gate passes:

| Observation | GPU frames/completed | Room net/seat | Messages | Audio frames/callbacks |
| --- | --- | --- | --- | --- |
| Play | 97/95 | 2/0 | 45 | 50,176/97 |
| Covered | 97/97 | 2/0 | 62 | 50,176/97 |
| Still covered | 97/97 | 2/0 | 92 | 50,176/97 |
| Resumed | 154/152 | 2/0 | 109 | 73,728/144 |

Every physical device sample is muted zero. Close observes audio retired,
inactive, and GPU ledger live/words zero. The native visibility workaround
was removed before this run. An earlier attempt copied the library to the
wrong resource directory and consequently loaded the old producer; that
failed attempt is retained rather than used as corrected evidence.

Durable raw evidence is under
`~/.openagents/aethyrforge/metal-host-12868/graphics-admitted-3f8/`:
`face-defaults-{red,green}.log`, `state-defaults-checks.log`,
`defaults-producer-build.log`, `window-defaults-corrected/` and
`window-defaults-corrected.log`. `window-defaults-green/` is the retained
wrong-directory failure; `window-initial-shown/` is the separate temporary
workaround diagnostic. Neither is the corrected verdict.

Frozen corrected 17a face:
`defaults-consumer/apps/openagents-web/everglade5e-face.c`, SHA-256
`cea7d0533dd6c1cb6ffbfa80e56cd112f73d52a1a06108ee3cb372ef68846883`.
The six Coax C/header/object inputs remain unchanged.
`defaults-sdk/libeverglade.dylib` SHA-256 is
`b924ccf9825d05eb10b19b1517edcfb8786ed98cadd87b14b1c374f7f4042848`;
the tested shell image SHA-256 is
`f7fe9cc4e3247b491d99d8e3a5a48deb312ad05cddf1b21bb6d103943b34e43f`.
The image uses a separate locally loaded producer library; its renderer
facade is not statically merged with duplicate generated exports.

The tool and face-only relink use the unchanged admitted Mac 0.50 image
`b5872254e02de4b2fb84f039e81f602090f5526301c036c077004294a7e580eb`.
Its original provenance receipt verifies against the protected source root;
no receipt is refreshed. This does not regenerate or relabel the older
17a CPU arithmetic as current main. Current producer packaging, display-rate
and remaining integrated host services remain separate gates. WebKit stays
the default and both host issues remain open.
