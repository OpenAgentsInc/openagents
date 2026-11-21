# Invariants — coax/INVARIANTS.md

> Scope: Coax language, compiler, runtime, #authority, stdlib, coax/web host (100 invariants). Indexed from root [INVARIANTS.md](../INVARIANTS.md).

## A change to `#authority` code passes its changed procedures' mutants

Owner decision, 2026-11-14 (#13583). A change that touches an `#authority`
unit passes `coax mutate --changed BASE` on each suite that loads one: every
check runs, and every mutant of a procedure whose text changed since BASE is
killed or marked equivalent with a reason; a changed procedure no check
reaches fails. `coax/remote/check.sh` (branch, and only with a BASE, which
the pre-push passes) runs it through `suites --mutate-changed`. After
landing, the same mutants are a report that blocks nothing (`check.sh
mutate-since`, the green-main pipeline's mutation report, #13601): it never
holds main-green, goes on the red list, reverts or counts as flaky, and the
`since` jobs run the suites alone. The
full sweep of every `#authority` file is a nightly report
(`coax/remote/mutation-sweep.sh`) and blocks nothing. It covers each
`#authority` file a suite reaches and each one outside the suites' folders,
except `docs/` (verification records), `coax/tests/` (compiler fixtures),
`coax/book/` and `coax/examples/` (book listings) and `psionic/`
(`psionic/check.sh` mutates its units to zero survivors). It keeps one
tracking issue: open while a night finds a survivor, an undecided mutant or
a part that did not run, closed with the night's summary when one is clean
(#13586). A unit's first pass still goes to zero survivors or marked equivalents. Shipped
code is not rewritten only so a mutant stops being equivalent. Checks:
`coax/tests/mutation_scope.py` (`--changed`: comment-only edits, one changed
body, new units, `--unit`, a bad REF), `coax/tests/mutation_parallel.py`
(several `--unit` equal their own runs), the checks in
`coax/web/suites.coax` and `coax/web/mutation-sweep.coax`, and
`coax/remote/test-after.sh` cases 15 and 16 (the report after landing).

## Host transport keeps concrete asynchronous owners across every effect

The v1 browser's `pack-stream.coax` and `private-pack.coax` bound public/private
streams before copying and use the existing whole Coax VPK1 reader. Each
stream is at most 64 MiB; separately retained allocation pages share a
256 MiB/64-ticket pool. Private metadata, decrypt/decompress extents and
plaintext identity precede asset publication. No chunk/prefix list or failed
pack part becomes ready. Cancellation and retirement retain tickets through
actual SDK completion and active immutable borrowers. Completed bytes can
transfer to a matching renderer; old pending completion cannot publish there.
The ordinary browser passes physical split/cut/TCP-cancel, current85c pack
reload/device recovery and held audio-source borrow observations. Required
pack failure continues RAF while refusing partial-world GPU submission.
Root component/adapter gates kill 155/132/72 and 43/31/36/28 mutants without
equivalents. The page's 8 MiB master remains retained; SDK/OS/GC memory is
separate. No producer/shader/default admission is inferred. Exact scope is
`docs/verification/host-contract-2026-11-05/browser-pack-loading.md`.

Known public/raw browser transfers additionally prepay their full reader
reactor and separately retained raw-array pages through existing pack-storage
leases. Admission happens before Wasm construction and fetch; a second-ticket
refusal publishes neither reservation and preserves terminal serial retirement.
Cancellation, uncertain SDK completion and immutable borrowers retain those
leases until zeroing/disposal. HTP1 derives complete container/source spans
from admitted bytes, binds the original scene/RCM1/model identities, and keeps
logical payload costs unchanged. The additive corridor carrier now binds
original scene/manifest, whole source, exact location and live attempt keys
before decoding. It refuses completion/destruction/rebinding until the SDK
has joined the reader. Bounded per-owner histories reconstruct original
leases and actual allocation peaks independently. See
`docs/verification/aethyr-streaming-12990/walk-transport-binding.md` and the
earlier `walk-transport-planning.md`.

The additive corridor diagnostic v2 admits its array and page reservation in
Coax. Exhaustion refuses observations, marks the history incomplete and
counts drops; it cannot throw through a resource effect or skip its join and
retirement. Only an explicit prefix check accepts incomplete observations.
Capture prepays readback, raw copy and logical UTF-16 headroom before SDK
allocation. Failed encoding zeroes its copy while original reservations stay
held through GPU completion observation. Mutable compiler input bytes are
zeroed and their extents reported separately from resource ownership; no
physical SDK/GC/driver or RSS claim follows. See
`docs/verification/aethyr-streaming-12990/walk-diagnostic-capture.md` and
`metaverse-engine/host/stream-adoption.md` for exact measured and remaining adapter scope.

`metaverse-engine/host/transport.coax` owns the four socket and eight response-bearing
HTTP slots for host contract v1. A callback carries its reserved slot,
generation and engine epoch; a reused slot cannot inherit an older callback.
Local close invalidates before returning and its deferred close is delivered
once only while that owner remains unreplaced. HTTP releases before callback;
cancellation and engine retirement are silent. A response overflow poisons
the complete request, never publishes a successful prefix, and bounded
failure is exactly `0\n`. Deadline and whole-response capacity include the
status line. Generations and epochs retire rather than wrap.

Version 1 HTTP inbox delivery preserves actual status/body bytes. Ordinary
unbounded replies may carry malformed UTF-8 and clip at the inbox's byte
capacity; a browser text conversion must not replace bytes, strip a leading
body BOM or discard the body on clipping. Bounded replies instead require
valid UTF-8 and the complete response inside capacity. The actual import,
inbox and next-frame browser regression checks all66 authored words, retaining
the original red plus unchanged pre-v1 helper controls. See
`docs/verification/host-contract-2026-11-05/http-bytes.md`.

`routes.coax` admits bounded root-relative HTTP inputs and the exact
development native-local URI/IPv4-loopback origin. `http-target.coax` owns
UTF-8 percent encoding, fragment removal, empty queries and dot segments;
its 192-mutant root gate has no survivors or equivalents. A normalized
pathname beginning `//` remains on the separately bound site origin.
Actual browser and sanitized Linux SDK receipts retain every physical peer
path and callback word (54 semantic and 23 JSON adapter mutants killed).
`buffers.coax` admits the complete
measured allocation before copying. The shared renderer carrier loads these
authorities; C/JavaScript project scalars and execute platform effects. Their
root gates at `3f1396f5a2` kill 292, 59 and 9 mutants, with zero survivors and
no equivalents. The written boundary and initial browser observations are
in `metaverse-engine/host/README.md` and
`docs/verification/host-contract-2026-11-05/`. Native adapters, production
GPU/lifecycle and rendered audio retain their separate conformance exits;
the desktop default stays web until the complete Metal/three-host gates pass.
The common core receipt admits every authored event word plus observed
version/default/dropped/held metadata through `conformance.coax` and its
`receipt.coax` JSON adapter (190 and 60 root mutants killed, no equivalents).
The Linux realm SDK exposes explicit still-frame and measured asset-copy
effects, so its fixture uses the same production dispatch as a realm.
Transport receipt admission checks the actual command/result/callback
streams and callback-after-reservation ordering, allowing only independent
connection scheduling (277 semantic and 20 JSON adapter root mutants killed).

Version 1 browser audio executes the shared Coax PCM interpreter in a real
48 kHz AudioWorklet; generated bank WAVs use the shared canonical decoder,
not platform resampling. All 48 actual native bank WAVs decode bit-identically
through the browser bridge. Eleven actual rendered ranges pass the unchanged
2^-20 sample bound without shifts or masks. Startup controls/beds may survive
suspension; earlier one-shot cues cannot acquire a later gesture. Engine
retirement fences pending decodes and closes the concrete node/context.
Actual muted host.js receipts cover gesture, pause/resume and replacement.
The word comparator kills 103/103 root mutants, with no equivalents. These
audio receipts remain separate from Linux output and complete three-host
qualification; the desktop default remains web.

The 48 generated WAVs are Everglade's explicit synth profile. Ordinary
default gameplay selects the picked Ogg/Opus variants and stereo bed.
Synth-bank decoding and DSP receipts do not qualify those codec outputs,
their complete retained-memory workload or default-profile audio parity.
The shared `audio/host-opus.coax` preflight admits all 122 picked sources
and bounds full coded PCM separately from pre-skip/EOS output trim. Its
395-mutant root gate has no survivors or equivalents; `hpo_output` requires
the exact admitted frame/channel shape and 48 kHz before an SDK publishes PCM.
SDK adoption and
codec-output comparisons remain separate; metadata is not decoded parity.

The complete authored 153-candidate inventory preserves all original Ogg
sources, including candidates outside the current pick set. Frozen SDK
observations fail the unchanged sample gate for 32 files. The new APC1
companion format binds original source bytes/SHA, exact finite F32 payload
bytes/SHA and output shape. Required companions refuse missing, corrupt or
mismatched bytes without SDK fallback or partial output. The complete
catalogue pins all 153 identities; 121 rows retain admitted SDK decoding.
Its format and catalogue root gates kill 179 and 26 mutants without
equivalents. These source/format components do not qualify host adoption,
default demand or compiler/build/load admission. Evidence and regeneration
identity are in `docs/verification/host-contract-2026-11-05/audio-pcm-assets.md`.
The bank registry separately caps aggregate copied source bytes at 8 MiB,
including overlapping spans under distinct names. Refusal changes no row,
count or byte total; reset releases all. Its sharp root gate kills 101 mutants.

The browser's v1 encoded-audio path now admits source copies before retention,
preflights Ogg before SDK PCM allocation and admits exact output shape/rate
before transfer. Required companions use the same Coax reader inside the
Worklet. Each observed scratch, encoded/decoded, transfer and retained plane
owns a separate shared ticket. Retirement waits for outstanding owned SDK
completion as well as Worklet cleanup; it cannot release an uncertain live
decode early. The actual held-decode trace preserves 31,586 words until
completion, then both owners and the command queue return to zero. Full
frozen demand retains 122 assets/7,739,161 words with no allocation refusal.
Actual default rendered cues/bed/room pass all 262,144 samples at unchanged
2^-20. Source-copy, receipt and refusal gates kill 19/70/44 root mutants.
The 105,010-byte audio and 20,024-byte services reactors have exact 24/13
source joins under the unchanged admitted c224/598 compiler. This qualifies
small host components, not current realm/graphics/default admission. See
`docs/verification/host-contract-2026-11-05/browser-opus.md`.

Version 1 browser clips replay the complete admitted command history through
that same 48 kHz Worklet interpreter, then crop the measured output. They do
not start a live device or use the pre-contract Web Audio mixer. The Coax
timeline owns at most 8,192 complete commands/8 MiB, each at most 1,024 bytes;
a refused offer poisons the entire recording and a sealed frontier cannot
change. Decoded sources and offline output/crop each use a separate existing
32 MiB PCM owner. Required asset, command or output refusal produces no WAV.
Retirement retains command bytes and output tickets until held descriptor/
decode effects and the concrete Worklet finish, then releases all ownership.
Actual ordinary-host receipts check every one of 9,600 PCM16 words and all
32 observed owner/clock fields; five physical refusal/retirement cases pass.
Timeline and observer root gates kill 103/202 mutants without equivalents.
The normally rebuilt 96,229-byte audio reactor also passes all 262,144
ordinary authored samples at unchanged 2^-20. These bounded clip/component
receipts do not substitute for complete ordinary three-host qualification.
See `docs/verification/host-contract-2026-11-05/browser-clip-audio.md`.

The Linux audio SDK executes the same Coax PCM/WAV interpreter and bank/
memory authorities. All 48 frozen bank assets decode byte-identically and
eleven rendered ranges pass the unchanged sample bound. Actual allocator,
pointer-span and retained-reference receipts pass normally and instrumented;
ticket ownership lasts through the final immutable sample reference. Real
muted PipeWire callbacks negotiate 48 kHz stereo, keep callback/Coax clocks
held while paused and after close, and publish zero nonzero device-buffer
samples. The device and retention observer root gates kill 59 and 11 mutants
without equivalents. These SDK components do not substitute for ordinary
Vulkan world integration or the complete host/default gates.

The Linux encoded-audio component admits all 153 original authored sources
through the shared catalogue/preflight and required companion reader. Actual
normal and instrumented SDK runs pass all 12,657,718 samples at unchanged
2^-20, including byte-exact companions and zero retained words after each row.
Shared read control requires the admitted frame count and an independent EOF
probe; finite publication and separate source/coded/probe/retained tickets
precede PCM publication. Missing/corrupt required data and exhausted capacity
publish no prefix and reach no SDK decode. New read/finite root gates kill
59/5 mutants. This component does not qualify ordinary Linux default demand,
realm/device composition or graphics/default admission. Exact provenance and
SDK scope are in `docs/verification/host-contract-2026-11-05/linux-opus.md`.

Ordinary Linux audio registers canonical bank bytes lazily. Actual frozen
startup-command replay retains all 122 requested assets/7,739,161 words;
complete default rendering passes all 262,144 samples at unchanged 2^-20.
Normal and instrumented runs preserve retained PCM through its last reference.
Decoding holds the asset owner separately from the mixer lock; a held decode
cannot acquire a later gesture or install after retirement. The threaded
callback-renderer observer kills 43 root mutants. The changed realm dispatcher
also passes the unchanged actual muted PipeWire product/session trace normally
and instrumented. These component/device receipts do not replace ordinary
Vulkan window, asset packaging or current realm/service integration. See
`docs/verification/host-contract-2026-11-05/linux-audio-demand.md`.

The Linux realm dispatcher admits physical bank metadata, WAV storage and
draining sound packet spans before forming views. A refused asset does not
silence later independently admitted assets. Its audio clock consumes the
shared `audio-session.coax` gesture/coverage/terminal-owner offer; a cue
captures gesture state when offered. Actual normal and instrumented realm
SDK traces cover packet draining, hostile spans, pre-gesture refusal,
covered gestures, pause/resume and retirement while unrelated producer
frames continue. Session and observer root gates kill 33 and 93 mutants
without equivalents. This recording-face/device trace does not qualify
the ordinary Vulkan window, realm/room integration or desktop default.

The Metal adapter registers canonical synthesized WAV bytes before decoding;
only a requested Coax effect retains their PCM. Actual frozen ordinary startup
observes all 120 selected variant fetches, ambient bed and room ready under the
unchanged shared allocation limits, then zero retained words after retirement.
Normal and instrumented receipts pass an 80-mutant authority. The prior eager
allocation failure is retained, as is the independent default Ogg sample-parity
failure; synth-only byte parity does not qualify those compressed assets.

Linux and Metal compile the same portable native socket workers. Shared
Coax leases precede retained event/command copies; a full inbound queue
blocks its reader, and outbound refusal produces error/close. Engine
contexts fence callback bytes through their final lease; the four physical
workers are process-owned. Actual Linux physical-pressure and ordinary
realm 79-row transport traces pass normally and instrumented. Fixed terminal
owner cells are separate scalar retention from the 64 leased event nodes;
kernel/browser/SDK buffers remain separate observations. Browser v1 now uses
the same checked lease/byte authorities before retained copies. Actual
ordinary-host pressure covers a blocked main thread, 64 events, four inboxes,
Unicode no-prefix refusal, 64 outbound commands and replacement. A real
unread peer observes 32 MiB SDK buffered bytes independently from application
leases. The local Worker holds even empty commands until real dispatch ACKs;
64 actual room-worker acknowledgments release every lease. Pressure and
room-completion observer gates kill 90/16 mutants without equivalents. Hidden
browser/SDK/OS queues remain outside those application storage claims;
complete world/default qualification remains separate.

Version 1 browser bank and packet consumers call the shared span/registry
authorities through a separate service reactor before forming byte views.
Actual six-mode metadata/source/packet failures retain later valid assets,
drain once after message handoff and leave no pending packet; all 54
observer words pass and all 62 root mutants are killed without equivalents.
Pre-contract adapters retain their existing behavior. Global browser PCM
retention and socket pressure remain separate qualification exits.

Browser audio offers hold shared Coax inbox leases before readiness or
decode continuations retain their independent encoded lines. Actual SDK
and held-module product receipts admit 38 and eight observed fields;
their 46/16 root mutants are killed without equivalents. The real product
holds 64 six-byte offers, refuses the 65th and returns all leases after
delivery while its pre-gesture audio clock stays held. Global PCM and
complete old-node retirement retain their separate gates.

Canonical browser WAV installs reserve shared PCM tickets before planning,
transfer/input copies and decoded output. Temporary reservations release
after the actual Worklet reply; retained samples release only after that
processor clears borrowed views/assets and acknowledges cleanup. The
concrete AudioContext closes before replacement installation. Actual
product and refusal/cancellation observers kill 18 and 28 root mutants
without equivalents; pending decoded results cannot install after cleanup.
Default Ogg codec ownership and complete host qualification remain open.

The shared lifecycle adapter records monotonic microseconds before handing
the unchanged delta to the producer. Actual browser, sanitized Linux realm
SDK and Metal SDK receipts admit the same ten frame/key/blur rows, including
held-input release while hidden. Its clock, semantic and JSON root gates
kill 80, 104 and 39 mutants, with no equivalents. This normalized SDK trace
does not substitute for ordinary window integration or session recovery.

The v1 Wayland window consumes the compositor's actual bounded XKB keymap
and serialized modifier state. A separate checked Coax companion projects
named keys, Unicode scalars, modifiers and scroll units into the existing
host ABI. Physical modifier transitions include the event itself, while
keysym-to-Unicode avoids Control's SDK control-character conversion. Text
fields receive committed UTF-8 once; local compose emits no intermediate
key presses. High-resolution/discrete wheel facts produce one frame offer;
finger travel keeps its pixel units. An implicit pointer grab retains a drag
outside the surface. Actual production-listener receipts require all 53
rows/212 fields normally and under ASan/UBSan/LSan; SDK ownership closes to
zero. Policy/observer root gates kill 201/219 mutants without equivalents.
This SDK component does not qualify external IME, clipboard, the complete
ordinary window or three-host/default composition. Exact scope and inputs
are in `docs/verification/host-contract-2026-11-05/wayland-input.md`.

The Wayland text companion admits complete bounded UTF-8 commits, never
preedit key events or a clipboard prefix. A text-input done serial remains
eligible within its current enable even when the compositor has older client
state; earlier-field replies cannot publish. Clipboard input is at most 2 MiB
with one five-second transfer; four retained sources at most 8 MiB total and
four nonblocking sends bound outgoing ownership. The checked input epoch and
engine identity invalidate delayed completion across focus/field/geometry,
pointer or navigation boundaries, hiding and replacement. Local compose
cancels on those same boundaries. A cancelled outgoing source remains owned
until its actual send ends; closed pipe readers do not terminate the host.
The production SDK/descriptor/window-callback component record is
`docs/verification/host-contract-2026-11-05/wayland-text.md`; it does not claim
an interactive third-party input-method session or complete window parity.

Canvas text consumers may receive a whole Wayland commit/preedit transaction
after `done`, separately from committed key text and clipboard paste. Cursor
indices are admitted UTF-8 byte boundaries (including a hidden cursor); a
bad index refuses the entire transaction. The component editor keeps preedit
out of the application draft, preflights combined capacity before applying a
commit and next composition, and restores the original selection on an owner
boundary. Single-line fields reuse the composer's horizontal scroll and hit
geometry. An oversized buffer edit changes neither undo nor redo. Native
allocations explicitly construct Coax ownership state; zero-filled C storage
does not run field defaults. Controlled Vulkan and actual Wayland protocol
SDK checks cover these rules normally and instrumented, without claiming an
external compositor/IME or screen-reader ceremony (#13645).

The shared WebGPU renderer has one physical mutable frame-buffer set. Its
availability is checked before producer advancement, and its busy state
waits for both validation and actual queue completion. Completion counts
belong to their GPUDevice; retired renderers cannot acknowledge replacement
uploads. The real held-fence/device-recovery receipt preserves producer
state across skipped offers and a nonempty new upload prefix across a late
old completion. All 21 receipt-authority mutants are killed; source fencing
passes. Ordinary 8-bit Vulkan parity and complete host qualification remain
open, so the desktop default remains web.

The Vulkan presenter reserves its actual completed frame slot and WSI image
before producer advancement, with zero-timeout acquisition. The ordinary
loop keeps platform/network polling outside that wait; opted-in faces hand
the shared Coax offer's delta through without another clamp. An unused image
reservation consumes its acquire semaphore before resize or close. The real
Wayland SDK probe observes refusal without a producer call or clock change,
presentation, unused-reservation teardown and validation enabled with zero
errors/warnings; all 18 receipt mutants are killed. Full production window
and room-continuity qualification remains separate.

Version 1 Everglade layer decode and mip arithmetic belong to the shared
`metaverse-engine/render/host/host-layers.coax` authority. Browser/native compare all 38
complete chains bit-identically; actual WebGPU readback matches every one
of 53,127,800 logical bytes across ten mip levels. SDK transfer padding is
separate from logical texels. Hosts upload canonical raw bytes without
another image resize or premultiplication conversion. Pinned pre-v1 browser
images retain their earlier SDK path; the material old/new visual difference
is recorded explicitly. The layer/profile and observer root gates kill 75
and 38 mutants, with no survivors or equivalents. This asset receipt does
not qualify the remaining display/default decision.

Version 1 still-image media consumes the original compressed bytes
through `metaverse-engine/render/host/host-image.coax`, retaining natural rectangular extent
and exactly one shared premultiplied byte round trip. Metadata bounds source,
workspace and output before the SDK allocates them; a failed decode never
publishes a successful prefix or falls back to a platform decoder. The SDK
uploads the returned RGBA bytes directly and retains them for recovery.
All 47 decoder root mutants are killed without equivalents; actual native
readback matches all 64,512 bytes of the seven authored JPEGs before and
after recovery. Browser/native actual GPU bytes match that entire bank;
the historical browser bytes match them too. The old native SDK's 3,830-byte
failure is retained. Pre-contract faces retain their earlier SDK path. The
render media composition authority remains `host-media.coax`; this decoder
does not replace its readiness, URL, extent or physical texture admission.
The browser observer root gate kills 87 mutants, with no equivalents.
The checked `metaverse-engine/host/media-notice.coax` formatter atomically creates the
existing607 ready/failed message and kills all 131 root mutants. Native
dispatch follows the producer/encoder return. A missing or refused image
becomes not-ready with zero notification dimensions, publishes no partial
texture and leaves unrelated world rendering active through recovery.
The separate browser reactor projects measured rows through Coax before copying URL
storage and publishes no truncated notice. Actual delivered browser bytes
match the formatter for all seven ready images and a failed first image,
before and after device recovery; six further submissions complete in each
case. Wrapper, semantic-observation and JSON root gates kill 54, 29 and 61
mutants without equivalents. These service receipts do not promote the
disabled presentation iteration or change the desktop default.
Image byte parity and the separately measured disabled presentation path do
not enable a desktop default or substitute for the remaining complete host
services.

Linux live media uses the same canonical image cache and Coax bundled-path
descriptor. The SDK checks containment of the actual opened descriptor before
reading its exact stable regular-file extent. Missing, malformed, late-decode
and symlink-escape inputs fail independently; six other images remain drawable.
Actual RTX readback matches all 64,512 image bytes before and after physical
device replacement, retaining the CPU cache and releasing every old/closed
ledger handle. The 48-mutant observation gate and all 58 actual fields pass
normally and under ASan/UBSan/LSan on Lavapipe. NVIDIA's instrumented ICD
initialization refusal remains explicit. These component receipts do not
substitute for ordinary Vulkan world/service integration; see
`docs/verification/host-contract-2026-11-05/linux-live-media.md`.

Version 1 media command packets expose their actual physical allocation via
`coaxial_media_capacity`. The shared `media-packet.coax` admits empty packets
without storage; nonempty packets require that export and the complete
measured span before a view or parser call. Refused bytes produce an empty
media request set and no derived readiness notice. Browser component receipts
observe all 40 fields through the real shared-renderer entry point; policy
and observer root gates kill 4/47 mutants without equivalents. Metal's actual
reader/parser passes the same 40 fields normally and under ASan, with no
parser call on refusal and eight continuing producer frames. See the host contract's
`media-packet.md` and verification `media-spans.md`.

Version 1 browser action readers require the selected outbox's measured
physical capacity before a byte view, decode or effect. The additive
`coaxial_outbox_capacity` reports the actual bar/place/city array extent;
empty actions need no storage. Missing, short, null or out-of-address-space
nonempty packets produce no effect, acknowledge exactly once, then deliver
604 outside that producer call. Actual ordinary-host receipts check all 110
fields across ten cases, including no prefix, acknowledgment/error order and
a later producer frame. Shared span and independent observation root gates
kill 4/117 mutants without equivalents. Native adoption remains a separate
host integration gate; see the host contract and
`docs/verification/host-contract-2026-11-05/action-packets.md`.

The opt-in Mac Metal host's final 85c producer snapshot runs ordinary gameplay
at its observed 120 Hz display rate, with same-seat GPU rebuild and engine
reload, fresh gesture ownership, and final GPU/CPU/audio retirement. Actual
current browser-to-Metal and direct native90-to-Vulkan comparisons pass the
unchanged live allowance; nonzero strict failures remain recorded. Its final
ASan run includes all four genuine producer C units. This completes the
Mac host acceptance, not the remaining three-host/default migration. Exact
compiler, producer, shader and SDK identities remain separate in
`docs/verification/metal-host-2026-11-05/current-85c-final.md`.

An explicit native desktop session may open its ordinary web realm with one
View-menu action and return with another. Shared `world-preview.coax` owns
the destination, readiness/busy refusal and labels (24 root mutants killed).
The shell closes the previous realm before switching; actual native→web→native
observations pass the 108-mutant Coax receipt authority, retaining the same
room seat and releasing old/final GPU, decoded-image and audio owners. This
does not select native by default or opt in the public web face. See
`docs/verification/metal-host-2026-11-05/web-preview.md`.

The subsequent current Coax 0.52 qualification completes #12869's three-host
boundary. Everglade's public face declares v1 on web, Vulkan and Metal;
the OpenAgents Metaverse Engine's empty selector chooses native Metal. Explicit web selection
and the same menu preview remain available. Current source/provenance joins,
GPU handoff, normal and fully instrumented Linux window integration, and
the unchanged preview/audio/transport component receipts are recorded in
`docs/verification/host-contract-2026-11-06/current/README.md`. Host selection
never selects different HUD, game rules or saved-document semantics.

The new admitted 3f8 compiler regenerates shader products byte-identical to
the frozen graphics corpus. Actual current17a browser, Vulkan and Metal
adapters admit the new package identity; complete displayed and direct
native/Wasm/Vulkan composition pass the unchanged live allowance, retaining
all nonzero strict failures. This clears the scoped graphics admission exit,
not ordinary service/default integration. The standalone shared presentation
mode and typed selector kill 5/2 root mutants; eligible invalid requests
refuse rather than fall back. No ordinary adapter or realm is opted in by
loading that standalone policy. Exact source/SDK scope is recorded in
`docs/verification/host-contract-2026-11-05/graphics-admitted.md`.

The ordinary browser renderer now projects the shared v1/profile2
presentation decision through its separate services reactor. An explicit
current17a v1 face fixture selects FP16 graph, quantized BGRA8 display and
canonical image paths with no capture override. Actual 20-case mode and
13-word held-completion/recovery observations pass, with exact media notice
delivery and unchanged served inputs. The new observer kills 34 root
mutants without equivalents; the six C/header pairs remain byte-identical
to the admitted current17a cache. This qualifies adapter selection, not the
remaining ordinary services, public web opt-in or desktop default. See
`docs/verification/host-contract-2026-11-05/presentation-v1.md`.

Host contract v1 refuses HTTP redirect statuses 301/302/303/307/308 before
publishing response bytes, including a response without Location. The shared
`http-redirect.coax` authority selects no-follow; browser effects send no
request to the destination and return one `0\n` to answer-bearing owners.
Pre-v1 browser behavior remains version-scoped. Actual 45-request physical
observations, the retained JSON-projection failure and corrected immutable
receipt pass the 54-mutant checker; policy kills 17 mutants. The exact
existing local-room `?re=1` reconnect spelling is admitted by the 60/11
route gates; other queries refuse. The ordinary Linux helper separately
passes exact original headers/bodies, 30 v1 failures, five legacy controls
and zero follow-ups normally and under ASan/UBSan; its receipt authority
kills 50 root mutants. Complete integrated adoption remains separate. See
`docs/verification/host-contract-2026-11-05/http-redirect.md`.

## The indexed KV arena preserves the durable v1 contract

`coax/lib/bitcoin/kvskip.coax` is an optional native representation behind
`kvstore.coax`. Its signature, SipHash layout seed, 24-level relative links
and unreachable old records are memory state, never durable records. Both
representations emit identical v1 headers, batches, masked values, checksums
and ordered snapshots. Reopening a snapshot rebuilds the arena; `used`
includes its allocated control/tower/record extent. Iteration begins at zero
and continues only with returned offsets. C offsets identify actual keys
and values, including the zero-cursor case. Every batch preflights all record
and tower growth before applying any operation; deletion allocates nothing.

The store and arena root gates kill 733 and 260 mutants respectively, with
zero survivors. `kvskip.py` checks an independent ordered dictionary and
64-bit offsets; `kvskip-store.py` checks exact durable bytes and recovery in
debug/release and under ASan/UBSan. The chapter-125 oracle's `--arena` journey
checks address history, balances and both reorgs on this representation,
including every cut of the state frame. The full signet node must use
bounded incremental file reads rather than treating the small example's
whole-file buffers as a full-chain storage design.

## Explorer light state changes one durable branch at a time

`coax/lib/bitcoin/lightchain.coax`, `lightpolicy.coax` and `lightstore.coax` own the explorer's
version-1 light schema over chapter 62's store: the `L` profile/tip, `C`
coins, `U` ordered inverses and `H` parent/block-bound inverse digests.
Preparation changes only disjoint private scratch. Coins, inverse records
and final tip form one KV batch; a late failure yields no publishable batch.
Disconnect checks the complete inverse digest and each current afterimage
before restoring old coin bytes. Same-block spends reverse in opposite order.
Only a preferred path through the admitted header tree can be prepared,
and neither active header-index tip nor validation levels are changed.
The file adapter acknowledges only after append and storage sync, requires
exclusive file ownership and immutable inputs, and truncates only a KV
reader's reported torn tail. `kvhash.coax` streams large-frame SHA-256d
without changing the existing KV format. This is explicitly proof-of-work-only
tracking, never full consensus validation or authority for money. Evidence:
chapters 58/60's explorer profiles, mutation checks, and `lightchain.py`'s
independent native UTXO/MuHash, physical-cut and real Coax file-tool journeys.

## A command line's login from a browser is the browser's email login, no more

A command line (GameBuilder's `gamebuilder login`) may log in from a browser
already logged in by email, on this site or a listed one, without the
L2 that linking a device's key needs. The browser's page posts
`POST /api/auth/cli/code`; the answer is a one-time code (128 random bits as 32
hex digits, `{"state":"code","code":...,"expires_in":300}`) that the page shows
with a Copy button. The command line, with no `Origin`, posts it to
`POST /api/auth/cli/redeem {code}` and gets an app's login (access and refresh
tokens) of its own.

- Only a person's own web session from an email login makes a code: its
  credential is the session cookie, its `Origin` ours or a listed site's, its
  `via` email (`ap_cli_mint`). An app's token, a signature, an agent, another
  site, a request with no `Origin`, and a session from a key or passkey make
  none (a key's L2 never becomes something a code carries).
- Only a request with no `Origin` spends one (`ap_cli_redeem`): an app's
  tokens never reach a page, ours or a listed site's.
- A code is kept as its SHA-256, works once and for 5 minutes, and a new one
  ends the same browser session's unspent ones (`auth.cli_start`,
  `auth.cli_take`, migration 0212). A code whose browser session has ended
  since (logout, removal, logout everywhere) logs nobody in.
- The login it makes is exactly what typing the email's code into the command
  line makes: the same account and email row (`via` email), at **L1**, an app
  session bound to no device, with no key, passkey, device or other way in
  linked. It is limited (`ap_limited`) whenever the account has a stronger way
  in, as the browser's session is. Its email proof time is the browser
  session's, never now (`auth.cli_carry`), so a code is not a fresh email code
  for the routes that accept one (`al_proof`).
- So a limited session may make a code (`ap_limited_handoff`): it is the one
  login write a limited session makes, and the login it hands over is no more
  than that session. Linking the computer's key, approving a device and every
  other way in still need L2.
- Both routes count against `agent_ask`, the computer login requests from one
  network address (30 an hour).

The decisions are `ap_cli_mint`, `ap_cli_redeem`, `ap_cli_code_ok` and
`ap_limited_handoff` (`coax/web/api/auth-policy.coax`, `#authority`); the rows
are migration 0212's. Checks: that file's `#test`s;
`coax/web/api/tests/auth_checks.coax` `ac_site_checks` ("cli:": nobody, another
site, no `Origin` and an app's token make no code; the site's page makes one;
it is kept as its digest; a page, ours or the site's, cannot spend it; an email
code and an unknown code are refused; the command line spends it once for an
app login on the same account at L1, via email, with the browser's `l1_at`,
bound to no device, no key linked; the audit rows; a code from a browser that
logged out since is refused). Not covered by the suite: a limited session
making a code (the policy's `#test`s cover `ap_limited_route`).

## A limited session writes only the account chat's stateless completions and a build agent

A limited session (an email login, L1, on an account with a key, passkey,
device or unspent recovery code) reads and plays, and is refused every write
on an account, human, scope, L2 or admin route (`ap_limited_admit`), but for
an explicit list: `POST /api/chat/complete/v1`,
`POST /api/chat/world/complete/v1` and `POST /api/chat/delegated/complete/v1`,
and only while their rule is `human` (`ap_limited_write`, `ap_limited_route`,
`coax/web/api/auth-policy.coax`, `#authority`, mutation-checked). The default
is deny: a route not named stays refused. Each named route stores nothing,
changes nothing the account owns or how it logs in, and asks the model with a
free key (`account-chat.c`, `inference_ask(..., true)`), so it counts only
against the account's own daily inference quota and never reads or charges
credit, sats or a wallet. The route's own rules still apply (the funding
rule, `act_funded_from`: the team's, or any person's account when the first
route is asked from a listed first-party site; the body's shape). Saving a thread, binding a player, project chats,
settings, identity, the wallet, agents, exports, deletion and every
authenticator change stay refused, and recovery still waits 72 hours. Its one
login write is a command line's code (`ap_limited_handoff`, the section
above), whose login is no more than the session. From a
listed first-party site only the first route is reachable. Checks: that
file's `#test`s; `coax/web/api/tests/auth_recover_checks.coax` (a limited
session reaches the three routes' own answer; nine other ordinary writes,
linking a key and logging out everywhere are refused as before).

The one stored write on the list is `POST /api/gamebuilder/agents/v1`
(2026-11-17): it starts a GameBuilder build agent, one durable text turn in the
account's own durable chat (`gamebuilder-agents.c`, its body made a
SubmitRequest by `gba_submit`, `gamebuilder-agent-wire.coax`), asked by the
hosted runner with the same free key and the same daily quota, with no tools:
its only effect is text the page reads. It changes nothing the account owns
beyond that conversation and nothing of how it logs in. The durable submit
admits a limited session only where `ap_limited_write` names the request's own
path, so `POST /api/chat/submit/v2alpha1` stays refused to it.

## A GameBuilder build agent is any person's own account's

`POST /api/gamebuilder/agents/v1` starts a build agent for any logged-in
person's own account, on the team beta or not (owner, 2026-11-17, GameBuilder
issue 15). Nobody logged in is 401; an agent, a guest or any other kind of
principal is refused as before (`ctw_owns`). Every other durable turn
(`POST /api/chat/submit/v2alpha1`, Project turns, native tasks started from
them) keeps the team's funding, 403 without.

A durable turn is admitted twice: at its start, and again when the hosted
runner takes it up (`dispatch_turn`, `account-chat-turns.c`) and at each
native effect (`native_guard`, `account-chat-tasks.c`), where no route and no
`Origin` is known. So the turn itself says what it was admitted for:
`chat.turns_v2.admitted_for` (migration 0214) is `'gamebuilder-agent'` only
when the turn was started through the build agent's route, which writes that
word itself (`gamebuilder-agents.c`); it is `''` for every other turn and for
every turn stored before, and the column's check allows nothing else. Both
admissions ask the same decision with the stored word:
`act_funded_turn(kind, account, team, admitted_for)`
(`coax/web/api/account-chat-policy.coax`, `#authority`, mutation-checked): a
person's own account, and the team flag or that exact word. Nothing is
guessed from the conversation, the input or the caller.

What the word opens is one build agent turn and nothing else: its request is
made by `gba_submit` (a new conversation, the game, the job and the brief
within their sizes, no tools, no Project), so the conversation it makes cannot
be continued by an account off the team (the continuing route is the team's).
The limits are the durable turn's: at most 32 queued or running turns an
account (`ctw_queue`; the 429 says the number), one model turn of at most
2,048 output tokens, the free key (no credit, sats or wallet is read or
charged), the account's own day (500 requests, $2.00; `inference.request`)
and the start body's sizes (`gba_submit`). A frozen or deleted account is refused at both admissions
(`ct_admit`, `ctw_grant`). The route has no `Origin` rule of its own beyond
first-party (`auth_first_party`: none, this site's or a listed site's), so the
command line and this site's own pages may start one for a person too: the
rule is the route's, not gamebuilder.org's.

Checks: `account-chat-policy.coax`'s `#test` (the exact word, a person's own
account, the team unchanged); `coax/web/api/tests/durable_chat_checks.coax`
`dc_open_agent_checks` (an account off the team: a durable chat turn is still
403; a guest is 401; a build agent starts queued and its turn is stored as
`gamebuilder-agent`; the runner claims it and its dispatch grants it, shown
with the claim expired first so no model is asked: the answer is "Claim
expired before dispatch." and the turn is untouched; the same start with the
stored word taken away is ended at dispatch with "Account admission changed
before dispatch" and never started) and `dc_gamebuilder_checks`. Not
covered by the suite: the native-effect guard for a build agent's turn (a
build agent has no tools, so it makes no native effect), and a real model
answer for an account off the team in production.

## Anime words become directions only through the catalogue holder

`POST /api/playtest/labs/anime/direct` admits a site session with matching
Origin or a fresh, payload-bound NIP-98 signature. A foreign Origin is refused
before signature first use. Coax owns input bounds, the selector schema,
catalogue holding, full probability maps, confidence dispositions and quota
decisions (`coax/web/api/anime-director-policy.coax` and
`anime-route-policy.coax`, both `#authority`). The host observes identity and
ledger counts, reserves one call under advisory transaction lock 3073002, then
commits before model work. No words or scene state enter `anime.calls`.

One malformed answer refuses the whole selector result without publishing a
prefix. Every selected finite field has a complete probability map on the
0.001 grid summing to one, and its selected value is the catalogue's first
maximum. The minimum selected score is `p`: above 0.85 plays, 0.5 through 0.85
is soft, and lower scores are heard only. Out-of-stock or statically unsupported
directions remain heard only, preserving their scores; these model-reported
probabilities do not assert empirical calibration. The route returns timing
intent but does not mutate a stage or schedule a take. Checks:
`coax/web/api/tests/anime_direct.coax` against the native API and PostgreSQL,
the two authorities' mutation gates, and the finite render-profile checks.
Contract: `docs/metaverse/2026-11-02-anime-director-api.md`.

Recorded `anime.v1` takes enter through `metaverse-engine/anime/take.coax`
(`#authority`): bounded JSON, catalogue fields, exact thousandth confidence,
ordered line intervals and a bounded timeline are checked before replacement.
The accepted state owns its source/word bytes and numeric offsets; no view into
a transient host message survives. The page accepts canonical unpadded
base64url in `take64` or raw message 701. A refused initial request displays a
refusal with no demo fallback; a refused replacement preserves the accepted
take and clock. An absent take keeps the built-in session.

The API and take holder share `metaverse-engine/anime/render-profile.coax`
(`#authority`). Slot 6 has no stock body in `draw_cast`, so directions relying
on that body remain heard, as do unsupported effects/poses and every arbitrary
`make`. Recorded words and heard-only answers remain caption data. They cannot
activate the demo's fixed dragon or claim an asset was made. Only admitted
executable answers enter the unchanged `anime_timeline` grammar. The format
caps documents at 32 KiB, lines/answers at 16 and duration at 600 seconds. The
declared frame budget is unchanged; transport buffers carry the full document
and cache keys include the full URL.
Checks: `metaverse-engine/anime/take-checks.coax`, the two authorities' mutation gates,
`apps/openagents-web/animelab-take-checks.coax`, and the headless
`coax/web/anime-take-probe.coax`. Format: `docs/metaverse/anime-takes-v1.md`.

The Anime CLI holds a complete director reply before appending a written line.
It reuses the API's probability holder and the take's catalogue gate; one bad
answer refuses the whole append. Cooperating writers compare the original
take under an exclusive sibling lock, then rename one complete checked file.
`play` requires the loaded page to report an accepted take before screenshots
or recorded frames are written. Literal arguments cannot become profile,
origin or report flags. Policies: `apps/openagents-cli/anime-rules.coax` and
`anime-args.coax`; native journey: `coax/web/anime-cli-probe.coax`.

## A presence is an agent's only when it is signed or a seat's

A row in a room is `agent`, shown to everyone with `{"agent": true}`, only
when its presence came from an open seat's own player key (0110) or was
signed with NIP-98: `room.c` verifies it
(`access_signed`, `caller-policy.coax`) and acts with the account's player key
(`auth.caller_key`); a forged, unbound or replayed signature is 401, never the
player key. An automation mark that says "seat" without a signature is plain
automation and stays seen only by itself (0069). A person's page posting
unsigned clears the flag. A seat's own page, which it names, isn't shown that
seat. Checks: `coax/web/api/test_room.py` (signed seat seen and flagged with
its account's name, its page not shown it, forged mark unseen, forged
signature 401, 40 s fresh, gone at once).

## A seat's key is its account's, and only while the seat is open

`POST /api/playtest/seats` opens a seat only for a verified NIP-98 caller
(`coax/web/api/seats.c`; caller-policy.coax) or, unsigned, this site's own
logged-in page (its session, `__Host-oa_session`, a POST only with this site's Origin: the
YOUR AGENT pane, `metaverse/edit/agent.coax`), never for the player key, and only where
`coax/web/api/seat-policy.coax` (`#authority`) admits it: a made realm the
caller may see, fewer than 8 other seats open on the site, and for a hosted
seat a configured runner. Opening one closes the account's open seat. The
seat's player key (migrations/0110_realm_seats.sql) belongs to the caller's
account, so every rule meets it as that account and nothing more; a site
admin's open seat also passes `verse.realm_hidden`. A seat not kept for 10
minutes (seat-policy.coax's `sp_live`), closed by its account, or replaced
has its key hash changed so the key never resolves again. A hosted seat's
key goes only to the runner, on its oa-seat's stdin; the caller never gets
it. Checks: `coax/web/api/test_room.py` (unsigned 401, unseen realm 404,
replaced key dead, private realm for owner and admin seat, keep and close by
the account only, idle close at 10 minutes and not before, 8 a site, the
hosted key only to the runner; a logged-in page opens, acts in and closes a
hosted seat, while its session from another site's page, a POST without this
site's Origin, an unknown session and the player key are refused),
`seat-policy.coax` (0 surviving mutants).

## Only a root admin changes who is an admin

A root admin is a key in `coax/web/api/admins.conf`, changed only by a deploy.
A database admin (`access.admin_keys`, `migrations/0100_admin_keys.sql`) is a
site admin for every check, but only a root admin adds or removes one
(`access.admin_may`), no request removes a root admin, and every change is a
row in `access.admin_audit`. Checks: `coax/web/api/tests/cli_auth.coax`
(`admin keys`: a non-admin and a database admin can't add; a database admin
can't remove a root, nor can a root; a root adds and removes; the removed key
is no admin).

## A player's progress save is kept only from the revision there now

Each progress slot (`verse.progress`, `coax/web/api/migrations/0119_progress_revisions.sql`)
has a revision. `POST /api/playtest/progress/<world>/<name>` names the
revision it was made from (`rev`), and the server keeps it only when that is
the revision there now; otherwise it answers 409 with the revision and the
saves there now, and changes nothing. No decision reads a device's clock: a
kept save's time is the server's. `coax/web/api/progress-policy.coax`
(`#authority`, 0 surviving mutants) decides; `player.c` gathers the facts
under the slot's lock and SQL does what it says. Transition rule: a save that
names no revision (a page built before 0119) is kept only while no page with
revisions has saved the slot (`marked`, which never clears). A change that
names no revision (a sign-in moving a guest's saves, an erase) gets a
revision no device has read. On a 409 a page of progress takes the server's
save; a page of where you are or the spell bar sends its own again from the
server's revision (`metaverse-engine/state/save.coax` `SaveSync`). Server-owned
room state (`verse.room_vitals`: HP, XP) is not a progress save: only
`oa-room` writes it, signed with the seat-ticket key or the writing shard's
own key (room writes v1, #13260), and XP only rises,
also when a guest's row follows them into an account (0119 `vitals_follow`).

## A room holds no key that signs a ticket or another shard's writes

Once the room keys are rolled out (`docs/deployment/room-keys.md`, #13260),
a room shard holds the API's ticket public keys and its own write key, and
nothing else. A seat ticket (v5, `metaverse-engine/session/seat-ticket.coax`) seats only
when the key its ID names verifies its BIP-340 signature over every byte
before the signature, so a room can't mint a seat, the admin bit included. A
room write (`coax/web/room/room-write.coax`) checks only under the key the API
derives for the shard it names, over its purpose, shard and body, and the API
refuses a directory report, stats or vitals post that names another shard,
and an item or checkpoint write for a player whose newest owner generation
another shard's ticket holds (`verse.room_owners.shard`, migration 0156; an
older generation is fenced as before). Two keys of each kind overlap only during a rotation. Until the shared
`OA_ROOM_TICKET_KEY` is removed (the rollout's last step) the older forms are
still taken. Checks: `seat-ticket.coax` and `room-write.coax` (`#authority`,
mutation-clean), their fixture profiles (`proto/verse/seat-ticket-v5.json`,
`room-write-v1.json`) and `coax/web/api/tests/room_checks.coax`
`rc_write_keys` and `rc_write_owners`.

Every seat ticket's bytes, every version, come from `seat-ticket.coax`:
`seat_ticket_v5_write` for v5 and `seat_ticket_hmac_write` for v2, v3 and the
v4 envelope (#13248). `coax/web/api/player.c` calls them through
`ticket-sign-unit.c` and no C file builds any part of a ticket. Checks: the
same file's tests, mutation-clean, and `seat-ticket-hmac-profile.coax` on
`proto/verse/seat-ticket-hmac-v1.json`, which the room's `tk_check` reads.

## Room interest gauges describe picks, not delivery

`room-stats.coax rm_st_interest` (#12840) aggregates the actual last production
interest pick of each live, placed seat: whole, health-only, blocked and hidden
categories. Totals and per-category maxima are process-wide gauges; the named
worst room/seat has the largest whole-plus-health selection, ties first. They
do not count transmitted rows, accepted saves or presented outcomes. The native
vitals report labels their scope and keeps existing seat records unchanged.
Checks: room-stats.coax's category, liveness and production-path cases.

`frame-observe.coax` counts every application frame actually accepted by a
native socket queue. Complete frame bytes include WebSocket framing (and the
client mask on input); D-header and entry bytes partition that complete D
frame exactly, with no one-in-four extrapolation. Queued bytes are not claimed
as delivered. Actual kernel writes, refused frame queues and bytes discarded
on close are separate native effects. Counter saturation marks the observation
degraded. Serialized rows and deferrals are read from the production v3
serializer and labeled before relay admission, with their own worst room/seat.

Accepted-save metrics advance only at the producer's exact receipt boundary.
Checkpoint revision, process, API acceptance time, capture age and local receipt
age stay separate from the latest pending capture and retry counts. A late or
failed post cannot advance them. Legacy expiry, eviction, stale/refused receipts
and historical bare success cannot create a durable revision; legacy receipt
times use their declared wall clock while profile 2 ages use monotonic time.
These observations do not alter ownership, persistence admission or loss bounds.
The separate server-queued ACK sample ends only when the final ACK frame is
accepted by the socket queue; it excludes estimated network RTT. The older
input-to-ACK estimate retains its historical RTT term. Client presentation
remains a separate measured boundary. Report truncation is explicitly counted.

`rm_st_inputs` (#12843) reads pending simulation inputs at report time. Its
process total, largest seat queue and oldest pending age are current gauges,
not maxima between report windows. A timestamp ahead of the monotonic clock
makes the age unknown (`-1`) and increments the unknown-age count. Credit drops
are cumulative for the current live seats; they are not cast refusals or
transport losses. The raid consumer's final whole-D ACK timing matches the
exact input sequence and ring generation, excludes missing/backward samples,
and labels its 2000 ms overflow bucket. Neither observer admits gameplay.

Capacity observations distinguish native world slots, connected seats, grace
holds and initial gates from API placement caps and outstanding player/invite
reservations. Startup readiness is the condition used by `/ready`. The admin
placement view credits it only from a report younger than 30 seconds whose
process generation matches the directory; missing, obsolete and expired
reports mean unknown. A draining process may be startup-ready while placement
refuses it. Reservation counters count completed database requests, including
retries, at their returned outcome. They neither change placement nor imply
unique players or durable gameplay acceptance. Failure to persist an
observation is logged and never replaces the original reservation result;
persisted counts cannot account for an unavailable database. Its independent
transaction has a 50 ms server statement budget and rolls back on failure;
the local timeout cannot leak into ticket issuance. This is not a network
deadline for an unavailable database connection.

The separate opt-in latency observation v1 has no gameplay queue or authority.
Its bounded kernel observes only a newly queued, exactly stepped input and a
complete D accepted by the socket queue. Clock synchronization retains an
offset interval with explicit quantization, drift and freshness assumptions;
unknown time is never zero. The codec/kernel, clock and socket-owned client
are checked and mutation-checked (`metaverse-engine/net/latency*.coax`). With `roomtrace=1`,
native and browser SDK hooks record input receipt, exact successful movement,
simulation and serialization, final D queue acceptance, WebSocket receipt,
application delivery, render submission and GPU completion. Application needs
both an ACK covering the input and the complete state tick. The captured
engine, socket and GPU device must still own the completion. Retirement and
the two-second deadline clear pending stamps; history holds at most 64 samples.
Interpolation delay stays separate. GPU completion does not prove physical
display scanout or that a particular pixel changed. Ordinary sessions allocate
no client observer. See `docs/protocol/everglade-latency-v1.md` and the scoped
records in `docs/verification/2026-11-05-room-latency-components.md` and
`docs/verification/2026-11-05-room-latency.md`.

## The room keeps the client's session lifecycle

The room server takes an intent only when it is newer than the last it
accepted and the last it queued (`metaverse-engine/session/lifecycle.coax`
`life_room_takes`), so a resend after a socket change applies once. A closed
socket's seat is held `LIFECYCLE_HOLD_MS` (`life_room_holds`, Everglade's
`EG_GRACE_MS`): longer than the client's resend window and shorter than a
long suspension. Each host's room tick has a 95th-percentile budget
(`LIFECYCLE_TICK_US`, native and browser worker) that the drift check reports
(`everglade-drift`). Checks: `metaverse-engine/session/lifecycle.coax`, `room.coax`
"relay:".

## A character travels between rooms as one body

A player's travel to another room (`T`, `coax/web/room/room-travel.coax`,
#12850) never puts two bodies in play. When the destination runs on the same
server, a seat is set aside for that player there first, or the travel is
refused and the player plays on. The source seat then takes no intents, and
its casts still on their bars stop (`e stop SS SLOT left`); a cast already
released lands. The room sends the page on (`g CODE`) only after the API's receipt covers the
seat's state at the cutover, so a crash after the route loses nothing. A
travelling seat's close ends it at once (no seat is held). The destination's
ticket takes the next owner generation, so the source can't save over it.
Each step before the destination's join is undone by time: after
`TRAVEL_MS` (`metaverse-engine/session/travel.coax` `TRAVEL_HOLD_MS`) without the page
leaving, the seat takes commands again and the set-aside seat lapses. The
room asks the travel contract each of these questions over its own state
(`travel_admits`, `travel_cut`, `travel_holds`, `travel_saved`). HP, XP and WANTED travel through the saved note. To a
room on the same server, the character's Temporary Hit Points, spent spell
slots and limited uses (onto the same build only) and its Repair wait travel
too, laid on once after its hello there; conditions, auras and zones belong
to the source room's world and end with it. Checks: `room.coax` "travel
(#12850)", `everglade.coax` "net: travel", and `room_checks.coax` travel,
which kills the source room process right after the route and then the
destination's after the arrival, kills a source before any travel, and loses
every vitals answer to a source so it never routes before it is killed.

## Versioned character checkpoints acknowledge a committed whole capture

The additive character-checkpoint-v1 receiver (`coax/web/api/checkpoint-policy.coax`,
`player.c`, migration 0131, #12838) holds the character's owner-directory row
locked through commit. Only the exactly issued owner may write; one owner binds
one process, revisions rise, and equal identities require identical captured
data (excluding retry send time). XP cannot decrease within this checkpoint
chain. Its receipt follows a synchronous WAL commit with fsync enabled; only
`applied` or `duplicate` acknowledge the capture. Legacy vitals receipts do not
acknowledge checkpoint fields. No room producer or ticket restore uses this
receiver yet; it supplies no new live crash-loss guarantee. The remaining
integration gates are in `docs/metaverse/character-checkpoints-v1.md`.

Character resume profile 2 (#12838, `docs/protocol/character-checkpoint-v2.md`)
is additive: checkpoint v1 and legacy gameplay tickets keep their identity.
Its v4 signed envelope carries the newly issued owner and the entire accepted
v2 capture. Both signatures and the saved engine/rules/content are checked;
an incompatible build or an unavailable store never falls back to a fresh body.
The profile becomes sticky when its first ticket is issued, before the first
capture: old unversioned vitals cannot write it. Identity and owner stay locked
through both legacy writes and checkpoint acceptance. Owner transfer reconciles
the cumulative XP maximum across all stores; within one owner the checkpoint
chain is monotone, independently of asynchronous legacy reports. Linking an
account carries its accepted body and advances both ownership fences.

Each admitted profile-2 character reserves a bounded save entry. Pending and
in-flight captures are not evicted or expired; a newly issued owner supersedes
the fenced owner's unsent captures. One immutable capture retries independently
of the latest mailbox, in turn with every other entry's, so no refused or
unanswered capture holds up another character's. A generic success or legacy
receipt cannot acknowledge it. The initial body cannot play, and gets no world
state, before its first complete durable acceptance; its encounter plays on for
the others meanwhile. An encounter advances only while every profile-2 seat's
accepted capture (for a body not yet accepted, its restoring hello) is less than
30 monotonic seconds old. A pause clears queued movement
and accrual, takes no mutating intents, and keeps retrying. Crash loss is at
most 30 seconds of authoritative gameplay plus one tick; an unavailable store
has no finite wall-clock restore objective. Normal capture cadence is 5 seconds;
the healthy first-socket-state objective is 60 seconds under the profile's
declared scheduling, capacity and API latency assumptions.

Restart restores HP (including explicit zero/dead), temporary HP, WANTED,
XP, same-room place, class, slots, uses, cooldowns and the down timer. Counts
are constrained to the actual build and timers gain no offline credit. World
effects and encounter contributions end with the lost world; ordinary fresh
town/raid rules apply. Same-process grace preserves the live body. Placement
waits for the restoring hello, which applies once. Travel waits for both the
legacy receipt and its complete checkpoint. Final drain freezes once and awaits
the final acceptance; timeout reports failure. Its network helper survives a
process-group SIGTERM until the room closes its pipe. Checks: shared character
codec, flow, envelope and profile authorities; room checkpoint checks; scratch
`checkpoint2_checks.coax` native commit-edge, queue, outage and drain faults.

## Only a character's newest owner saves its room state

The API is the one directory of owner generations (`verse.room_owners`,
migration 0127, #12834): each room ticket it signs takes the character's
next generation, never wrapped, and the signed ticket carries it, so no
client names one. A room sends the generation on each vitals line, and
`verse.room_vital_apply` refuses (`f`) a line below the character's newest,
writing nothing. A line without a generation applies as before 0127. This is
the store-side fence of `metaverse-engine/session/control.coax` (`control_fence`).
Checks: `coax/web/api/tests/room_checks.coax` vitals (older owner refused, newest kept),
`coax/web/room/room-ticket.coax` and `room-vitals.coax` (#12834 checks).

A store promoted to a newer region epoch (`verse.region_promote`, migration
0132, #12853) raises every character's newest generation to at least
`EPOCH << 20` in the same transaction, and `verse.room_owner_next` never
issues below that floor, so a ticket or room write from a region the store
replaced is older at every fence. An epoch not newer than the store's, or
2048 and up, is refused. A store never promoted is at epoch 1 and issues
generations as before. Model: `metaverse-engine/session/region.coax`. Check:
`room_checks.coax region` (a replica promoted after a partition refuses the
old region's line, and its tickets start past the floor).

In the room server, one seat plays a character at a time (`room-relay.coax`,
#12834). Among tickets that carry a generation, the newest wins: a join with
an older one than a seat of that character holds on the server is refused
(`elsewhere`), and a newer one ends the older seats, in the same room (the
new socket takes the seat over) and in the server's other rooms. A vitals
line answered `f` with no newer ticket seated ends that owner's seats too,
and the room then refuses that owner's tickets; a seated character's line
goes at least every 30 s, changed or not, so a resumed process hears `f`.
Every socket that loses its seat this way hears `r 0 elsewhere`, and the page
then opens no socket on its own (`everglade-net.coax` `net_elsewhere`), so
two pages that rejoin by themselves can't take the character in turns. A
seat of an older owner never notes vitals over a newer owner's. Tickets
without a generation keep the earlier rule (a second tab sits beside the
first). Checks: `room.coax` "owner (#12834)", `everglade.coax` "r 0
elsewhere", and `room_checks.coax` owner, which pauses and resumes a running
room process.

## A game item changes only in the ledger's one transaction

Every item, each character's inventory version and every applied operation
ID live in one store (`verse.items`, `verse.inventories`, `verse.item_ops`,
migration 0130, #12849). The rules have one owner, `metaverse-engine/session/ledger.coax`
(`#authority`; decision [0006](docs/decisions/0006-item-ledger-rules-in-coax.md),
#13318): its in-memory store and the API's route (`player.c` `room_items`
through `coax/web/api/item-policy.coax`) both ask `ledger_gate` and
`ledger_rule`, and SQL decides nothing (`verse.item_apply` is gone, migration
0165). The route reads the facts and writes the answer in one transaction,
under one lock an operation ID, the item's row and the inventories' rows in
one order: the same ID and payload answer the recorded result (refusals too),
another payload under an ID is `conflict`, an ask first sent a day
(`LEDGER_KEEP`, ms) ago or more is `expired`, a generation below the
character's newest owner (`verse.room_owners`) is `stale`, a version other
than the inventory's is `version`, a version with no next value is
`exhausted`, and an escrowed item moves only by its settle or cancel. A
`consume` uses an item up, and a `claim` changes no item (#13291). An
operation can carry a claim on the character's record (a quest's stage from
one value to the next, coin, XP, an item worn; `char_claim` in
`everglade/model/character.coax`): the route applies the operation and its
claim in one transaction, or neither, so a reward is paid once. A page can't
write an item, coin, a quest's stage or a reward. Each
change appends its `verse.item_outbox` entry in the same transaction, and
replaying the outbox gives the same items, owners and versions
(`verse.item_reconcile`). Only a room-key-signed post reaches it. The ledger
holds game items only, never money. Checks: `metaverse-engine/session/ledger.coax` and
`coax/web/api/item-policy.coax` tests with `coax mutate` at zero survivors,
`coax/web/api/tests/room_checks.coax` items and item ops, and
`coax/web/room/room-items.coax` (1,000 fault-injected plays).

## Room rules never ship in a page

The rooms decide the game and the page draws it. They build separately: a
page's load closure has no file under `coax/web/room/` or `everglade/server/`
except a wire codec both ends speak (`X-wire.coax`). The proof is
`coax/web/tool.sh layer-check`'s `page-room` gate, which runs before every
push, covers every module `apps/openagents-web/build.sh` builds with
`wasm.sh` plus the native clients, and has no allowlist (#13440). A page
that needs a room's rule moves the rule to `everglade/shared/`, `rules/` or
the engine; it never loads the room.

## Every moderator action is logged before it is done, and the log is never edited

A ban, a mute, a kick and a moderator's look or move have one set of rules
and one codec, `metaverse-engine/session/moderation.coax` (`#authority`; #13302,
[moderation v1](docs/protocol/moderation-v1.md)). A room asks them, and the
API asks them through `coax/web/api/moderation-policy.coax`; SQL decides
nothing (migration 0170). Only a moderator acts: a seat whose signed ticket
has the moderator bit, or a caller the API finds among the realm's admins.
A moderator never acts on themselves, so none lifts their own sanction. A
banned or kicked account gets no seat ticket, and a banned or muted one
says no chat line and sends no inbox message, while the kept row holds. A
room takes an action only after its line is in the log's outbox, and
refuses the command when the outbox is full. The API writes the log row in
the same statement as the sanction it sets or lifts, once for each
operation ID (shard, generation, sequence). `verse.mod_log` is append-only:
`UPDATE`, `DELETE` and `TRUNCATE` raise for every role. Checks:
`metaverse-engine/session/moderation.coax` and `coax/web/api/moderation-policy.coax`
tests with `coax mutate` at zero survivors, `room.coax` "moderation",
`coax/web/api/tests/room_checks.coax` moderation, and the profile check
`coax/web/room/moderation-profile.coax`.

## A critical counter moves only through a named integer operation

Owner generations, the ledger's inventory versions, entity slot generations,
the room's seat generations, Everglade's outcome counts and its LIFE/WARP
numbers are `#counter` fields (#12927). Plain `+`, `-`, `*` or a compound
assignment on one is compile error E0840; each moves through `add_checked`
(an optional the caller must handle), `add_sat` or `add_wrap`, which give the
same result in debug, release, the evaluator and native C. A ledger write
that would advance a version past the largest int is refused
(`LEDGER_EXHAUSTED`) before anything changes; a control generation at its
last is `CONTROL_EXHAUSTED`; a seat or owner generation saturates, never
wraps to an older one. Checks: `coax/tests/integer_ops.py`,
`metaverse-engine/session/ledger.coax` "a version at the largest int".

## A party roster carries health only, to its members, never across a block

A party (`verse.parties`, migration 0129, #12852) outlives rooms and
sessions; its roster (`GET /api/playtest/party`, `verse.party_view`) goes
only to a member, by their player key, and lists each other member's id,
name and HP from the room's own report (`verse.room_vitals`, so the newest
owner's), nothing of a member's place, room, look or the room's simulation.
HP older than ten minutes is no HP. A member who blocks the reader or whom
the reader blocks, and a hidden member, are left out, and a player blocked
either way by a member can't join. Every change of membership or hiding
raises the party's version, and a roster lasts a minute (`until`). Placement
by party (`verse.party_room`) never seats anyone past a room's seats: a full
party room is named to the page (`party_full`) and the page goes elsewhere.
A page whose room came from the realm's list asks its ticket for the realm,
so placement by party applies; a room a link names, or a friend's INVITE,
stays the player's choice. A failing party service never blocks a ticket:
placement goes on without the party. The page's PARTY pane shows only what
the roster carries. JOIN on a member (#12911) asks a seat ticket for the
realm naming that member (`?party=`, `verse.party_member_room`, migration
0134): the ticket places the asker in the member's live room only when both
are in the same party, neither blocks the other and the member isn't hidden;
otherwise it says `party_away` and places as usual. The ticket names the one
room it seats; the roster still never names a room. The rules are one
contract, `metaverse-engine/session/party.coax` (`#authority`, mutation-checked): the
API's SQL keeps its durable form, the page takes its member limit from it, and
the room server's health-only channel for an out-of-range party member asks
`party_sees`, so a member who blocks the seat, or whom it blocks, gets
nothing. Checks: `coax/web/api/tests/room_checks.coax` parties
(with `ROOM_BIN`, a real room's crash, restart and the service down),
`everglade5e.coax` "party:", `metaverse-engine/session/party.coax` and `room.coax`
"interest".

## Spatial values keep affine role and nominal identity

The initial native-C `Point3`/`Vector3` types contain three raw coordinates,
with no scalar-carrier erasure or stored frame/unit tags. Affine operators
require matching nominal frame, resolved unit and carrier; minimum frame
precision is checked. Point units have physical length dimension. Only vectors
scale by a matching raw float; a dimensionless Quantity is not a raw float.
Whole operands are captured once in source order, including aliasing reads.

Mutation-only preparation asks Coax for an alternate with the exact result
type. Supported binary token changes redirect one source call, retaining its
arguments and leaving shared helper bodies untouched. Point differences,
vector-plus-point and scalar-left products have no paired alternate in this
profile; unary negation is unsupported. Their separate inventory is not a
check kill or an equivalence waiver. Exhausted preparation capacity refuses
before a partial mutation score. Normal compilation prepares no alternates.

Raw component access/reconstruction remains an explicit boundary. Frame
metadata establishes no realization, correction epoch, sensor truth or
transform. Schema1 spatial reflection and device output are refused in this
profile. Shared compiler-instance capacity and measured debug helper calls are
explicit limits; this does not complete R12 or prove bounded robot control.
The first Spacecam consumer preserves the public D3/SpaceCam layouts and
subtracts F64 points before narrowing. Its boundary assertion is not an
end-to-end migration of every world coordinate.

## Nominal integer domains require explicit conversion

A `struct @id` holds one concrete integer field named `value`; phantom generic
parameters preserve separate domains.
Only the same nominal domain compares for equality; `@id(ordered)` also
admits ordering. Construction and `.value` extraction are explicit. Integer
arithmetic and numeric constraints never implicitly accept a wrapper.
Checked and saturating arithmetic operate on its extracted integer;
`#counter` continues to restrict writes to the integer field. Generic keys
and read-only views preserve the domain. The checker lowers comparisons to
field projections, evaluated once in source order by the evaluator and C.
Generated C uses distinct records with the integer's size and alignment;
its new `coax-c-id-lp64-v1` header schema includes the ordering opt-in.
Existing header schemas retain their identity. Device output refuses E0962.

`metaverse-engine/core/ids.coax` defines the shared seat, entity, character, item and
owner-generation domains. Codec adapters construct/extract them; a decoded
wrapper grants no liveness or permission. Entity liveness requires a
canonical `Handle(Entity)` word and the live slot's generation. Handle construction
refuses out-of-range components. Stable entity ID exhaustion refuses before
reserving a slot. Checks: `coax/tests/nominal-ids.py`, compiler parser/checker
checks and zero-survivor mutation gates, and `metaverse-engine/core/entity.coax`'s decoded
handle and exhaustion checks. Store IDs remain session-local.

## CPU scalar transcendental math has one Coax source

CPU `sin`, `cos`, `exp` and `log` use `coax/lib/compiler/strict-math.coax` in
both evaluation and generated C, including verification C. Its committed C
seed and embedded text are regenerated from that source and compared during
bootstrap, never silently refreshed. Private runtime names prevent replacing
the four kernels through libm symbol interposition. The profile requires nearest
IEEE binary64 arithmetic, gradual underflow and no contraction/reassociation;
f32 widens exactly and rounds once. Finite angles use full-range reduction.
Native IEEE domain/overflow values remain nonfinite, while compile-time
nonfinite results still fail E0412. These are fixed approximations, not a
correct-rounding claim. `tan`, `pow` and GPU profiles remain separate.

`coax/tests/strict_math.py` checks independent Decimal references, IEEE edges,
optimization/evaluator/native bits and ASan/UBSan; `seed-portability.sh --linux`
also runs it with GCC. Exact book digests use one recording per program,
without inspecting host-libm bits. See `coax/strict-math.md`.

## A simulation step uses strict math

A step's state must have the same bits natively and in the browser, so a
step keeps its qualified `math_sin`, `math_cos` and `math_pow` profile from
`metaverse-engine/core/math.coax`. Coax's CPU `sin`, `cos`, `exp` and `log` now have their own
fixed profile (#12917); `tan` and `pow` still use host libm, and GPU math is
target-specific. A new deterministic implementation does not silently replace
the engine's recorded arithmetic. Drawing may use either. Random draws
in a step come from `metaverse-engine/core/rand.coax` (seed, stream, counter). Checks:
`metaverse-engine/core/math.coax` (sampled comparisons use a scaled tolerance with a near-zero
floor, not a universal ULP theorem; f64 kernels against pinned Python values;
mutation 0 survivors) and `coax/web/replay-hosts.coax`
(the recorded walk's session digests, and the recorded Grid session's digest
at every step, are equal natively and in wasm). Both sessions must stay
sharp: each steers while turning through angles where replacing the engine's
polynomial with another sine/cosine implementation changes the recorded bits.

The math authority keeps the trigonometric angle bound separate from powers.
`math_pow` accepts finite positive binary32 bases and finite exponents; a zero
exponent yields one even for an invalid base. Otherwise invalid operands or a
computed wide result outside finite binary32 return zero before narrowing.
Underflow may also yield zero. Identity powers preserve finite endpoints.
The chapter-5 power-boundary transcript covers native debug/release and
ASan/UBSan; 169 root mutants are killed with zero survivors (#12921).

The room server's step runs in two hosts (oa-room natively and the
browser's local host in wasm), and Everglade's town, lifts and falls still
call Coax's `sin`, `cos`, `tan`, `exp` and `pow`. So both hosts link
`coax/web/room/room-math.c` in place of the C library's sines, exponentials,
logarithms and powers, and build every simulation unit with
`-ffp-contract=off -fno-fast-math` and `-fno-builtin-NAME` for each
(`coax/web/room/build.sh`, `worker.coax`, the Dockerfile). The two towns parted
otherwise: a fall on the edge of a 10-foot band downed a townsperson in one
host and not the other (#12893). `build.sh` refuses an `oa-room` that still
imports one of those functions. Checks: `coax/web/room/room-math-check.coax`
(room-math.c's results are bit-identical natively and in wasm, and close to
the host C library's) and the drift check's local profile
(`coax/web/tool.sh everglade-drift --no-raid`).

The paired drift rehearsal explicitly turns its camera toward the route through
ordinary pointer input before its start mark. Gravity placement must contain
both complete NPC patrols and produce an actual condition. Background-player
positions publish only from complete acknowledged wire snapshots; part zero of
a multipart full snapshot cannot temporarily erase the page. A skipped part
still refuses clearance. `drift-snapshot.coax` is authority (10 mutants killed,
zero survivors). Neither the 32 m clearance nor the 600 µs supplemental worker
gate changes. Current target evidence: `docs/verification/worker-12981-2026-11-07/`.

The worker's river unit, its Linux `-O3` and its bounded unrolling left with v1's
city units (#13346: the worker links the room and everglade-step only, as build.sh
does; `worker-profile.coax` went with them). Paired replay hosts receive the same complete
control answer. Shifted taps wait through a processed input step. The gravity
rehearsal uses ordinary movement to clear the gate lintel and place both loose
stable props safely inside its field; it still contains both complete NPC
patrols. `drift-placement.coax` bounds that open-road rectangle (11 mutants killed,
zero survivors). See `docs/verification/worker-12980-2026-11-07/`.


The Metaverse Engine binds `local:` room sockets to its existing native room process,
using the same text frames. Browsers retain the Wasm Worker. Native ticketless
admission requires development mode, an explicit IPv4 loopback bind and the
exact `everglade/local-1` path, with one seat; ordinary room ticket checks
are preserved. `native-local-policy.coax` is authority (all 8 mutants killed).
The native executable refuses wildcard binds. The desktop `--offline-check`
observes seat zero, actual Worker creation, room traffic and tick costs;
`everglade-drift --offline native --profiles local --no-raid` passes all 25
probes at the existing tolerances. Paired marks and inputs use a common future
input step, avoiding asynchronous frame origins at collisions. Evidence:
`docs/verification/aethyrforge-offline-2026-11-05/`.

## Source facial animation layers over an independently retained body pose

Valley Lab's optional Slay catalog binds its original Echo rig, named EFC1
face data and SHS1 shot through a whole-package SHA and bounded structural
admission. Invalid names, indices, nonfinite values, duplicate bindings or
inconsistent extents cannot publish partial source data. CPU playback keeps
only a checked runtime projection; sparse morph deltas stay in GPU storage.
Shared Metaverse Engine morph arithmetic runs before skinning. Facial bones overlay a
fresh body pose each presentation, so repeated presentation cannot accumulate
facial deformation. Exact zero weights retain the original vertex path.
The v2 source shot additionally admits its unit world orb quaternion before
playback. The legacy browser opt-in draw-blend v1 preserves opaque defaults;
Coax selects additive orb colour or continuous source-over strand coverage.
The fixed source-shot clock is separate from the walking controller. Source
sample equality is not qualification of Unreal rendering or screenshot parity.
The full Slay groom uses an explicit Coax storage-binding request through the
optional browser storage-limit v1 ABI. Ordinary modules retain default limits;
unsupported adapters refuse through the SDK rather than dropping source strands.
Evidence and outstanding exits: `docs/verification/valley-lab-slay-2026-11-06/`.

## A device's texture layers take Coax's format and size tier

`metaverse-engine/render/texture-format.coax` picks the GPU format of a realm's layer
array from the adapter's block-compression features and the encodings its pack
carries, and the side the array takes. A block format is picked only when the
device has the feature and the pack carries layers encoded in it; otherwise
the layers are RGBA8. A phone (a coarse pointer) keeps RGBA8 layers at most 512
a side and block-compressed ones at most 1024; a desktop keeps the side its
realm declares. The browser host (`coax/web/host.js` `layerTexture`) asks the
shared image reactor (`host-layers.wasm`) for both and, when the reactor
cannot load, uploads as it did before. The block encoder
(`metaverse-engine/render/texture-encode.coax`) ships only bitstreams the independent
decoders (bcdec, ARM astcenc 5.7.1) read as its own decoder does:
`metaverse-engine/tools/texture-encode.coax check` fails on any texel that differs by
more than one level. Checks: `coax/bin/coax test
metaverse-engine/render/texture-format.coax --library` (root-only mutation: zero
survivors, one equivalent at the cap's boundary) and the encoder's
`--library` checks. GitHub #13014.

## Simulation steps and supplied wall clocks keep their domains

The step-clock precursor uses integer simulation steps and distinct nominal
simulation, steady and civil records. Its fixed 500Hz servo/20Hz goal schedule
keeps chunk-exhaustion request demand separate from a periodic timer. Paused
inference advances no simulation step. Capture precedes fresh sample; explicit
forward-generation labels do not imply a generic external cache timestamp.

Ordinary admission preserves state. A bad integration receipt instead returns
a halted cursor on non-ok status; the consumer adopts that returned state and
does not publish a plant step. Epoch reset invalidates old tickets. Clock labels
and host observations are caller-supplied facts, not authentication. This
library reads no clock internally. A wall watchdog, compiler R13/R14, complete
replay, generic sensor-age policy and robot embodiment remain outside this profile.

## Solver checkpoints preserve accepted input order and exact identity scope

The oscillator replay precursor snapshots a completed original RK4 step, its
physical parameters/state, integer tick/dt and already-accepted pending forces.
It preserves active signed zero and requires canonical zero in inactive lanes.
Future unaccepted events and external producer state are absent and must be
resupplied. The versioned little-endian format validates all fields and a tagged
checksum before atomic restore; it binds caller-expected nonzero solver,
numeric, target, exact executable and run identities. A checksum is integrity,
not remote authentication or proof that a caller supplied honest identities.

Same-build restore/replay is exact under the named serial Darwin-arm64 profile.
Different builds refuse instead of implying portable snapshots. First divergence
compares applied force before state and names tick/field/source operation.
This precursor does not snapshot robot/policy/Verse state, establish typed
clocks or compiler `#bounded`, or qualify cross-architecture deterministic math.

## Joint PD keeps active feedback, gravity and actuator saturation explicit

The seven-arm query uses source-range radian current/target coordinates,
rad/s rates and positive typed J/rad² and J*s/rad² gains. Matching current
finger metre geometry participates in owned sixteen-body g0. Requested force
is exactly `(P + D) + g0`, with D using target-minus-current rate. Gravity is
independent of rates and must not be replaced by velocity-dependent bias or
added again downstream. Five force vectors retain individual terms, requested
force and source-clipped command; the controller's clipping count is explicit.
Pa receives command once, with its same cap idempotent and stage counters kept
separate. Physical source limits and numerical boxes remain distinct.

Every shape precedes coefficient admission and refusals preserve complete
output. Independent potential-gradient/scalar references retain all44frozen
contexts, including the specifically named valid-home SDK tuples. Actual emitted
mutation scopes and Quantity-token limitations remain separate. This pure
query supplies no measured holding/tracking, stability, collision behavior,
clock admission, compiler bounded proof or hardware permission.

## The owned neural-query route admits its exact model before opening policy

The new wrapper/build profile accepts only its exact owned choice and immutable
scene/reset/loaded-SDK/85-asset observations. Matching counts or a raw state POD
are not model provenance. Coax hashes/checks observed bytes; C only reads files
and locates the loaded implementation. The private file adapter bounds totals
at 16 MiB and individual streaming updates at 1 MiB without changing old crypto.
Local snapshot admission is not remote attestation or a lock against later
file replacement.

Cached request observations/cameras still precede fresh forward. Owned control
selection follows forward and precedes sample/control receipts; the scenario
fault follows selection. Failed clock state is adopted, and sample publishes
only after success, with no SDK fallback. The default wrapper path retains its
old profile, flags and ABI. The new built binary and mocked/file gates do not
claim a new neural/simulator episode, timing guarantee or hardware result.

## The owned controller sample is one fresh query with a separate behavior proof

The fixed-source constructor admits matching arm/finger positions and rates,
then uses one owned pose result for mass, Jacobian, velocity and nonzero-rate
bias. It preserves PcSample's published 144-F64 layout and grip SITE conventions;
all fields publish together after admission. Source-visible helpers remain
unsealed and trust their inputs. No sensor stamp or simulation step is inferred.

Constructor mutation evidence retains the initial 100/97/3 result, the changed
pcs_forward 29/29 result and exact carried bodies separately. Counts describe
emitted sites, not complete Quantity-operator token provenance; typed literal
velocity and independent SDK fields qualify arithmetic separately. Paired
static-controller comparisons require successful reset/held-call histograms,
exact status/readiness/clipping and separately bounded logical state/output.
Intentional refusals preserve initialized state snapshots. Held calls do not
establish elapsed simulation time, stability or a new policy episode.

The portable paired gate aliases exact historical receipt/sample bytes only
after newly emitted C/header and owned output match their frozen identities.
Historical execution is not relabelled as fresh; differing compiler artifacts
require a new qualification. Driver selection, SDK-stepped prefixes and whole
closed-loop outcomes retain their separate evidence.

## Query selection preserves the SDK ABI and the two observation generations

The admitted Coax selector explicitly chooses SDK or owned controller math,
requires both effects and finite full shadow transports, then validates only
the selected mathematical sample. Owned refusal never falls back. SDK sample
admission checks numbers, pose and actuator limits; it is not an SPD proof.
Published 173-word Telemetry remains unchanged; additive RobotStateV1 copies
nine named qpos and independently addressed DOF velocities without forwarding.

Cached request observations precede explicit forward. Selected/shadow controller
samples follow successful forward, use the same current state and maintain
separate controller state. The two new recorded-action prefixes recompute every
control; they never copy old actuator words. Their 500 SDK steps, 1000 successful
normal controller calls and exact scheduling fields have separate frozen
numerical budgets. Full neural provider wiring is still pending. SDK shadow
geometry, integration/contact and scene effects remain, with no owned simulator,
passive/fluid solver, hardware or compiler boundedness claim. The actual SDK
prefix is explicit opt-in with authenticated assets, never a default gate.

## Native debug maps and performance reports are scoped observations

Coax 0.13.0 can map emitted C statements to original physical Coax files.
Schema-1 maps retain source hashes and authored procedure/variable names;
the optional LLDB presenter refuses stale files and distinguishes separate
libraries by physical owner. Plain C emission stays unchanged without the
debug option, preserving exact seeds. Native debug objects must be retained
where the linker references them. This is not a Coax expression evaluator,
complete type encoding, shader map or device-support guarantee.

Driver clocks, file reports, native compilation and linking are host effects,
never new `#run` capabilities. The checked Coax fingerprint implementation owns
map SHA-256. Timings and source maps do not certify compiler correctness.

Check: `coax/tests/debugging.py` uses actual Mac LLDB, native diagnostics and
sanitizer stacks, including multi-statement lowering and loaded units.
The live Simulator cast breakpoint reaches Coax through the Swift renderer.

The R0 workload and statistical policy live in `metaverse/benchmarks.coax`,
with authority checks and a zero-survivor mutation gate. The external harness
pins historical native source, its explicit reader adaptation and the real
app corpus before comparison; timing policy is separate from output and
allocation assertions. A successful correctness run must not be reported as
meeting a failed proposed timing budget. Measured allocation hooks have a
documented scope; Simulator clocks are not physical-phone performance.

Check: `metaverse/benchmarks/run.py` compares every player palette element,
independently recomputes ranks/medians, observes kernel allocation calls and
runs under ASan/UBSan. `R0UITests` drives the normal Grid/Metal path; inspect
retained frame counts and command errors too. Reference-adapter review,
independently enforced adoption gates and device/tail budgets remain open.

The pose rewrite retains the frozen reference, reader, asset, workload and
threshold. Coax owns clip/skeleton preparation, bounded decoded-row caches and
math; admitted parent-before-child order ensures every cached parent is written
before use. All accesses remain checked, zero-weight interpolation still
normalizes, and public quaternion defaults/target ABI are unchanged. Raw
median/tail evidence and explicit stack costs stay separate from adoption claims.

E0383 rejects known local backing origins in returned views/strings, including
supported checked-helper paths and descriptor fields/elements of aggregates.
Storage ownership is separate from descriptor pointee origins; copying a
record does not rebase its views. Exact returned parameter/field projections
exclude unrelated arguments, including scratch that a helper merely reads.
Borrowed parameter returns remain valid under the caller's lifetime duties.
Direct immutable by-value array slices retain E0366; local-copy slices are
E0383. Known local witnesses survive incomplete origins. Recursive queries and
unsupported helper writes remain explicitly incomplete/manual, never complete
empty or lifetime-safe by default. Bounded analysis exhaustion is E0381.
Storage a view parameter points into is read at the call as the argument's
storage at that place, so a passed-on slice of a caller's array keeps its
caller (or local, E0383) origin. Static origins never decide a returned
borrow: a constant is one, and static origins alone that would overflow a set
collapse to one per type; local and parameter origins never collapse.
Retained stores have the separate bounded B1 profile below. Completion/cancellation,
general fixed points and nested-block regions remain work; this is not general
ownership safety.
Check: checker authority/mutation and seeds, original-source diagnostics,
internal summary status and native/sanitizer buffer-return profiles in
`coax/verification-return-borrows.md`; the initial direct-local profile remains
in `coax/verification-local-borrows.md`.

## Supplied retained-store facts do not prove lifetimes

B0 originally delivered `coax/policy/retain-policy.coax` as a standalone,
unloaded supplied-fact policy. Its historical qualification remains bound to
that earlier source and producer. B1 moves it into `coax/lib/compiler/`, which
is included in the recursive compiler fingerprint and build snapshot, and
loads it from the checker. Descriptor backing and addressed storage have
separate checked keys; nonstatic terms belong to the supplied current frame,
static terms to frame 0. Fact completeness is independent of known-relation
completeness, forbidden witnesses and pending outlives obligations. Unknown
alternatives never erase known pairs; complete-empty descriptor facts are not
an ordinary zero-length view of an owner. Canonical pending rows retain the
64-term/128-pair/8-path bounds and atomic logical publication, without a
C-padding contract.

B1 examines typed writes in concrete checked procedures, including uncalled
void procedures, concrete instances and the separate hidden check procedures.
Known local backing stored into caller/static addressed storage is E0383 at
the original RHS. Parameter-backed external stores remain latent obligations;
actual caller substitution can reject them at the original call or discharge
known caller backing. Same-formal ordinals alone do not discharge obligations.
Addressed storage is derived independently of its previous descriptor value.
After crossing a view, LOCAL/STATIC ELEMENT wildcards describe frame ownership
only; they provide no exact element/field alias or strong-update proof.
Recursive and foreign/intrinsic effects, unsupported helper writes and local
region ordering remain explicit incomplete/manual facts. Known bad alternatives
survive those incomplete alternatives.

B2 raw return-origin queries compose selected checked helper writes through
formal views into local carriers and their returned descriptor fields.
Retained-store-only misses keep the manual helper boundary; complete raw facts
can serve source-class queries. Neither missing raw facts nor manual fallback
prove write-free behavior. Write-only queries exclude incoming descriptors;
caller defaults and possible writes join separately. Unrelated reads add no
origins, and a later safe write cannot erase a local witness. Callee-local
external stores retain their original RHS diagnostic. Active recursion stays
incomplete. An already incomplete retained fact skips the opacity scan that
could only add incompleteness; complete facts still require that scan. Calls
and arguments retain admitted-prefix checks and the existing depth-32,
eight-step path, 64-origin and shared 1,048,576-work limits. Actual `so_post`
return fields select caller out/err, static problems or modeled default empty;
foreign retention stays incomplete. By-value carriers containing mutable views,
unmodeled addressed getters, broader context lifetimes, B3 and recursive fixed
points remain manual. The [early-exit record](docs/verification/coax-b2-early-exit-2026-11-01/README.md)
binds the changed guard and actual-compiler cost: **1,044,325 / 1,048,576 steps; 4,251 spare (0.41%, owner accepted)**. The
[failed candidate](docs/verification/coax-helper-write-returns-2026-11-01/README.md)
keeps its own evidence identity. Neither focused checks nor development-stage
compiler production establish normal release/bootstrap qualification, a full
desktop application or general lifetime safety.

B1 source-only queries canonicalize STATIC lifetime classes while keeping LOCAL
and PARAMETER identities, descriptor types and roles. Their memo keys include
the mode. A1 return and addressed-storage queries do not enable this B1 source
projection; their existing static-origin rules still apply. The source projection
supplies no alias/value identity proof. Completed raw facts may serve source-class
queries; STATIC/mixed spans use the existing normalization in a temporary result
before output publication. Class-to-raw value reuse, active and incomplete rows
remain unsupported. Direct storage paths without dereferences compute the
owned-slot fact without a separate origin-query row/span.

Frozen concrete field layouts use a B1-only type/ordinal cache. Type-owned
kind4 query rows hold eight lazy field positions, stay separate from declaration-
owned origin queries, and share the existing query capacity. Rows and slots
publish only successful lookups. A1 contexts keep
their ordinary field walk. Selecting an inline array behind a source view
parameter substitutes the actual storage at that path, preserving local witnesses.

The 2026-11-02 desktop continuation extends the represented projection from
eight to twenty-four logical steps, retaining every origin component. Step 25 still
refuses E0381, and a represented local escape still refuses E0383. Internal
native headers and their objects rebuild together; no wire contract changes.
A whole write wider than 64 descriptor leaves chooses existing wildcard array
precision for all its arrays before publishing effects. Distinct fields stay
distinct, exact 64 writes stay exact, and 257 independent effects still refuse.
A private type classification memo resets each pass and publishes no failed
preflight result. Historical dated verification keeps its eight-step profile.

The pass shares A1's bounded work budget and memo workspace. Owner-approved
2026-11-02 desktop adoption raises that cap to 2,097,152 steps; since #12644
it is per declaration in each pass, not shared by the whole compile (spec
16.2), and the refusal names the declaration. The
separate bounded sum scanner keeps its explicit 1,048,576-step limit. Exhaustion
still refuses E0381 before publishing partial results. Dated B2/current24
records above retain their original 1,048,576-step profile. Optional reports
expose only a committed used prefix, with count zero on any rejected
compilation, including later authority/backend rejection. Disabled
reports add no semantic work. Private staged rows and unused tails have no
published-value or C-padding contract.

Retained whole writes stream aggregate descriptor leaves individually under
that shared budget. Aggregate width has no separate 64-leaf/element cutoff;
the per-leaf 64-term/128-pair, per-procedure 256-effect and twenty-four-step path limits
remain. A cold structural scan meters every field. A private per-execution
shape cache may reuse a complete validated immediate-field layout; warm reuse
meters its selected retaining-field validation and recursive visits. Shapes
hold at most eight immediate retaining fields; wider layouts keep full streamed
traversal. Type/declaration/head identity and captured maximum field/type IDs
preserve committed-prefix bounds. The graph and unvisited scalar links/types
remain frozen for that execution; this is not authentication of arbitrary
within-run mutation. After a child query rejects, structural traversal and
later retained-store stages stop. A1 query-internal diagnostic collection is
unchanged. A late
local witness or exhausted workspace cannot publish earlier staged reports.
This replaces the initial B1 breadth profile; A1 returned-value limits remain.
Check: [verification profile](coax/verification-retain-policy.md) and
[actual root-mutation/native evidence](docs/verification/coax-retained-borrow-policy-2026-11-01/record.json):
128/128 root mutants killed and LP64 O0/O2/ASan+UBSan, using the earlier
qualified producer with explicit standalone scope.

## Tagged error handling preserves payloads and cleanup

Coax owns nominal union, Result and optional identity, layout, selection,
handling obligations and sum-aware borrow admission. The native bootstrap
transports those decisions and executes active payloads. The initial profile
is [specified separately](coax/results.md): direct Result locals, parameters
and returns are admitted; transitive Result containment and top-level Result
constants are refused. A payload's exact nominal error type survives propagation.

Result obligations are separate from ordinary returned-value use. Naming,
printing or reflecting a Result does not handle it. Transfer requires an
actual checked Result formal; a later argument's early return cannot erase
an earlier temporary obligation. Continuing branches agree on handling state.
Once-only range bounds and foreach collections execute before loop restrictions;
repeated conditions, outer bindings in loop bodies and deferred captures keep
their conservative restrictions.

Recoverable propagation is an ordinary return: operands and assignment places
execute once in source order, later effects stop, and registered cleanup runs
once in reverse lexical order after capture of the return payload. Fatal traps
retain their fatal contract. A sum has one active alternative; none constructs
no payload storage. Malformed runtime tags trap. Padding and inactive bytes
have no equality, initialization, wire or foreign-ABI contract. New sum-capable
native C headers and layout reports have distinct schema identities; legacy
inspection and device profiles must explicitly support new forms or refuse.

The synchronous file adapter keeps protocol and cleanup decisions in Coax.
It preserves primary foreign errors and records secondary cleanup failures.
It retires a descriptor before its single close attempt and never retries a
failed close; release remains unconfirmed on failure. This does not establish
asynchronous completion/cancellation ownership or general lifetime safety.

Checks: `coax/tests/results.py`, `coax/tests/results_profiles.py`, the authority
policy `coax/lib/sys/file-policy.coax`, and the actual native/fault-injection
adapter harness `coax/tests/sys/read-file.py`. The book's tagged-shape and
typed-port examples exercise these forms in chapters 5 and 7.

## A System One ticket owns one canonical request generation

`coax/lib/systemone/owner.coax` keeps one canonical owner at a stable address,
with serialized exclusive access and live wire storage. Only its generation
ticket is copied. Pending work and an unread terminal result refuse a new
start; generation exhaustion refuses rather than wraps. Stale tickets cannot
poll, cancel, take or discard a newer generation, even after the host reuses
its raw process slot. Cancel produces a terminal result; discard retires it.
An unstarted owner never releases an unrelated slot zero.

Taking a terminal result validates every snapshot capacity before writing,
copies body/stderr/problem bytes to independent caller storage, publishes
metadata last, then retires the owner. Snapshot buffers are disjoint from each
other and the wire buffers, and outlive every returned descriptor. Process
reaping ends native buffer access; it does not retain a borrowed result.
Arbitrary owner-value copies, races and overlapping destinations remain API
violations, not compiler-enforced linearity.

`outcomes.coax` makes admission/snapshot/cancel/discard failures handled Results
and polling/terminal states exhaustive sums. The real Lab copies a snapshot
before response validation, routing and transcript publication; displayed text
is copied again into the transcript pool before temporary snapshot storage
expires. Only explicit no-connection uses its existing alternate door. Timeout,
lost reply and other unknown outcomes never authorize automatic retry or prove
that a remote effect did not happen. Stop and quit handle discard explicitly.

Checks: the owner policy's focused mutation, `coax/tests/systemone-owner.py`,
`coax/tests/systemone-outcomes.coax`, actual native child traces, and the
hash-bound production Lab/default/quit consumer checks in
`docs/verification/coax-outcomes-12625-2026-11-02/`. Their declared component
scope does not establish a full-desktop build or R0 device readiness.

## One protocol for all OpenAgents surfaces

Root `proto/` and the OpenAgents Protocol specification define interchange for
all surfaces, including the OpenAgents Metaverse. Coax is the product implementation target;
The engine is the Metaverse Engine (formerly Metareal, then Verse, then Aethyr). Profiles select imports from one source tree,
not separate definitions per app, target or implementation language.

Check: `scripts/proto.sh check` compiles actual roots and profile closures.
Coax descriptor ingestion is checked by `coax/tests/proto_import.py`. Product
migration and domain/admission validation remain explicit consumer gates.

## No GitHub-hosted CI

Protocol and compiler checks run locally or on owned infrastructure. Do not
add GitHub Actions workflows. A schema check needs no running service,
production credentials or network download.

## A model call is asked of the us endpoint first, of global when us is slow or failing, and counted once

`coax/web/api/inference.c` asks Vertex's Chat Completions for every caller of
`inference_ask` (`/api/v1/inference`, the account chat, the project and world
chats, `/chat`). Vertex serves the same models from the same project at two
endpoints: the us multi-region and global. us is the first choice because it
is the faster one; neither was chosen for where data is kept. The rules are
`coax/web/api/inference-upstream-policy.coax` (`#authority`); the C makes the
calls and keeps the memory.

- **Failing.** A 429, a 5xx or no HTTP answer from the endpoint asked first
  sends the same call to the other endpoint once. Any other status is the
  answer (a 400, 401, 403 or 404 would be the same from both).
- **Slow.** When us has sent no byte of its answer within the bound (3
  seconds, `OA_INFERENCE_HEDGE_MS`; 0 turns it off), global is asked as well.
  The first endpoint to send a byte of an answer that is not a failure is the
  call's answer and the other is dropped (from global in a race only a 2xx
  is an answer: its own refusal never replaces what us is writing). The
  choice is made before any byte reaches the caller, so a caller never gets
  bytes of two answers, and a stream that has begun is never left for the
  other endpoint.
- **Memory.** After us fails or loses to global, global is asked first for a
  while (60 seconds, `OA_INFERENCE_BREAKER_MS`, doubled each time in a row
  us loses again, to 10 minutes), then us is tried again. While global is
  first no race back to us is started; a failure of global still goes to us
  once. After us wins a race (a long answer, not a slow endpoint), no race is
  started for half that while. The memory is one small shared mapping per
  instance, made before the workers start; it is never a place a request can
  write.
- **Counted once.** `inference.request` counts the request against the
  account's day before any endpoint is asked, and `inference.finish` keeps
  one row and one charge, from the status and usage of the answer the caller
  got. An endpoint that failed or was dropped adds no request, no row, no
  spend and no debit of credit. (Vertex may still bill us for a dropped
  call's tokens; that is our cost, never the account's.)
- **The same call.** Both endpoints get the same body (the model, messages
  and limits `inference.request` wrote) and the same service token. When
  both fail the caller sees what one failing endpoint always gave: the first
  endpoint's status and body, else the second's, else 502 `the model was not
  reached`.
- **Seen.** Each call logs one line: which endpoint answered, which was asked
  first and why the other was asked, and each one's status and times. No
  token, key or body is logged. A relayed answer says its endpoint in
  `X-OA-Upstream`.
- `OA_INFERENCE_URL` (tests, the local serve mode) names one endpoint and
  turns all of this off unless `OA_INFERENCE_FALLBACK_URL` names a second.

Check: `coax test coax/web/api/inference-upstream-policy.coax --library` and
`coax mutate` on it with zero survivors; `coax/web/api/test.sh` (the
`upstream:` checks in `test_inference.py`: a second API asks two fake
endpoints, us fast, slow, failing and silent, both failing, a late stream, a
stream under way, the breaker opening and closing, and one kept call for
each ask).

## Check gates reject vacuous and uncovered success

`#forall` domains are finite and nonempty; range cardinality is computed with
widened arithmetic and values are enumerated lazily. `#authority` requires real
procedure execution during checks, not dead references. Empty suites,
uncovered/no-site mutation runs and malformed step budgets cannot report a
successful verification run. Coverage and mutation scores are not correctness
proofs or admission authority; equivalent-mutation exemptions remain reviewed
assumptions. JSON diagnostics retain concrete witnesses.

Check: `coax/tests/verify.py`, compiler ASan/UBSan signed-span checks and the
actual imported wire checks in `coax/tests/proto/runtime.coax`. Native aliasing
and non-finite representations remain in the target-side conformance suite.
Descriptor/importer/fingerprint-module/runtime hashes and generator/compiler versions are recorded
in generated C/headers. The authoritative pure runtime is
`coax/lib/proto/wire.coax`, with `#authority`; its checks and zero-survivor mutation
gate run before publishing a bootstrap compiler. `memory.coax` only observes
native storage overlap and delegates policy to the pure core. Physical addresses
remain a native ABI/sanitizer boundary, outside Coax check claims. Checks and
`#authority` are not copied into imported applications. Descriptor parsing and
generation live in `coax/lib/proto/importer.coax`, with `#authority`, bounded
checks and a zero-survivor mutation gate. Coax owns scratch capacities. Stage0
has proto imports disabled and compiles this actual Coax implementation to C11;
the final compiler links it and must regenerate identical importer C. There is
no C++ parser/generator fallback. `coax/lib/crypto/sha256-kernel.coax` owns
SHA-256 and hexadecimal digests, and `coax/lib/proto/fingerprint.coax` loads it
and owns runtime extraction, provenance formatting and output assembly; each
has its own authority checks and zero-survivor mutation build gate, and the
kernel is a listed compiler input in `coax/provenance.sh`.
The native bridge only reads files, allocates Coax-requested buffers and copies
Coax results. It contains no hash or formatting fallback. The frontend, evaluator, C emitter, toolchain
and native host effects remain trusted; these gates are not a formal proof.
Shell byte copies embed the runtime from its authoritative Coax sources. Normal
compilation must not invoke Python, protoc or helper processes or depend on
adjacent runtime files. Explicit descriptor input reads are not permitted inside `#run`.

Check: legacy schema declaration hashes characterize output compatibility, not
correctness. Independent protoc vectors exercise interoperability. See
`coax/proto.md` for the evidence scopes. Relocated-binary/no-PATH iOS C/header
generation, native descriptor ASan/UBSan fuzzing and the real iOS simulator
flow in `coax/tests/proto_import.py` and app tests.


## Native check C comes from checked bodies and Coax-owned selection

`coax/lib/compiler/native-checks.coax` owns bounded eligibility, substring
selection and the callee closure for `coax checks-c`. C++ transports resolved
AST facts and emits the admitted procedures through the ordinary C backend;
it does not rewrite source, resolve names again or copy/mutate `File` ownership.
All existing authority checks run before selection and remain unfiltered.

The first command emits verification-only checked C11 for selected zero-argument
`#test` bodies, with length-delimited names, original physical locations and a
versioned C table. It emits no product `main`. Selected finite properties,
foreign effects and unsupported representation/lifetime forms refuse rather
than disappearing. Wrapping arithmetic and standalone source maps refuse;
normal product C/profile behavior is unchanged. The stage0 driver explicitly
refuses this command while testing and compiling its authoritative Coax policy.

Emission alone is not execution evidence. The optional native execution
companion below supplies bounded observations. Shared finite-domain traversal
and broader semantic/consumer qualification remain separate work. Existing
`coax.evidence.v1` continues to describe evaluator runs.

## Native execution requires the original baseline and complete observations

`coax/lib/compiler/native-execution.coax` owns baseline, budget, selection,
source/artifact identity, EP1 protocol and phase/verdict validation. The optional
`coax/tools/native_check_execution.py` maintenance tool performs host effects
and transports typed facts; it never parses or rewrites Coax check bodies.
Python is development tooling, never a product or ordinary compiler-build
runtime dependency. Policy startup failure has no semantic fallback.

Every selected file gets one complete, unfiltered original evaluator baseline
under normal or explicitly lower step limits. A failed baseline stops before
that source's native emission/compilation. `--only` scopes native checks only.
The same baseline invocation captures its input profile; observed Coax source
and protobuf descriptor identities must agree across emission and execution.
One compiler-owned C artifact, the fixed observer and generated Coax policy C
are linked at checked O0/O2. Each selected body runs in a fresh bounded process
group. Actual LP64 observations must be admitted before table access.

Native names are byte spans; declaration indices and physical locations remain
distinct even for duplicate names. A passed process needs complete matching
BEGIN/END, normal exit0, complete pipe EOF and no transport error. A signal
after BEGIN is a raw nonpassing observation, never automatically a recognized
Coax trap. Nonzero status after END or malformed successful output is
infrastructure failure. Time/output exhaustion is inconclusive. The host
enforces caps while reading and kills the whole process group on exhaustion.

`coax.native-checks.execution.v1` is separate from evaluator-only evidence and
the existing legacy/strict-baseline companions. It binds original sources,
selected identities, C/observer/policy/native hashes and actual compiler/SDK
observations. Those observations do not claim a complete SDK-content lock or
broader target qualification. Default ABI 1 leaves selected `#forall` as an
explicit gap. Existing product authority gates remain unchanged.

## Finite checks share Coax domain planning and bounded native progress

`coax/lib/compiler/check-domains.coax` owns finite-domain admission, product
limits and tuple traversal for both the evaluator and verification ABI 2.
The trusted FD C/header seed reproduces at both bootstrap fixed points;
there is no C++ enumeration fallback. Original bound expressions retain their
order, step accounting and executed-procedure coverage. A rejected product
stops before the next bound is evaluated. Enum declaration order, canonical
signed values, nonempty domains, the 2²⁴-case cap and last-binding-fastest
order remain language rules. Native arity 64 is a separate profile limit.

ABI 2 calls the original checked parameterized procedures. Coax owns catalogue
framing, progress transitions, expected tuples, witness formatting and verdicts
in `quantified-execution.coax`; hosts allocate/map files, atomically publish
encoded slots and invoke admitted procedure pointers. One fresh bounded process
runs each property. A fresh 32769-byte region has a one-byte committed selector
and two 16KiB slots; BEGIN is published before each body, END after it returns,
and DONE after complete enumeration. A passing observation needs valid complete
progress, normal exit0, both pipe EOFs and no transport error. A begun signal is
a raw counterexample, never automatically a recognized typed trap. Exhaustion
is inconclusive. Original unfiltered baselines remain mandatory for qualification.

`execution.v2` is distinct from published ABI 1/EP1/execution.v1 contracts. An
artifact-only failing-tuple observation does not invent an evaluator baseline
or qualify a source whose original baseline failed. Captured source, descriptor,
compiler, policy, observer and native bytes remain bound across phases.


## Native semantic experiments preserve failed originals and artifact identity

`native-semantics.coax` owns structured runtime-trap frames, expected-fault
matching, the single verification-C seed and bounded tuple replay. Only
`checks-c` emission has an optional trap callback; ordinary product C has no
callback or check harness. Existing ABI 1/2 tables, EP1/QP1 and execution.v1/v2
keep their meanings. The semantics observer calls original checked procedures.

Strict positive qualification retains the complete original baseline. A
negative original stays a counterexample: matching its expected evaluator
code family/location and structured native fault is an artifact-only parity experiment,
with `baseline_qualified=false`. Recognition requires a valid begun case,
known reason, original physical location, expected abort and complete transport;
arbitrary crashes or stderr text cannot become recognized Coax traps.
Failed claim locations remain fault locations. Named negative fixtures supply
separate declaration oracles; exact native-table admission never rewrites the
original evidence or infers a declaration from its fault.

The multiplication-to-addition seed changes exactly one admitted helper anchor
in a retained copy of verification C. Original sources, original C and their
passing baseline stay bound. Independent literal products detect the changed
arithmetic. Coax proposes at most 32 admitted tuple replays against the same
seeded executable; a fresh process reports separate SP1 single-case progress,
never invented earlier traversal. Accepted reductions preserve optimization,
check identity, reason, location and seed identity. This is bounded tuple
reduction, not a global minimum or arbitrary source minimizer. Host tooling
retains compiler/SDK/artifact hashes and enforces process/capture bounds.

## Compiler profile selection preserves legacy defaults and authority

`coax/lib/compiler/profiles.coax` is `#authority` and owns command admission,
duplicate/value validation and independent arithmetic, optimization, BUILD_DEBUG
and debug-information resolution. Explicit axes override the legacy preset
independent of argument order. No-flag and `--release` defaults retain their prior
behavior. Native C++ transports choices and performs tool/file effects; it has no
alternate resolution policy.

Compile-time/check arithmetic stays checked and authority checks remain mandatory.
One loaded graph has one profile; separately compiled C/foreign callees keep their
own arithmetic. Schema-1 profile reports are optional observations, separate from
existing report schemas. Native optimization is null when no native compiler
started, and absent artifacts have null hashes. Input hashes use consumed source
and descriptor bytes. A report refuses aliases with inputs/outputs and is not
written when the complete input graph could not be admitted. These observations
do not establish filesystem immutability or correctness. Resolved profile identity
binds mutation receipts.

Check: profile-unit checks/zero-survivor mutation, explicit trusted seed
regeneration and both bootstrap fixed points; `coax/tests/semantic_profiles.py`
covers native axes, O0/O2 behavior, failures, authority and mixed-profile receipt
refusal. New device qualification, arbitrary native check execution and f32 decimal
rounding remain separate gates.

## The compiler lexer is Coax with an explicit generated seed

`coax/lib/compiler/lexer.coax` is the authored lexer, including keyword/operator
selection, locations, recovery, decoded strings, diagnostic text, integer
accumulation and decimal-to-binary64 conversion. `coax/src/lex.cpp` only supplies
owned storage and copies the Coax result into the native token ABI; it has no
native scanning or numeric-conversion fallback. Token ordinal ABI agreement is
checked when compiling the bridge.

A checked-in generated C/header seed builds the first frontend. Normal builds
must regenerate that seed exactly from the Coax source, pass its authority and
zero-survivor mutation gates, and reproduce the final compiler modules before
publication. Refreshing the seed is an explicit operation with a previously
trusted compiler. The seeds, C++ evaluator/emitter, native tree bridge, host/toolchain
and native ABI remain trusted; fixed points and finite checks are not proofs of
bootstrap security or of the whole compiler. Normal bootstrap and iOS code
generation remain Python-free.

Check: `coax test coax/lib/compiler/lexer.coax`, `coax mutate` on that file,
`coax/build.sh test`, and `coax/tests/lexer.py` (compiled Coax ABI/recovery,
independent exact-decimal and binary64 midpoint oracles, ASan/UBSan byte-pair and
random-input fuzzing). The real iOS Home/Grid/animation tests exercise product
code compiled through the Coax lexer.


## The language parser is checked Coax, with no native grammar fallback

`coax/lib/compiler/parser.coax` owns grammar/precedence, declaration lookahead,
syntax diagnostics and bounded indexed tree construction. Source proto import
syntax, top-level/Metal admission and prefix/count/path checks also live here;
the driver only resolves/reads/assembles their files. `src/parse.cpp` only
transports tokens and reconstructs arena objects/lists; native AST kind mappings
have compile-time ABI assertions. Raw token spelling lengths must survive
separately from decoded string content. The first syntax error is retained and
no partial tree is admitted. Node/message capacities come from Coax; recursion
admits at most 128 active grammar entries, reporting E0221 on exhaustion.

The generated parser C/header seed is explicit, refreshed with a trusted
compiler, and must reproduce exactly in the Python-free bootstrap. Its actual
Coax checks and zero-survivor mutation gate run before publication, followed by
final self-regeneration. C++ evaluator/emitter, toolchain, generated
seeds and native ABI remain trusted. Compatibility snapshots are not proofs.
Check: `coax test coax/lib/compiler/parser.coax`, mutation testing on that file,
`coax/build.sh test`, `coax/tests/parser.py` (independent tree assertions,
random expressions, bounds/stack limits, actual native bridge and kernel under
ASan/UBSan), plus the real iOS simulator tests from empty generated outputs.


## Read-only views preserve access restriction and backing lifetime

`[] const T` (#12924) is a distinct read-only descriptor in the evaluator and
C output. Restricting `[] T` preserves element identity, backing and count and
evaluates the operand once; no assignment, call or cast restores writable
access. Slices and inline aggregate projections preserve the restriction.
Array constants and immutable array parameters can be borrowed in a read-only
expected context; array parameters retain their by-value calling convention.
Temporary arrays still require a local, and ordinary returned-borrow and
retained-store checks still govern every descriptor and carrier.

Concrete element containment is validated after layouts and generic instances:
writable or recursively unresolved descriptors are E0950, rather than an
implicit way to recover write access by copying an element. Read-only views
do not freeze other aliases or establish race freedom. Generated C descriptors
have distinct names and const element pointers; existing writable descriptor
identities are unchanged. Device output is explicitly unqualified (E0951).
The contract is `coax/readonly-views.md`; checks cover the parser/checker policy,
native ABI and sanitizers, refusal fixtures, chapter 6 and the normalization
table consumer. Engine scheduling and resource policy remain the Metaverse Engine's.

## Type checking is Coax with no native semantic fallback

`coax/lib/compiler/checker.coax` (with its parts in `checker/`, #12646) owns
semantic admission and diagnostics,
name/scope/type resolution, numeric folding/range/IEEE decisions, natural
layout, calls, control flow, checks and Metal admission. The C++ bridge only
supplies owned storage and reconstructs native records/references/lists after
success. The indexed input is the trusted parser's internal graph; no partial
typed tree is admitted on failure. E0381 bounds storage, native layout size
and 256 active semantic entries.

The explicit checker C/header seed reproduces in the Python-free bootstrap,
passes authority/check/zero-survivor mutation gates and self-regenerates in the
final compiler. Seed provenance, C++ evaluator/emitter, native ABI/storage and
toolchain remain trust inputs. Compatibility snapshots and finite mutation
scores do not prove semantic soundness or bootstrap security.

Check: `coax test coax/lib/compiler/checker.coax`, mutation testing on that
file, `coax/build.sh test`, `coax/tests/checker.py` (independent IEEE, natural
layout and policy expectations; actual emitted checker under ASan/UBSan),
complete compiler bridge sanitizers in `coax/tests/parser.py`, protoc
interoperability and the actual iOS Home/Grid/animation tests from empty
generated outputs.

## Incremental speech owns a separate model and current stream epoch

`psionic-nemotron-streaming-en-v1` is the hash-pinned original English RNNT
checkpoint, separate from the multilingual offline TDT profile. Coax owns its
incremental features, causal subsampling, fixed attention/convolution histories,
LSTM state, GPU arithmetic/schedule and append-only hypotheses. The C host
performs file, memory, mutex and CUDA transport effects. Linux CUDA is the
qualified backend; absence refuses rather than selecting a different model.

`psionic/lib/incremental/policy.coax` (`#authority`) admits one nonzero local
epoch, strictly increasing resets and exact push sequences. Samples are finite
mono F32 at 16 kHz in [-1,1], at most 16,384 per push and 960,000 per stream.
Zero length is final-only. Whole-push validation precedes consumption;
stale/duplicate/gap/extent/audio refusals preserve the current stream. A backend
failure invalidates text/inspection histories and requires reset. Cancellation
clears host and mutable device histories; final/cancelled streams accept no
further audio. Snapshots copy only under a current live/final epoch and sufficient
capacity. Fixed token/text exhaustion fails the epoch. Backing weights/tokenizer
remain immutable while mapped; fixed inference storage excludes OS file cache
and driver/JIT allocations. A bounded transfer mode changes byte residency only.

`incremental/voice.coax` (`#authority`) binds local tickets to existing voice
media/turn identities (including initial media epoch zero). Partials are display
only; one final is eligible for the current turn. The file-backed original-weight
consumer uses the actual Metaverse Engine question/answer gate, revokes tickets on
barge-in/cancel and refuses late answers. It does not replace a default provider.

Checks: 58/58 stream/model-admission mutants and 33/33 voice mutants killed;
frontend/kernel hand checks, native API/refusal/sanitizer consumer, and thirteen
original-weight reference cases including exact partials, token positions,
features, caches, ragged pushes and a continuous 30.46 s stream. The explicit
Transformers mask-representation adapter, finite quality, latency, fixed-memory
observations and remaining platform/UI adoption are recorded in
`docs/verification/incremental-asr-2026-11-03/README.md`. Chapter 153's offline
profile/evidence remains unchanged.

## Source composition preserves physical ownership

`coax/lib/compiler/sources.coax` owns literal load syntax, canonical identity
admission, cycle/budget rules and verification-scope decisions. Native code
resolves/reads regular files and transports owned storage. A canonical source
loads once; an active cycle fails; graph source reads are bounded before and
during I/O. Loaded units cannot close each other's delimiters. There is no
source-load interpreter effect or native syntax-policy fallback.

A physical unit's `#authority` requirements survive loading without spreading
to unmarked dependencies. The internal checker graph receives source-owned
scope annotations, never untrusted wire claims. Actual executed-check coverage
still gates authority functions, and default mutation covers reachable helpers
while requiring entry/authority coverage. Optional `--root-only` selects
mutation sites using the procedure's physical declaration provenance; it does
not filter baseline checks or weaken authority coverage. Scoped scores do not
qualify dependency mutation coverage. Shard receipts bind scope even when
filtered and default inventories coincide; merging modes refuses. Compiler/evaluator/emitter, native ABI and
filesystem behavior remain trust inputs; finite checks are not proofs.

Check: source-kernel authority checks and zero-survivor mutation, explicit
`sources.c/h` seed identity in the Python-free bootstrap, final regeneration
from `lib/compiler/compiler.coax`, and `coax/tests/sources.py` (real driver,
cycle/diamond/symlink tests, scoped authority, relative descriptors, original
file diagnostics/traps, graph limits, helper-free relocated compiler, native
ABI and ASan/UBSan with an independent widened budget oracle). The book
capstone and real iOS HUD consume loaded policy/types. `coax/tests/mutation_scope.py`
checks default frozen-compiler parity, imported baseline failures, physical
scope selection, worker/shard parity and scope-separated receipts. `coax deps` is a checked
path inventory, not a lockfile, content cache key or native build manifest.


## Release status has one authored source and explicit limits

`coax/release.coax` owns the current compiler version and reviewed feature
catalogue. Coax renders the embedded CLI metadata, JSON inventory, accepted
feature reference and explicitly managed current-version regions. The
`#authority` validation model checks metadata shape/identity; it does not prove
feature semantics or grant runtime authority. Implementation, inventory schema,
historical introduction and public protocol versions remain separate.

Generated snapshots are reviewed bootstrap inputs. Normal bootstrap compares
them with fresh output, then compares the final binary before publishing it.
App generation and book publication refuse drift; the publisher checks before
reading credentials or creating a draft. Unmanaged historical prose is never
rewritten by version-value substitution and still needs review. Normal builds
and generation remain Python-free.

Every compiler-version bump requires a matching newest entry in
`coax/CHANGELOG.md`, including an authored summary, valid date and commit line.
Every release heading follows `## VERSION — 2–4 word summary — YYYY-MM-DD`;
the plain-text ASCII title is mandatory for current and historical entries. Missing,
out-of-range or placeholder titles refuse in both check and write modes.
The Coax changelog model validates headings, ordering and the current entry before release
generation writes anything. Preparation may use `Commit: pending`;
`coax/build.sh` (including compiler-cache reuse), app generation and publication
checks require a finalized hash. In a Git checkout that hash must match the
observed introducing commit; archives retain structural validation. `--write`
fills or repairs only the current commit line from Git, preserving authored history.

The public `/coax/changelog` page and its Markdown twin derive from that same
file. Each website build runs the Coax generator before compiling the Coax
reader; display text and accessible HTML resolve repository links against the
canonical GitHub source, while the Markdown twin preserves every source byte.
The Coax project links to the page, and staging/production smoke checks require
source parity, discoverability and the reader's named assets.

Check: `coax test/mutate coax/release-model.coax` and
`coax test coax/changelog-model.coax` /
`coax mutate coax/changelog-model.coax --root-only` (zero survivors),
`scripts/coax-release.sh --check --binary coax/bin/coax`, and
`coax/tests/release.py` (snapshot repair, version-domain separation, invalid
metadata, managed regions, stale binaries, real acceptance probes and
publication rejection before credentials/network). Workflow and limits:
`coax/RELEASE.md`.

## Compiler builds reuse only exactly matching checked results

`coax/build.sh` installs a cached compiler only after `provenance.sh install`
verifies its record against the current tree's compiler-input fingerprint. One
builder runs per fingerprint on a machine; the others wait and install its
result. Every install is a copy to a temporary name followed by a rename.

A machine without a cached build installs the published build of this tree's
compiler sources for its platform before it bootstraps (owner, 2026-11-02:
"we need release builds per platform when we ship new versions, so we don't
need these waits"; `coax/published.sh`, `coax/RELEASE.md#published-builds`).
A published build is named by `provenance.sh source-fingerprint` (the compiler
inputs and the platform, not the local toolchain) and installed only when its
download matches its checksum, its manifest names exactly this tree's source
fingerprint and this platform, every file matches its hash, a Linux build
links no shared library, and the compiler runs. The installing machine then
writes its own provenance record for its full fingerprint, so every later
install from its cache is verified as before. Only a checked build of a
committed tree with no surviving mutant is published, and a published name is
never replaced. Release hosts are our own machines (`coax/publish-host.sh`),
never GitHub Actions. `COAX_BUILD_FRESH=1` or `COAX_NO_PUBLISHED=1` always
bootstraps.

The bootstrap reuses a stage0 `test` or `mutate` gate only when a passing
result exists under a key that hashes stage0's complete inputs (sources, seeds,
the embedded proto runtime, compilers, flags, SDK and build environment), the
exact gate command and the bytes of every source that `stage0 deps` lists for
the gate's file. A gate whose sources cannot be listed always runs. Failures
are never kept. Final-compiler gates and seed comparisons always run.
`COAX_BUILD_FRESH=1` runs every gate and ignores cached builds. The cache
directories are trusted local state, like the checked-build cache; a reused
gate records an earlier pass on this machine, not a signature.

Check: `coax/tests/build-cache.sh` (three concurrent warm builds in new
worktrees install the same verified compiler within the time limit), and a
bootstrap log after a one-file change names the reused gates. Workflow:
`coax/RELEASE.md#build-caches`.

## Only survivors a change causes stop a build or a release

A bootstrap mutation gate that fails only on surviving mutants stops the build
when the change since the build's base caused a survivor: the survivor is on a
changed line, or the change removed or edited a check line (`#assert`,
`#test`, `#forall`, `#authority`) in the survivor's file or in the file the
gate mutates. The base is where the checkout left `origin/main` for
`coax/build.sh` (or `COAX_MUTATION_BASE`), and production's release commit for
`coax/web/release.sh`, computed for the commit the release builds. Changed
lines include `.coax` files the working tree adds without committing. Other
survivors are recorded in `coax/bin/mutation-survivors`, in the checked-build
cache and in `releases/<time>/MUTATION`; they never stop a release. A gate that
fails for any other reason (a failing baseline, an unreached procedure, an
error) always stops the build. A cached build is reused only when none of its
recorded survivors is on a line the reusing build counts as changed.
`COAX_MUTATION_FULL=1 coax/build.sh`, and any build without a base, gates on
every survivor; the zero-survivor statements for compiler units elsewhere in
this file are what that full check enforces. The Coax taskmaster owns it.

Model boundary: a survivor on an unchanged line that a code change elsewhere
caused indirectly (for example, a changed caller that makes the line's result
unobservable to the checks) is reported, not gating. The full check catches
it.

Check: `coax/tests/mutation-gate.sh` (changed-line listing for the working
tree, for a commit range, for untracked files and under `diff.noprefix`; a
survivor on an unchanged line reported; one on a changed line named and
gating; a removed or edited check gating the survivor it leaves, also through
the gate's mutated file; strict mode gating; and refusal of reports that failed
for other reasons). Workflow: `coax/RELEASE.md`.

## Generated C headers check compatible declarations across libraries

Headers share compatible fixed arrays, views, records and enums through
losslessly encoded direct ABI schemas. A schema includes the C spelling,
kind, size/alignment and ordered field names/types/offsets, array extent or
enum representation/members/bits as applicable. Referenced declarations are
checked independently, including recursive borrowed records. Defaults and
checkout paths do not participate. A conflicting public spelling fails with
`incompatible Coax C type`; equal size alone cannot make it compatible.

Each type has its own guard, so repeated includes work without a basename
guard hiding another library's exports. C names, layouts, procedure bodies
and plain C emission remain unchanged. Library C and headers add only the
`coax_default_T` state constructors below. Related headers must be regenerated
together; this does not establish logical module identity, ownership, public
protocol identity or semantic interchangeability. Encoded identifiers are
qualified with the tested Clang C11/C++17 toolchains, not a guarantee for C
implementations with short significant-identifier limits.

Check: `coax/tests/tools.py HeaderComposition` compiles and links independent
libraries in both include orders, tests recursive/shared and empty records,
rejects equal-size and enum conflicts, and exercises repeated/same-basename
headers. Bootstrap regenerates the explicit header seeds while retaining
byte-identical C seeds. The real iOS Haunt/Cast/HUD headers compose without
the old fixed-array macro rename.

## Hosts construct Coax state with the compiler's defaults

A `--library` unit defines, and its header declares, `coax_default_T(T *p)`
for each header struct or fixed array that has a declared default, directly
or in a field or element. It writes zeros and then the declared defaults in
place, so the result has the bytes `s: [1] T` has natively, whatever `*p`
held. It is weak because every unit that uses the type defines it the same
way. A host that allocates Coax state (a browser engine's WebAssembly state,
which starts as zeros, or a C face's globals) calls it before the first use.
No build step edits generated C to apply defaults; `coax/web/defaults.py` no
longer exists.

Checks: `coax/tests/state_defaults.py` (in-place defaults over 0xab bytes
match native `coax_init`, natively and in WebAssembly; two libraries sharing
a defaulted type link; a program emits none) and
`coax/web/state-defaults.coax`, which boots every engine
`apps/openagents-web/build.sh` makes from zeroed memory, compares each
defaulted state with native `coax_init` byte for byte, compares every face
global of a defaulted type natively, and fails when a zeroed state would
pass. The iOS app's Swift host doesn't call it yet.

## Machine-readable diagnostics keep one schema and leave text unchanged

`--diagnostics json` writes each diagnostic on stderr as one
`coax.diagnostic.v1` object a line, with the keys `schema`, `severity`,
`code`, `message`, `span`, `notes` and `fixes` in that order. Every span lies
inside its file, and its line and column agree with its byte offset. A fix
carries edits only where its insertion is certain ("add 'X'", "add 'X'
before the end of the line"); otherwise it carries text only. A change to
this shape takes a new schema name. Without the flag, or with
`--diagnostics text`, the text output is byte-identical to the release
before the flag.

Check: `coax/tests/diagnostics_json.py` (recorded lexer, parser, checker
and failing-check diagnostics; every line parses and every span is checked
against its file; the parser fix applied makes the file check; `mutate`
gives the same object; text recorded with 0.45.0 is unchanged).

## A check's step budget is its own #steps or the file's, and a failure names it

A check with `#steps N` runs under a budget of exactly N steps in `coax
test`, in an `#authority` compile and as the base run of `coax mutate`;
every other check runs under the file's budget (`--steps`, default
20,000,000). `--steps` never changes a check's own budget. A mutant's budget
is ten times the unmutated check's steps, at least 10,000, and never more
than the check's own `#steps`. A check that runs out of steps fails with
E0400, and its report names its steps, its budget and whether the budget is
the check's own or the file's. `--profile steps` only reports: it changes no
check's steps or outcome, and its rows' own steps add up to the check's
steps.

Check: `coax/tests/steps.coax` (a check whose own larger budget passes
where the file's fails, one whose own smaller budget fails where the file's
passes, `--steps` larger and smaller, a mutant killed only by its check's own
budget, the profile's rows for a small fixture);
`coax/tests/fail/steps_zero.coax` and `steps_name.coax` (E0220).

## Code queries answer from the checked graph and use a cache only while its inputs match

`coax query FILE defines|uses|exports` answers from the checked program
through `#load`, never from text, so no answer lies in a comment or a string,
and each use's line and column point at the written name. A use is a written
name that resolves to the declaration; an inferred type is not a use. The
cached index is used only when the compiler binary's SHA-256, the entry unit,
the selected front-end profile and library mode, every source unit's path and
SHA-256 and every descriptor's SHA-256 match the ones it was built from. Its
private payload checksum and record validation detect damaged local cache
state; they do not authenticate facts supplied by another party. Old private
indexes are rebuilt without changing the public `coax.query.v1` answer.
Expression traversal visits each shared node once per owner. `--library` is
honored, and changing `#if BUILD_DEBUG` selection cannot reuse another
profile's checked graph. Any mismatch makes the query check the program again.
The query's check stops before compile-time
evaluation and `#authority` checks and changes nothing the compiler emits.

Check: `coax/tests/query.coax` (recorded answers for a fixture module with
comments, strings, an alias, an export list, untyped and folded constants
and a generic body, and for the compiler's own sources; every compiler use
is checked to be the name in code; a cold and a warm answer match, and a warm
query over the compiler sources takes under 1 s; an edit to a loaded unit
changes the next answer; shared expression chains stay bounded; library and
development/release modes stay distinct; repeated descriptor aliases remain
cacheable; a damaged or old private cache is rebuilt).

## A library's checks are checked once, and a cached result equals a fresh one

A compile that emits C, a header or nothing leaves a loaded unit's `#test`
and `#forall` declarations out of the consumer only when that unit checked
in its own `#load` closure, and, when the program has an `#authority` unit,
when its checks also passed when run. `coax/lib/compiler/sources.coax`
decides which declarations those are (`SU_check_spans`) and which units
qualify (`SU_library_checks`): never the entry unit, nor a unit whose
closure has an `#authority` unit or an `#import_proto`. A unit that doesn't
check on its own keeps its checks in the consumer, which reports their
errors as before. The cached result is used only under the same compiler
SHA-256, checking options and closure units' relative paths and SHA-256, and
it gives the same C and diagnostics as checking again. `coax test`,
`mutate`, `query`, `checks-c` and `deps` see every check. C holds only the
generic instances that emitted code calls and the print and enum-conversion
helpers it uses; leaving out what only checks reach never changes what a
program does.

Since #12659 the units without a cached result are checked alone in forked
children, up to the machine-wide build-worker budget (`$COAX_CHECK_JOBS`
caps it). A unit's child starts
only after every unit before it in the sequential order that it loads has a
verdict, and a child sees only its own closure, so the verdicts, cache
records, C and diagnostics are the sequential ones for any job count. The
compiler's own SHA-256 (part of every key) is computed on a thread while the
sources load and lex; reading it, and every fork, waits for that thread.

Check: `coax/tests/library-checks.coax` (a first compile checks both loaded
units, a second reuses both, a new cache gives the same C; the C has the
instances main calls and none that only a check makes; only the printed type
keeps its helper and the program prints; `coax test` runs every check; an
edited library is checked again; a library check with a type error, and one
that fails when run under `#authority`, still fail the consumer; one job and
four give the same verdicts, records, C and errors over a `#load` chain, a
unit that doesn't check alone and failing libraries); `sources.coax` checks and zero-survivor mutation gate; the bootstrap's seed
and compiler fixpoints.

## A passing #authority compile is reused only under the same inputs

A compile of a program with an `#authority` unit (`coax c`, `check`, `build`,
`run`, `lib`, `header`, `metal`, `cuda`) records that its checks passed in
`~/.cache/coax-check/` (`$COAX_CHECK_CACHE`) and a later compile reuses that
pass instead of running them. The key is the SHA-256 of the compiler binary,
the options that change checking (the run-step budget used, including the
entry file's `#run_steps`, `--run-steps`, the profile, arithmetic, build
profile, library mode, Metal or CUDA), every unit's canonical path and
SHA-256, the SHA-256 of the whole input after `#import_proto`, and which
units' checks were left out as libraries. A failed check, or any error during
the checks, is never recorded, so a failing program runs its checks and
reports the same diagnostics every time. A passing record keeps frozen capacity
facts and replays the same near-limit notes, so a reused and a run pass give
the same output, exit status and C. The near-limit threshold participates in
the key; text, rich JSON and quiet one-line JSON render from the same facts.
The private v2 record publishes nothing until its full payload validates.
`coax test`, `mutate` and `--evidence` always run the checks. The build-only
stage0 can't hash itself and records nothing. The cache is local trusted
state, not a signature: anyone who can write it can make a compile skip
checks.

Since #12657 a compile runs those checks in forked children, one check a
child, up to the machine-wide build-worker budget (`$COAX_CHECK_JOBS` caps
it), with the same evaluator,
budgets and limits. A child that passed contributes its steps and the
procedures it entered (for E0380); every failing check runs again in the
compiler, in order, so diagnostics and exit status are the sequential ones.
The parallel result is used only when it must equal the sequential one: no
child crashed, no check changed shared evaluator state (frozen values or
struct defaults), every check's peak storage plus what earlier checks left
behind fits the limits, and no check failed after storage was left behind.
Otherwise every check runs again sequentially. The order children start in
(slowest last time first, from a times record beside the check cache)
changes nothing else.

Since `ax-job-budget` every coax process that runs work in parallel (these
checks, the library checks and `coax mutate` workers) draws its workers from
one budget shared by every coax process on the machine: `flock` tokens in
`$COAX_JOBS_DIR` (default `~/.cache/coax/jobs/`), by default the performance
cores less a third reserved for the person at the machine. Invariants: a dead
process never holds a token (the kernel drops its locks); every process always
runs at least one worker, so no build waits on another; no budget, reserve,
load backoff or wait is hidden (`coax status`, and one stderr line when a build
waits); and the pool decides only how many workers run at once, so stdout, the
C, the mutation report and exit status are the same for every worker count.
`coax/tests/job_budget.coax` tests concurrent processes, a killed holder and
byte-identical output at 1, 4 and all cores.

The evaluator computes number, bool and enum expressions (literals, local
reads, field and index reads, unary and binary operators, `ifx`) on a scalar
path that builds no `Value` per node (#12657). It charges the same steps in
the same order and reports the same errors at the same places as the general
path, which every other node still takes; storage accounting is unchanged.
Indexing a local view reads it in place rather than copying it, with the
same step and checks. Host bookkeeping (the live-storage list, pooled local
frames, executed-procedure marks) is not compile-time storage. Check: before
and after, `coax test` printed identical per-check steps, verdicts and
diagnostics on `metaverse/demolish.coax` (91 checks), `coax/tests/pass`, `fail`,
`steps`, the results and System One policy files, the parallel-checks and
authority-cache fixtures; `coax/tests/ct_storage.cpp` under the sanitizers.

Check: `coax/tests/parallel-checks.coax` (passing, failing `#assert` and
`#forall`, over-budget E0400, E0380 coverage from children, storage left
behind within the limit, and past `--slots 40000` only sequentially: each
gives the same stdout, stderr, exit status and per-check steps sequentially
and in parallel, and the last falls back to sequential).

A child that observes near-limit steps, traversal or storage does not stand
in for that check's diagnostic details: the parent reruns that check and
collects its current consumers and source spans. Such checks are omitted
from per-check reuse records; quiet checks remain reusable. If earlier
checks retain storage, the diagnostic path falls back to the sequential
plan. Whole-program reuse still replays the completed capacity facts.
Checks: `coax/tests/capacity_refusals.py` compares cold, warm, edited and
moved-source diagnostics in text and rich JSON, one and multiple workers,
threshold changes, cross-format reuse and incomplete private records.

Storage peaks belong to the limits in effect during that evaluation phase.
When compilation enters checks, direct test/mutation execution, or native
check-domain preparation, measurements restart at the live storage retained
from earlier evaluation. Released storage and old holder snapshots do not
carry into the new phase. Returning from authority checks restores the
compile limits and starts measurements from the remaining live storage.
Previously reported capacity facts retain their original limits. Parallel
children start from the same live baseline. Real check pressure, including
retained constants, still produces notes and allocation refusals.
Checks: `capacity_refusals.py` covers released and retained compile storage,
real notes and refusals, direct tests, one/multiple jobs and cache reuse;
`ct_storage.cpp` checks slot, container, string and snapshot resets under
ASan/UBSan, including limit restoration after passing and failing checks.

Check: `coax/tests/authority-cache.coax` (a second check reuses the pass and
prints what the first printed; `coax c` gives the same C either way; a changed
loaded unit, `#run_steps`, `--run-steps`, build profile or compiler binary
runs the checks again; a failing check runs every time with the same
diagnostics and exit status; `coax test` runs every check).

## A reused check is one whose every input is unchanged

When an `#authority` compile's whole pass isn't cached, `parallel_checks`
takes a check that passed before from the check cache, instead of running
it, only when a key over all its inputs is the one it passed under (#12647,
`ReuseIndex` in `coax/src/verify.cpp`). The tokens are split into top-level
items at depth-0 `;` and `}`. The split is used only when every parsed
declaration starts an item that spells its name and no item holds two;
otherwise nothing is reused. A check's inputs, by name from its tokens, are:

- its body, in full;
- each procedure it ran, in full, and what that body names;
- each procedure such a body names but the check didn't run, by its
  signature (the tokens before its body) only;
- any other declaration so named, in full, with every procedure it or a
  body holding `#run` names in full (those run at compile time);
- every token outside a named item, and the units' paths.

A name matches every top-level declaration that spells it, in any unit, so a
new declaration a check's code could bind to changes the key. The procedures
a check ran come from the run that recorded it. If every input is
unchanged, the check takes the same steps and runs the same procedures. The
key also holds the compiler's SHA-256, the checking options, the units left
out as libraries, the step budget, the storage limits and the storage every
check starts with. Content is token kind, spelling and value, never source
position. A record holds only a passing, unshared report (steps, cases,
peak and final storage, the procedures run), which stands in for a child's.
The sequential-equivalence rules and E0380 then apply unchanged, so the
diagnostics, exit status and C are a cold compile's. A failing check is
never recorded. A procedure an executed check ran that has no item, or a
name with a newline, means that check isn't recorded.

The check cache is bounded: at most `$COAX_CHECK_CACHE_FILES` (65,536)
records and `$COAX_CHECK_CACHE_BYTES` (2 GiB). No compile walks it to learn
its size (#13307). Its account, `.account` in the cache directory, holds the
record count and bytes; every record is renamed into place under the
account's flock, which adds it to the account (a replaced record adds only
its change in size), so concurrent compiles keep it exact. A put that leaves
the account over a limit, or finds none, starts one detached `coax
check-cache evict` (at most once a minute a process, none while one runs:
the flock on `.evict`). It walks the cache once, evicts the least recently
modified records (a hit touches its record) down to 7/8 of each limit, removes
temporary files older than an hour, and writes the account from what it found
plus what was put meanwhile. A missing or wrong account only causes a walk, or
a later one; no compile reads the account to answer a check.
`$COAX_CHECK_CACHE_EVICT=inline` evicts in the compile, at its exit. Keys and
records name each unit by its path inside its Git checkout, never by the
checkout's absolute path, so worktrees with the same sources share passes
(#13077).

Check: `coax/tests/check-reuse.coax`. An edited procedure, constant,
compile-time-run procedure or called signature runs only the checks that
reach it. A body no check runs, or a moved declaration, runs no quiet check again.
Checks that produce capacity notes run again after edits to refresh their
diagnostic locations.
Options and the compiler binary run every check. A failure is never
recorded, and putting the source back reuses the earlier pass. Coverage
holds from reused checks, `coax test` runs every check, and the cache stays
within its limit. Each step is compared with a new cache: stdout, stderr,
status, per-check steps and `coax c` output are the same. The pass, fail,
diagnostics and check-fixture corpora compile the same cold, warm and,
for `#authority` files, after an edit. `coax c` of Everglade, Newland and
the CLI gives byte-identical C cold, warm and after an edit (the commit
records it).

## A resident compiler answers with a cold compile's bytes

`coax serve` (#13138, `coax/src/serve.cpp`) keeps work between compile
commands, and a command run in it must produce what the same command
produces in a new process: the same stdout, stderr, exit status and files.
Three rules hold that.

- **The resident runs no compiler phase.** It forks a child for each
  command, and the child runs the command's ordinary code with the caller's
  arguments, directory, environment and streams. The only differences from
  a cold process are the memo the child inherits and the check of changed
  bodies it may run first (next section).
- **A memo entry is a pure function of its whole key.** An entry holds what
  the compiler computes from one unit's bytes alone: its tokens, that it
  passed the unit syntax check, the SHA-256 of the bytes and the SHA-256 of
  its tokens. A lookup compares the unit's whole text with the entry's, never
  a hash alone and never a path or a modification time. An entry is made
  only from a unit that lexed without a diagnostic, by the child that
  computed it. The compiler's SHA-256 is kept the same way, and a resident
  serves only clients whose binary has its own bytes. The skeletons of
  compiles that passed the type check are kept too; the next section says
  what one may be used for.
- **A command runs once.** A request starts only after its client says it
  is still waiting. A client that gets no reply runs the command itself
  only when the request never started or is known to be gone.

- **A resident is bounded (#13323).** At most `COAX_SERVE_JOBS` requests
  (default the machine's cores) run at once; more wait in its queue and their
  clients are told so. A request never outlives its client, its resident,
  `COAX_SERVE_TIMEOUT` (its client then exits 124 with a note) or 30 s after
  its reply. A resident whose binary is gone takes no new request and leaves
  once its requests end.

A resident that is absent, refuses, is silent or dies costs a cold compile.
`COAX_SERVE=0` turns residents off.

Check: `coax/tests/serve.coax` (`tests/run.sh --only serve`). Commands run
cold and in a resident agree on stdout, stderr and exit status for a
three-unit `#authority` program after an edited line, an appended comment, a
syntax error and its repair; for the C and header of a library compile; for
the pass, fail, diagnostics and check-fixture corpora; and after the
resident is killed or stopped; eight requests at once in a resident with
`COAX_SERVE_JOBS=2` give it at most two children; a request ends within 3 s
of its client or its resident being killed and at `COAX_SERVE_TIMEOUT`; a
resident whose binary is removed finishes its request and leaves. With a checkout, it compares the C and header
of fivelab, everglade2 and Character Create before and after a comment and a
new procedure in a loaded file.

## A changed body's errors are reported alone only when they are the whole check's

In a resident, a compile whose sources differ from a compile that passed the
type check only inside procedure bodies checks those bodies first, in a
forked child, with every other body emptied (#13142,
`coax/src/skeleton.cpp`). When that check reports errors, the compile reports
them and stops without checking the rest. The errors must be exactly the
whole check's, in its order. These rules hold that.

- **Nothing outside the changed bodies changed.** The resident remembers a
  passing compile as each top-level item's tokens outside its final brace
  group and the tokens inside it (kind, spelling and value, as bytes), the
  units' paths, modules, loads, aliases and exports, and the options that
  change the parse or the check. They are compared whole, never by a hash.
  The answer is used only when all of those agree except the inside tokens
  of items whose group was, in the remembered parse, the body of a procedure
  without type parameters or of a `#test` or `#forall` check. The child's
  own parse must say the same of each such item.
- **A body's check reads no other body.** The checker's body pass
  (`CK_run`, `lib/compiler/checker/statements.coax`) reads, for a
  procedure, its own body and declarations outside bodies. So every
  unchanged declaration passes again, and the changed bodies report what
  they report in the whole program. With one error reported, the checker
  expands no generic instance and runs no later pass, in the child or in the
  whole check.
- **Only errors inside a changed body are taken.** An emptied body that must
  return a value reports E0338 at its procedure; the child drops those. Any
  other error outside a changed body, a storage error (E0339, E0381), a
  coverage error (E0380: it reads every check's body, and the child emptied
  them), a refused run or a use near its limit makes the answer unused.
- **The child never outlives its compile.** It ends when the compile that
  forked it ends and holds none of its build tokens (`job_child`), so a
  killed compile frees its tokens at once.
- **The whole check would stop on no limit.** The remembered check's use of
  the node store, generic instances, types and text, plus the child's use,
  must be under the near-limit fraction of the child's limits, which are
  never larger than the whole check's. The remembered compile had no use
  near a limit.

An unused answer costs nothing: the compile checks the whole program as
before, and a check that passes records the next skeleton. `COAX_SKELETON=0`
turns the child off.

Check: `coax/tests/serve.coax` (`tests/run.sh --only serve`) sets
`COAX_SKELETON_VERIFY=1`, which also runs the whole check and stops with
status 70 when its errors are not the child's. It compares a cold compile's
stderr and status with the resident's for a type error in one body, in two
bodies and in a changed signature, and, with a checkout, for a type error in
a procedure of a loaded file of fivelab, everglade2 and Character Create.
A loaded file that checks alone as a library keeps its own checks in the
compile once its check fails, so more than a body changed and the whole
check reports the error.

## Development body replacement keeps the resident host and requires a source proof

The explicit Everglade scalar experiment in `coax/web/fast-edit.coax` seals
the compiler, source graphs, SDK, toolchains and all bytes outside `door_ease`.
Its closed f32 candidate receives no host pointers. The native room publishes
between complete Coax calls; the page publishes between animation frames and
refuses an active Wasm room worker. A rejected offer retains the old callable.
The guarded commit command requires completed room/page checks for the exact
current source and matching index bytes. Background failure appears in the
development session. Ordinary builds retain their existing checking path.
The Linux/native and Wasm observations, mutation gates and full-build comparison
are in `docs/verification/fast-edit-2026-11-06/`. General replacement and migration
remain roadmap R26–R27 work.

## CPU reflection lowers to bounded ordinary values

`type_info`, `field_info` and `enum_info` are implemented in the Coax parser
and checker. Their schema-1 records describe the bootstrap LP64 CPU layout;
indices use declaration order and enum bits are zero-extended from the
underlying width. Dynamic selectors use ordinary bounds-checked indexing in
both build modes. Expansion shares a bounded checker pool; no native compiler
object or host pointer becomes a target value. The bridge only transports
nodes. Stable IDs, general derivation/plugins and other target layouts are
not supplied by these queries.

Check: parser/checker authority and zero-survivor mutation gates,
`coax/tests/reflection.py` (independent native size/alignment/offset oracle,
ASan/UBSan, invalid categories/indices, cross-unit layout and expansion limits),
and `metaverse/test_haunt.sh`. The actual app's buffer admission consumes the
reflected Haunted House stride, layer offset and compile-time scalar table.
Metal packing and source binary compatibility remain explicit native checks.

## Coax Native Web pages: Coax decides everything; worlds on the GPU, text surfaces in the browser; TypeScript only for browser crypto

Everything a person sees on a Coax Native Web page (formerly Coaxial:
`apps/openagents-web/`, any app served by `coax/web/`) comes from a Coax
module. Since the 2026-11-11 decision in `coax/web.md` ("text surfaces use the
browser", #13516 brought this rule in line with it), there are two ways to
draw it:

- **Worlds, labs and the splash** are drawn by Coax on the WebGPU canvas:
  words, the graph, the 404 page, the access terminal, presentations,
  buttons and links. The page's JavaScript is the generic host
  (`coax/web/host.js`): it owns the GPU, feeds input (keys, pointer, text)
  into the Coax module and does what the module asks (navigate, submit a
  code).
- **Text surfaces** (the site's side and kit pages, `/chat`, `/settings`,
  `/worlds`, `/milestone`) are drawn by the browser from a Coax module's
  tree: the module builds it from the kit's components
  (`components/kit/frames.coax`, `components/backend/dom.coax`) and
  `coax/web/dom-host.js` applies it with no logic of its own, and hands
  presses and typing back to the module. State, rules and every decision
  stay in Coax. A page names its module (`data-app`), and its app's
  `build.sh` builds that module.

The owner's 2026-11-14 component-composition request extends the text-surface
model with **explicit GPU islands** (proposed; `components/rendering.md`). A
Coax-built DOM subtree may mount a shared GPU component or scene view while
its surrounding document remains browser-rendered text. The DOM applier must
own those mounts; no hand-written visible HTML, JS widget state or arbitrary
CSS overlay is introduced. Geometry, clipping, focus, accessibility and input
ownership must agree across the boundary. Authored realms retain the isolated
realm-origin/`realm-frame.v1` policy below; composition does not grant texture,
cookie or credential access across origins.

On both, there is no hand-written visible HTML, no CSS-styled overlay and no
DOM UI built by JavaScript outside `dom-host.js`'s applier. There is still
no npm, bundler or JavaScript framework. TypeScript exists only where the
browser's crypto and Nostr plumbing need it (`coax/web/unlock/`: phrase
checks, NIP-44, NIP-59, NIP-42, key storage); it hands results to Coax as
data and draws nothing.

On a full GPU surface, allowed HTML (none visible inside that surface to a
WebGPU visitor; DOM text surrounding a declared island follows the text-surface
rule above):
- the `<canvas>`;
- text for search engines, agents and screen readers, clipped out of sight
  while the GPU draws (the `.gpu main` rule);
- the accessible layer (`host.js` a11yMirror): a module's controls mirrored
  as buttons, checkboxes and a polite live region for screen readers and the
  keyboard, always clipped out of sight; the canvas draws what a sighted
  visitor sees, focus included;
- the text fallback a browser without WebGPU sees instead (an owner
  decision, 2026-10-28), and one line above it saying why the page isn't
  drawn and which browser to use (`host.js` noGpuNotice, #13155: with no GPU
  Coax can't draw it; `check-visuals.sh` passes only its lines marked
  `// text fallback`);
- an invisible `<input>` over a Coax-drawn text field (the rect Coax reports),
  for the keyboard, paste and the context menu; it draws nothing itself;
- the development-only error note `coax serve` shows on localhost;
- the editor shell's mount (`host.js` editorOpen, #13416, an owner decision,
  2026-11-12: the browser editor's panels come from the `/chat` page's
  components): one empty `<div id="coaxial-editor">` that `coax/web/dom-host.js`
  fills from a Coax module (`metaverse-engine/editor/shell.coax`). It opens
  only with `?edit`, on a dev server or for an admin; a visitor never gets it.
  The host builds nothing inside it. `check-visuals.sh` passes only the line
  marked `// editor shell`;
- a page drawn by a Coax module through `coax/web/dom-host.js` (the site's
  side and its kit pages, 2026-11-13, #13483: `/`, `/chat`, `/worlds`): empty
  mounts the module fills (`<main id=...>`, `<p id=...>`), and on `/worlds`
  the hidden text inside `<main id="page-root">`, which the module draws over.
  An inert `<template>` (the home page's engine, held until wanted) is never
  drawn. `check-visuals.sh` checks these pages with the same element list;
- a realm's loading screen (`coax/web/loader.js`, an owner decision,
  2026-10-30): the realm's name, a bar moved only by what really loads
  (engine bytes, compile, GPU, shader pipelines, asset bytes, textures, the
  world's first frame) and the line naming it, in plain HTML and CSS so it
  shows at the first paint, before the engine can draw. It is gone on the
  world's first frame; on a failure it says what failed with one working
  action (no WebGPU: the realm's text page; anything else: try again). It is
  only on realm pages, written by `realms.ts` (a made realm's by the host),
  and holds nothing else.

One exception, an owner decision (2026-11-08: "standard mmorpg landing page
then right into the game world"): a realm's own domain
(docs/metaverse/everglade-domain.md) has a front door and a 404 page in plain HTML
and CSS, `realms/<slug>/domain/index.html` and `404.html`, written by
`realms.ts` from the realm's `domain` block. They hold the domain's wordmark,
tagline, PLAY NOW, features, screenshots and a footer, and draw no part of a
world: PLAY NOW opens the realm's own Coax page (its `entry`), which draws
everything as above. The domain's `/play` is a realm page and is checked like
one. Check: `coax/web/realms.test.ts` (no openagents.com on the domain's
pages), `check-visuals.sh` skips only those two files.

Places that are one world, such as Character Create and Everglade II (#13018,
#13023), are one page, one engine and one renderer, and say a new address with
action 19 (`history.replaceState`, same origin only) rather than load it or open
it in a frame. (`zone-host.js` and action 18 remain only for the old fivelab
creator that Everglade Experimental builds on.) Check:
`everglade/client/create.coax` "create:" tests (the save, action 19
to `/worlds/everglade2` and the run with no load).

Check: `coax/web/check-visuals.sh` (run by `coax/web/deploy.sh` before every
upload; `release.sh` runs it as a report, since only what breaks staging
against production stops a release) refuses visible elements, inline styling of UI, and DOM-built UI in
the app's HTML, the host and `coax/web/unlock/`; the loading screen's block
may hold only its own elements, and `loader.js` styles only marked lines. It also
refuses a page that loads `dom-host.js` with no `data-app` module, or names a
module its app's `build.sh` does not build (#13516).


## Private projects answer non-members with nothing

A private project's contents (`docs/private-projects.md`) reach only its
members and the agents they register, from oa-api with a session cookie or a
NIP-98 signature. Anyone else, and any slug nobody made, gets the same 404
bytes from the API and a generic locked page that names nothing. Answers are
`no-store, private`; the oa-api backends have Cloud CDN off. Tokens are
stored only as hashes; sign-in links are single use, 24 hours, bound to one
member and carried in the URL fragment; sessions last 30 days. State-changing
requests are POSTs with a same-site Origin or a signature, and rate limited.
Agents never change agreed goals, members or the title, mint links or register
agents. Contents never go to the public relay or the site bucket, and the
public repo holds only the mechanism, never a project's contents.

Check: `coax/web/api/test_private.py` (via `coax/web/api/test.sh`),
`coax test apps/openagents-web/private.coax`, `coax/web/smoke.sh`. Threat
model and residual risks: `docs/private-projects-threat-model.md`.

## Our own testing is not usage

The team's visits (any browser an admin logged in from, the players and
emails in `coax/web/api/team.conf`, and every device seen in one of their
sessions) are kept with `auto = 'team'`, at the source (`analytics.ingest`,
and triggers for imported rows and playtest joins) and by
`analytics.team_tag()`, which the API runs on every start to backfill. Every
visitor count that leaves out `auto IS NOT NULL` leaves them out;
/admin/analytics leaves them out unless OUR TEAM is on and says how many it
left out. Automation keeps its own mark. `team.conf` holds opaque ids and
email hashes only, never an email.

Check: `coax/web/api/test_team.py` (via `coax/web/api/test.sh`),
`coax test apps/openagents-web/admin.coax`.

## Analytics never lose visibility

Every page keeps sending the analytics contract
(`coax/web/analytics-contract.json`, the same rows as `analytics.contract` in
oa-api): the landing's session-start and page-view, the galaxy's page-view,
and the Haunted House's page-view, world-enter, load-engine, area, objective,
quest, click and world-leave; Ice Cream Man's page-view, world-enter,
load-engine and world-leave; Dead Gears' page-view, world-enter,
load-engine, wave and world-leave; and Hecate's Cemetery's, the Sandbox's,
the Grid's, Everglade's and Fairy Forest's page-view, world-enter, load-engine
and world-leave (migration 0069). What people do inside a realm is recorded
with `analytics.coax`'s track_act, track_once, track_tally and track_walk:
counted, batched and sent at most every 30 s for the frequent ones, never
per frame, no words but a chat's length. The golden walk after every release
(`coax/web/after-release.sh`, releases/<time>/AFTER) reports a page that stops
sending one: `coax/web/goldens.ts` plays the journey in headless
Chrome, runs in through the house's front door, clicks and leaves, reads
every batch the pages send (a realm's after its own walk, then leaves it)
and names what is missing. The journey through the house is walked in full on
every release; a realm the walk skips as unchanged since production's last
staged release (its golden comparison only) still opens, starts, draws and
sends its contract. Staging and production get the brief start check
(`goldens.ts --starts`); the full walk runs after the release. In production the Health card on /admin/analytics shows,
per surface, the last event, the last hour and day, and which contract
events no page sent in a day. Changing the contract changes both lists in one
commit.

Check: `coax/web/release.sh` (staging walk), `coax/web/api/test_health.py`
(via `coax/web/api/test.sh`), which also checks the two lists agree.

## Coax group kernels share memory only through owned writes and barriers

`coax cuda` and `coax metal-compute` admit `for thread: 0..<T` loops (T a
multiple of 32, at most 1024) as group kernels: one block or threadgroup of T
threads per item, every thread loop fenced by a barrier on both sides
(`__syncthreads()`, `threadgroup_barrier(mem_threadgroup)`). A thread loop
writes a shared array only at elements its thread owns (`thread*k+c`,
0 <= c < k, plus nonnegative multiples of T*k), code between thread loops
reads shared arrays and never writes them or the views they feed, and a
kernel's shared arrays fit in 32 KiB; anything else is E0700/E0702 with its
own reason. A launch with the wrong group size traps (CUDA) or fails with
`PM_CHECK` (Metal), never computes. The Apple matrix primitives
(`coax/lib/compute/apple.coax`) are Metal-only, group-kernel-only, and keep
the CPU meaning written next to them. On the CPU a thread loop is an
ordinary loop, so the C build is the oracle the device must match bit for bit.

Check: `coax/lib/compiler/compute.coax` (`coax test`, zero-survivor
`coax mutate`), `coax/tests/cuda.py`, `coax/tests/metal_compute.py`,
`psionic/tests/run_group_kernels.sh` (Metal locally, CUDA on `coderos-4080`).

## The homepage publishes verified live observations with explicit freshness

`/` and `/new-land` use the same Coax homepage. Public event ID/signature,
beacon digest/freshness, subscriptions, bounded history repair and project fold
remain in shared Coax. C bridges effects and scalar byte buffers; JavaScript
supplies generic sockets and a wall clock. No runtime Nostr SDK or signing key.
Incomplete project reads preserve the prior projection. Expired Pylon slots
become unknown; signed answer/status counts are deduplicated observations since
this tab's watch began, not completed-job or latency measurements. Worker labels
are project-authored claims. Featured identities/layout and Git trees are deploy
snapshots; no simulated people enter the production view.
The homepage PYLONS board shares that same verified watch and owned rows.
Its join/guide actions use the generic host; full model identities and public
keys wrap in Coax, and panel scrolling never launches an action or moves the Grid.

Frozen real data is used only for deterministic golden checks. Release readiness
also opens the ordinary homepage with real sockets in muted headless Chrome and
requires all six featured projects, the watch and beacon EOSE. A genuine empty
provider set is allowed. Scope and checks: [live homepage record](docs/verification/2026-10-30-live-network-homepage.md).

## Private child input uses an owned pipe and one write deadline

`cxproc_flush_timeout` is the bounded native write effect for private
checker input. It accepts only registered child stdin on a writable FIFO
fd >= 3, preserves unsent bytes after timeout/error, restores mutable flags
and never forwards opaque child output. Partial retries cannot replay an
already delivered prefix. The original blocking flush remains unchanged.
The caller in Coax controls the checker program, trust, payload bounds
and deadline. Native Mac/Linux sanitizer evidence is in
`docs/verification/2026-10-30-process-flush-timeout.md`.

## A game consumes a remote decision only under its current local ticket

`psionic/lib/game/decision.coax` owns a single bounded waiting/ready ticket.
Only its simulation-boundary take can return an action, once, before a monotonic
deadline and under the same epoch, actor/generation, semantic revision, policy,
option set and query sequence. Legal-action bits come from the game. A refused
old callback never revokes a newer owner. Timeout/cancel preserve the sequence;
exhaustion refuses rather than wraps. This is local application authority, not
proof that remote work stopped or that a model's probabilities are calibrated.

The generic browser HTTP slots fence async completion across cancellation,
reuse and engine replacement. Bounded requests have an abort timer and stream
byte cap; explicit cancellation releases silently. Host JS does effects only.
The ambient Everglade consumer keeps admission and its authored option contract
in a separately linked Coax policy library. It is off by default; opt-in shadow
mode records enums and changes no patrol, combat, dialogue or replicated state.
Golden/clip/background and revoked local contexts cannot initiate/apply a guard
proposal. Guests, quota failures and missing Pylons preserve ordinary play.
See `docs/metaverse/pylon-guard-decisions.md` for limits and remaining rollout gates.

Shadow measurements keep a fixed six-entry, local-only ledger with one callback
sample and one terminal outcome per known sequence. Duplicate/unknown inputs
cannot create capacity or counts. Metrics never expand game admission; histories
of callback and admitted-frame latency remain separate. Corpus evaluation owns
pre-authored labels, rejects duplicate/ineligible remote records, exposes
missing observations, and never promotes from a confidence or editorial label.
Actual public/cookie scope and remaining quality gates are recorded in
`docs/verification/pylon-guard-shadow-2026-10-30.md`.


The separate `everglade-directions-v1` candidate in
`everglade/npc/guard-guide.coax` requires an explicit authored help intent,
a current unmet goal, eligible gameplay and a known available route. It selects
only fixed authored tips. Its decoder pins the checkpoint/provider and complete
choice contract; semantic admission rejects a route for another goal, expired
answers and revoked requests, including quiet. It is stateless: a page must
still bind new intents, current world/goal/actor/cooldown revisions and transport
ownership through Gd before applying anything at a simulation boundary.

The actual page's separate Directions/Find Work interaction uses
`guard-guide-game.coax` for current-state projection and
`guard-guide-session.coax` for intent, local display and shadow admission. A
new eligible gesture displays the deterministic local rule immediately, including
for guests. An empty quest tracker supplies no goal until explicit Find Work;
active quests use the same followed objective as the HUD. Known unmodified
geometry, living calm nearby Bram, foreground play and an unmet available
destination are required. Saved edits, hidden/gone objects, spell walls and
destruction refuse authored routes until reachability is qualified.

Only literal `guard-ai=directions-shadow` plus a confirmed viewer hint can start
the optional cookie-authenticated shadow. Player/guest IDs confer no authority;
the server validates the cookie. The narrow world-envelope gate and successful
game edit parser must agree before world-known. The local tip lasts four seconds;
new tips have a six-second cooldown. Shadows have an eight-second lease,
ten-second cadence and six-attempt visit budget, with no queued retry. Current
epoch, actor/generation, world, followed quest/stage, goal and monotonic intent
own each request. A queued gesture retains its input epoch; invalidation before
the next frame cannot mint it as a fresh intent. A new gesture cancels an older owner even during cooldown.
Only this intent's own local hint is normalized; external hints still revoke.

The page updates gameplay and captures facts before the separately linked Coax
policy runs; it draws the HUD afterward in the same frame. C transports those
facts and effects without deciding admission. Callbacks stage only. Lifecycle,
world, objective, combat, editing, leaving and respawn revoke pending owners;
the host cancels their HTTP slots. Nonboundary updates cannot consume queued
gestures or start/admit work. Every displayed line remains the local rule;
remote admissions, refusals and quiet answers are telemetry only. The ambient
consumer and remote-selected behavior remain off by default.

Directions evaluation binds every remote record to its case's frozen request
SHA-256, keeps development and held-out observations separate, and exposes
missing, wrong, refused, late and quiet answers. Quiet fallback never earns raw
model accuracy. The deterministic goal-to-tip reference is perfect for this
finite task; matching it does not establish a remote-model advantage or player
benefit. Labels, split and exact request bytes precede live calls; existing
ambient-v1 records remain unchanged.


## A warm fleet route belongs to a caught-up current connection

A cached beacon is eligible only after the current authenticated/subscribed
connection reaches its beacon EOSE and ready input is drained. Reconnect clears
that fact. Pending newer withdrawals and selected identity/generation/free-slot
and expiry gates are observed again after signing, before publication. When no eligible route exists, discovery keeps its bounded window. A warm
eligible route skips the full window. Fleet text/byte and additive transport-step limits refuse/drop on
exhaustion, never report false empty input. Existing connect/flush ceilings
remain separate from bounded processing work. Focused signed-peer and sanitizer
evidence is in `docs/verification/pylon-guard-decisions-2026-10-30.md`.

The fleet container recipe pins its Nix builder and Nixpkgs source/hash, links
OpenSSL from Nix and ships the complete measured runtime closure, including
loader, TLS libraries, CA bundle and shell. Its numeric runtime user retains a
writable `/tmp`; the Coax-emitted protocol code stays separate from packaging.
The context exporter hashes those Nix inputs alongside emitted C and host effects.
A recipe change alone makes no claim about an already deployed revision.

Image promotion checks authenticated cloud observations through Coax authority:
template and resolved revision specs change only their image, template and selected
service security annotations are preserved, model/provider pins and private IAM
match, and the exact ready candidate digest owns the required traffic phase.
The old revision remains available for rollback. Snapshot files are operator
observations, not cloud attestations; credentials and raw snapshots stay private.

## Metal graphics keeps the portable shader tree and binding layout

`coax metal --vertex/--fragment` uses the same checked tree serializer as WGSL
and SPIR-V. `coax/lib/compiler/metal-shader.coax` owns MSL and reflection;
the C++ driver observes the tree and writes its results. Resource argument
structs are passed by value, storage V3 fields retain the portable stride,
buffer 29 carries element counts, and vertex stream 30 is reserved. Buffer,
texture and sampler namespaces remain separate and bounded. Compile MSL 2.3+
without fast math. Refused interfaces/layouts fail before an output file is
written. Scalar Metal and Metal compute keep their existing profiles.

The offscreen verification planner is Coax (`metaverse/gpu/metal-pass.coax` and
its `metal-plan.coax` authority policy); Swift executes SDK effects and reads
back pixels. Realm shader arithmetic stays in Coax. The seven G5 cases and
eleven entry pairs are qualified in
[the Metal record](docs/verification/everglade-metal-2026-11-05/README.md).
An offscreen executor does not establish interactive host parity or authorize
switching the desktop default before #12868/#12869 pass their own checks.

## Recoverable state changes have explicit boundaries

`coax/checked-state.md` defines typed store/seat lifetimes, enforced source
effects, in-memory rollback and persisted compatibility. A stale reference
returns absence and cannot acquire the next occupant's authority. Generations
retire before wrapping; saved references use stable IDs. Shared Everglade rules
exclude clock, I/O and foreign effects through every call.

A transaction commits on normal completion and undoes escaping Result errors
or optional absence, including helper and aliased-view writes. Traps stay fatal.
Refusal payloads cannot publish borrows or nominal references from aborted
state. Service commits retain their own operation IDs and revision fences.
Release compatibility compares retained production contracts and suppresses only
the same pre-existing failures. No mechanism here closes general view regions
or changes the spec's memory-safety claim.

## Animation Lab retains admitted document storage across GPU replacement

Animation Lab's ALB1 bundle contains one immutable binary, one rig and
1–64 clip documents. The bounded table is checked before any view is made;
the rig and clips use the shared animation authorities. A failed admission
retains the previous library. Borrowed views refer to the retained source
buffer, which outlives playback and GPU replacement. The browser face stages a replacement in a second 64 MiB bank and
publishes it only after admission. Rejected input cannot replace bytes
underneath admitted records.

The inspection renderer retains all eight skin influences and sparse morph
records, with a 64 MiB GPU upload budget. It is an explicitly tracked
renderer for `animation-lab` / `animationlab`, not a shared-renderer adoption
claim. Its Coax-owned 4,096-page (256 MiB) memory declaration is used by both
the Wasm linker and the browser page; ordinary pages retain 1,024 pages
(64 MiB). A5 uses an 8 MiB stack inside that ceiling for staged project admission. The selected
rig, clip and held frame survive device replacement with equal frame hashes.

Checks: `bundle-policy.coax` kills 66 mutants; the exact renderer/memory
adoption gate kills 55, with zero survivors. Station checks, fixed-frame
captures, recovery and timing are recorded in
`docs/verification/animation-lab-a1-2026-11-06/README.md`.

## Animation performance sampling is bounded and starts from source each frame

`metaverse-engine/anim/layers.coax` evaluates an admitted performance from immutable
clips. Each frame starts from rest, then applies base, masked override,
additive and face layers. Source body tracks cannot leak through a face-only
layer. Fix-ups follow in foot, look-at and hand order. Foot locks use authored
contact events and targets, preserve segment lengths, cap pelvis correction
at 15 cm and preserve the swinging foot. Head and eye aims use rest-corrected
axes with explicit angle limits. No solved frame becomes the next input.

The bundle reader reuses one JSON arena across rig and clip documents and
publishes only after all records pass admission. The browser retains pending
JSON in fixed host buffers. Clip/performance parsing uses separate ABI calls;
a staged derived clip is restored if performance admission fails. One spare
clip slot supports the performance without changing source clip enumeration.
Rig views and bounded parser scratch kept A3 inside its 4 MiB Wasm stack;
A5 increases the stack to 8 MiB within the same 256 MiB memory limit. Headless checks exercise the actual asset ABI,
malformed replacement, GPU recovery and fixed frames.

A3 authority gates kill 30 envelope/pose, 82 mask, 88 layer, 84 anchor and
47 bundle mutants: 331 total, zero survivors. Source frame preservation,
contact travel, blend-boundary limits, toggles and frame times are recorded
in `docs/verification/animation-lab-a3-2026-11-06/README.md`.

## Animation directions publish only one checked version

Animation Lab directions use the schema-constrained Decisions API, never a
keyword parser. A reply binds to the exact current bundle hash and names
existing stock. Every field's model-reported confidence must exceed 850/1000.
Unmatched, ambiguous, stale, invalid and no-op proposals publish nothing.
The shared Coax editor re-admits the whole project and screens the numerical
scene report, including hero framing, before publishing an inactive bank.
Licensed library and speech bytes remain identical.

CLI versions live only in canonical private storage. Receipt hashes identify
immutable versions and bind content, parent, words and proposal. Atomic head
replacement occurs under a directory lock after comparing the current resolved bundle again.
Corrupt heads, receipts or snapshots refuse without silently using old source.
Undo validates its parent and restores that exact bundle. Browser history is
bounded to 16 session edits of one JSON component, 32 KiB on each side.
Explicit voice activation, final-transcript deduplication, cancellation and
HTTP response ownership prevent repeated or late replies from creating edits.
See `docs/metaverse/anim-directing-v1.md` and its A7 verification record.

## Native character detail keeps CPU draws and GPU coverage coherent

The character pass shares projected-detail rules in `metaverse-engine/character/detail.coax`.
Its groom draws a prefix of 251 ordered curve groups, reuses identical ribbon
corners, and fades the fractional last group. Width compensation uses the same
effective share; shadows apply their own capped share and matching boundary
weight. Face morph detail fades continuously. Body skinning retains all eight
influences. These `ch_*` paths do not change Valley's frozen `cv_*` renderer.

Everglade rejects empty rain candidates before querying roofs, outside-map
shadow receivers before PCF, and provably empty or depth-hidden puffs before
noise and lighting. Rain colours match the frozen reference oracle; the graphics
quality ladder, pixel floor, spell state and frame-admission policy are unchanged.
The frame timer reports actual raster dimensions, counts held callbacks in the
next drawn frame, requires hardware WebGPU and labels draw-suppressed runs as
non-qualifying diagnostics. Hero qualification additionally requires a ready,
visible character pass, nonzero groom share/geometry and the exact requested
raster (`coax/web/everglade-echo-proof.coax`, zero mutation survivors). A missing
character or smaller raster cannot qualify a fast empty scene. Requested hero
loads are awaited; terminal load failure refuses promptly. R4's historical
timing and complete head-frame review are in
`docs/metaverse/2026-11-06-native-hero-characters.md`.


Everglade 5E's optional grass-range v1 draws preserve original instance IDs.
The shared `render/grass-root.coax` profile rejects only complete root cells
beyond the existing shader planes, using terrain-corner height bounds and at
most 128 ordered ranges. Unsupported grid alignment keeps the original full
draw. Its independent corner/terrain oracle and range access boundary kill
206 mutants with zero survivors. Legacy producers retain the full-draw ABI;
blended producers retain their grass ordering. The opaque browser path draws
world geometry before grass without changing its density or material inputs.

The sky's inverse projection runs at the background triangle's vertices;
homogeneous world points interpolate before the existing per-pixel division
and normalization. Lamp/contact-shadow rejection preserves the frozen sums,
and unused water vertices return before reading nodes. The full fixed-epoch
217-frame orbit comparison records the numerical scope and sparse extreme
tail in the #12984 follow-up. A live hero FPS receipt requires confirmed GPU
submissions with both current world draws and the hero in every sampled frame;
stale readiness alone cannot qualify. The new receipt boundary kills all 30
mutants. No change to the 60 FPS rule or two-frame admission limit is implied.
See `docs/verification/everglade-echo-12984-2026-11-07/performance.md`.

## A Ritual spends no slot, holds Concentration and fails without cost

`ritual.coax` decides Ritual admission: the source Ritual tag (carried on every
executable record and compared with the pinned catalog), a prepared spell or a
Wizard's Ritual Adept spellbook entry, and an unused Magic action with no
Quickened spell this turn. `ritual-cast.coax` applies it: beginning ends earlier
Concentration and holds the caster's Concentration under the spell's own ID for
the ten extra minutes; each later turn owes one Magic action. A missed turn,
failed Concentration save, incapacitation, Rage or a new Concentration spell
fails the Ritual with no slot or free use spent, and never ends an earlier
copy's effects. The final turn's action is the ordinary cast at slot 0, so the
same target, component and capacity preflight applies; a refused completion
keeps the rite. Find Familiar adds its hour of casting time to the Ritual.
Its dedicated completion checks the destination and familiar capacity before
paying incense and, for the prepared one-hour cast, the chosen spell slot.

## Accepted incapacitating effects interrupt concentration immediately

`effect-lifecycle.coax` owns accepted effect insertion and its shared
incapacitation hook; `concentration.coax` owns reconciliation, dismissal and
teardown. A retained Incapacitated/Paralyzed/Petrified/Stunned/
Unconscious condition ends an active concentration spell immediately; death
and zero HP are also reconciled at clock/turn boundaries. Immune/empty condition
outcomes and storage refusal do not manufacture an interruption. All cleanup
uses exact owner/spell identities, resets the concentration deadline, preserves
other owners and unrelated effects, and decrements Spirit Guardians once.

Concentrating programs stop before later recipients or steps once ownership
ends. Spell condition messages precede the resulting concentration-end event.
Dismissal needs no action or component, may happen outside the actor's turn,
and refuses while Incapacitated. Halt alone does not interrupt concentration
or prevent dismissal. Unknown executor identities and out-of-span actors refuse
without treating a fallback spell record as an active effect.

Authoritative raw condition/death writers must notify the shared interruption
hook; clock/start-turn reconciliation is a fallback, not permission to defer a
known immediate consequence. Other condition/feature lifecycles, targeting and
world controls remain open. Evidence and exact limits:
`docs/verification/srd-concentration-2026-11-08/README.md`.

## Exhaustion levels are cumulative state, not expiring condition records

`exhaustion.coax` owns D20 penalties, Speed reduction and one-level recovery;
`exhaustion-lifecycle.coax` admits one gain with immunity/logical-actor checks.
Six levels kill without damage and notify the shared interruption lifecycle.
Attack/save/initiative/escape and standalone/battle death-save paths apply the
penalty; natural criticals and death-save twenties retain their source meaning.
Recovery cannot revive and clears only Exhaustion at level zero. Producers must
use cumulative APIs, not set a raw flag or create a timed condition record.
Long Rest admission and complete mid-turn movement-credit reconciliation remain
separate requirements. Evidence: `docs/verification/srd-exhaustion-2026-11-08/README.md`.

## Ability checks use selected training and an explicit task context

`check-training.coax` owns skill/tool identities, atomic mask selection and
declared class-only preset choices. Expertise must be trained. `ability-check-math.coax`
and `ability-checks.coax` combine the requested ability, one proficiency bonus,
Expertise/Jack, task senses, circumstance/condition edges and existing rescue
policies. Automatic sensory failures and malformed requests consume no dice or
rescue resources. An admitted automatic check consumes matching next-check Help.
Tactical Mind spends a Second Wind use only if its added die succeeds.
Entangle escape and Initiative use the shared plan; rolled results preserve
Exhaustion. Passive Perception uses a score without dice/rescues. The internal
creature stores explicit masks; a display count cannot grant Expertise.

Task geometry, usable tools and source grant provenance remain caller facts.
Legacy open-grid fear visibility, the passive Exhaustion interpretation, exact
native/sanitizer scope and remaining #13039/#13044 acceptance are recorded in
`docs/verification/srd-ability-checks-2026-11-08/README.md`.

## Strike features observe the true turn and reserve their retained riders

`strike-features.coax` requires the actual turn owner for Berserker Frenzy;
borrowing a reaction frame cannot turn an off-turn attack into that feature.
Represented Unarmed/simple melee/Light martial Monk weapons support Stunning
Strike, including reactions. Spell/natural proxies and an unspecified Pact
Blade identity do not establish that weapon category. The same eligibility
feeds strike execution and `strike-admission.coax`'s retained-slot preflight.
Full storage refuses before Reaction/Focus/dice/event spending; one available
slot admits the rider. Its existing per-turn mark belongs to the borrowed
strike frame, while the mover's marks and slot history remain intact. Successful
save Speed loss can interrupt departure without refunding an accepted attack.
Evidence and remaining source/class/world scope:
`docs/verification/srd-strike-reactions-2026-11-08/README.md`.

## Experimental rest uses observed activity and settled owner controls

The shared rest-control policy admits only scheduled validated owners. Actual
walking/jumping selects exertion before frame credit; settled stopping selects
watch, never automatic sleep. Motion closes Short Rest choices, while passive
translation/blocked attempts cannot invent walking. End preserves completion
history and refuses unconsumed earned boundaries. Experimental derives start
admission from the actual grounded/still body, released controls and no pending
strike/dash/automatic walk. Its panel uses these operations and shared HP/condition
state; room migration and persistence remain separate. Layout helpers borrow
combat views instead of copying the entire battle for a few dimensions. The
existing Wasm stack, memory and size budgets remain unchanged. Evidence:
`docs/verification/srd-rest-controls-2026-11-08/README.md`.

## Lighting reads the simulation's day and the shared weather, and changes neither

The environment resolver's day comes only from `metaverse-engine/world/day.coax`: a
versioned `DayClock` with an explicit mode (fixed, cycle or real), source,
rate and epoch tick, evaluated at the simulation tick. A shared (multiplayer)
world refuses a cycle or real clock whose source is the visitor's own page
(`DayRefusal.local_clock`); the room's clock is anchored at a midnight tick
from its W header, so seats that joined at different ticks hold the same clock
and resolve the same day. Its weather comes only from
`metaverse-engine/world/weather.coax`: a versioned `WeatherCondition` read from a copy
of `metaverse-engine/world/water-weather.coax`'s schedule and ground, with spell overlays a
separate local condition that never enters the global one. Applying it is
Everglade's existing weather light; nothing here pins the schedule or changes
gameplay weather, room authority, combat or simulation. A resolved value is
cached only under its whole key (versions, set generation, profile, tick,
world, clock and condition), and a refusal is never cached. Checks: `coax test`
on both files ("fixed-state replay and late-join evaluation agree", "the
cache holds one value under its whole identity", "a shared world never uses a
visitor's local wall clock"), with zero `coax mutate --root-only` survivors.

## Environment regions blend continuously, from a bounded candidate list

`metaverse-engine/world/environment-region.coax` decides where a profile applies. The
state's profile (0: the set's default) is the map default, and outside every
region's band the result is `env_resolve`'s exactly. A region's weight is a
smoothstep over its inner/outer band, and admission refuses a band narrower
than `ENV_BAND_MIN` (0.25 m) and a polygon whose non-adjacent edges meet, so no
region has a hard edge. Overlaps apply in priority order, equal priority with
the lower id last; every weight is continuous and the weights sum to 1. The
dominant profile (largest weight, equal weights to the lower id) supplies the
identifiers and flags; channels mix linearly. A resource slot keeps its four
heaviest ids and reports the weight it leaves out. A set and its fixed index are
replaced together or not at all. A bucket holds at most 8 regions, and a
candidate list holds at most 8 and is refused rather than truncated, so a query
costs at most 64 × 8 index entries whatever the world's size. Camera grading and
sky read `env_resolve_camera` at the eye; surfaces and fog read `env_resolve_at`
inside their stored tile, which refuses a point outside the tile or a stale set
generation. Checks: `coax test metaverse-engine/world/environment-region.coax` ("band
weights are continuous and monotone across every shape's edge, sampled
densely", "overlapping regions normalize by priority", "a pinned route walk
changes continuously"), with zero `coax mutate --root-only` survivors.

## A world's document changes only through fenced, checked edits

Worlds with an ID (#13412; `docs/protocol/world-edit-v1.md`, `world-doc-v1.md`,
`world-live-v1.md`):

- A write to `/api/worlds/ID` is a `world.edit.v1` batch that names the dev
  revision it was made from. Any other revision is refused whole, 409
  (`coax/web/api/world-policy.coax` `wr_write`, `#authority`).
- The server carries a batch out with the world's own Coax, the code its page
  runs (Splice Bots: `metaverse/splice-bots/world.coax` `spw_batch`). One refused edit
  refuses the batch, in words. The client is never trusted for placement.
- The stored document is the one a fresh game builds from its own pieces. A
  document this build would not write is refused (`spw_open`).
- Each saved edit is one outbox row (`realm-authoring.world.edited.v1`),
  written in the transaction that changes the document, one a revision.
- An undo is saved as edits of its own, so every reader follows it the same way.
- A cell of a world's content tables is an edit of the same log (#13419). The
  server checks it as a content document is checked
  (`metaverse-engine/editor/content-edit.coax`), with the realm's own rules
  and its check of every placed thing, and saves the tables in the
  transaction that saves the document (Splice Bots: `spw_cell`). The tables keep
  the revision their build shipped with: a cell edit never changes a pin.
- An agent's turn changes a world only through that same write (#13418;
  `docs/protocol/world-turn-v1.md`). A hosted turn in the editor answers a
  typed plan whose steps are the hosted world tools
  (`coax/lib/agent/world/turn.coax`, `WT_TOOL`); the editor shell makes it one
  `world.edit.v1` batch from the revision the plan names and the page sends
  it as the person's own. The conversation writes no world and reaches no
  world route but a card's read. Which tools a turn uses is the model's
  answer under the schema; no code matches words in a message.
- A session never takes a live revision whose pins (engine, rules, content,
  wire) are not its own. An update that changes a pin starts with the next
  session, and live keeps the revision before it for one release
  (`metaverse-engine/editor/world-live.coax`, `#authority`).

Checks: `coax/bin/coax test metaverse/splice-bots/splice-game.coax --only "Splice Bots' world"`,
`coax/web/api/tests/world_checks.coax`, `coax mutate` on the two
`#authority` files, and for an agent's turn the checks of `turn.coax`,
`metaverse-engine/editor/shell.coax` ("a hosted turn's plan") and
`apps/openagents-web/chat-web.coax` ("in the editor").

Model boundary (#13422): a WorldBuilder world (`/create-world`) is not a world
with an ID. It is kept in the visitor's browser, no server stores or trusts
it, and it has no route. Its page takes the same `world.edit.v1` edits and
batches, fenced by its own revision and checked by its own Coax
(`apps/openagents-web/worldbuilder/worldbuilder-world.coax`: `wbe_thing`, `wbe_dab`,
`wbe_batch`). Check: `coax/bin/coax test apps/openagents-web/worldbuilder/worldbuilder-plan.coax`
and `apps/openagents-web/worldbuilder.coax`. A WorldBuilder world that is ever
stored on a server becomes a world with an ID and takes every rule above.

## A seam inside level ground is never a wall

A seam is an edge inside the ground: a heightfield cell's diagonal, the line
between two cells, a corner four cells share, the line where two floor tiles
meet. Crossing one on level ground, or on an even slope one can stand on,
never stops, slows, turns or slides a walker or a mount, and the ground query
they read (`query_ground`) answers the ground's own slope there, with the
broadphase (`Colliders.near`, #13279) and without, to the bit (#13434).

A sphere that touches the ground at its start touches the nearest feature
(`ground_contact` in `metaverse-engine/physics/ground-contact.coax`): on
level ground that is the face under it, never the edge of the triangle
beside it, so an overlap's normal is the ground's.

One rule for whoever builds a controller on sweeps: a sweep that starts
already touching answers at `t = 0` facing back along the motion (every shape
is solid), which on level ground is a wall's normal. Don't read it as the
ground. Start a sweep clear of the ground, or ask `query_ground` where the
character stands, as the walker does.

Checks: `metaverse-engine/tests/seam-normals.coax` (walks at 16 headings from
9 starts on four level and four sloped planes, rides over a tiled floor and
its lips, the ground query on a 1/8 m lattice both ways, sunk and moving
spheres; it fails on a wall and on a spike), and `ground-contact.coax`'s own
(367 mutants, 0 survivors).

## A realm script is held to its profile by the compiler, and to its module by everyone else

A realm script (#13467; [docs/research/2026-11-13-realm-scripts.md](docs/research/2026-11-13-realm-scripts.md),
[coax/script-profile.md](coax/script-profile.md),
[docs/protocol/realm-script-v1.md](docs/protocol/realm-script-v1.md)) is a
Coax unit marked `#script "name";`, compiled to its own WebAssembly module.
Two walls stand between a script and anything it should not touch, and
neither depends on the other.

- **The profile.** The compiler refuses a script whose procedures reach an
  effect outside reads, writes and traps through their whole call graph
  (E0981), a `#foreign` outside a `#script_api;` unit (E0982), recursion
  (E0983), an export that is not one of the seven typed handlers (E0984) or
  state that is not one fixed-size `#persisted` record (E0985). The policy
  is `coax/lib/compiler/script-profile.coax`, an `#authority` unit with a
  zero-survivor mutation gate in the bootstrap. No existing check is
  weakened: a unit without the directive compiles as before.
- **The module check.** A server, a page and a room each run
  `metaverse-engine/script/wasm.coax` on the bytes they are handed, and load
  only what passes: every import is a row of the script API
  (`metaverse-engine/script/api-table.coax`) with the row's signature, the
  memory has a maximum of at most 4 pages, the module is at most 65,536
  bytes, the exports are the module face's and every instruction is in the
  profile. The check trusts neither the source nor the compiler. A compiler
  bug cannot grant a module a power, because a module can call only what it
  imports.
- **No trap ends the process.** The check, the interpreter
  (`metaverse-engine/script/interp.coax`) and the host bound every index
  before they use it. A refusal and a trap are values. The fuzzer
  (`metaverse-engine/tools/script-fuzz.coax`) runs damaged and generated
  modules, damaged frame and result bytes and damaged manifests through
  all of them, and a Coax trap there is a failure.
- **The interpreter computes what WebAssembly computes.** For a module the
  check passes and whose operand types are sound, every instruction gives
  the result WebAssembly 1.0 gives, except that a NaN result is the
  canonical quiet NaN and `memory.grow` by one or more pages answers -1
  (growth by none answers the size). The oracle
  (`metaverse-engine/tools/script-oracle.coax`) runs a compiled torture
  script in our interpreter and in V8 and compares every state byte and
  command.
- **The API is one table.** `api.coax` (what a script loads) and
  `docs/protocol/realm-script-api.md` are generated from the table; a row
  keeps its number, name and signature, and new rows go at the end.

Checks: `coax/tests/run.sh --only script_profile` (the policy's 65 checks,
214 mutants with 0 survivors, and every refusal in `coax/tests/script/`);
`coax/bin/coax test metaverse-engine/script/wasm.coax --library`;
`coax/bin/coax test metaverse-engine/script/interp.coax --library`;
`coax/web/tool.sh metaverse-engine/tools/script-api.coax --check`;
`coax/web/tool.sh metaverse-engine/tools/script-profile.coax proto/world/script-v1.json`;
`coax/web/tool.sh metaverse-engine/tools/script-fuzz.coax --smoke` (20,000
rounds; longer runs take `--seed N --rounds N --corpus DIR`);
`coax/web/tool.sh metaverse-engine/tools/script-oracle.coax` (needs Node).

## Attachments are private and never reach a player or a pack

A world's attachments and its project's (#13468;
[docs/protocol/world-files-v1.md](docs/protocol/world-files-v1.md)) are
reference files for the people who make the world: concept art, design
documents, notes and links. Only the project's members and the site's admins
list them, read their bytes or change them; a world with no project, its
admins only (`coax/web/api/world-files.coax` `wf_may`, `#authority` through
`world-policy.coax`; `worlds.c` asks before every route under
`/api/worlds/ID/files` and `/api/projects/SLUG/files`). Their bytes live only
in the private drops bucket (`PREFIX/attachments/SHA256`) or, on a dev
server, under the app's build folder; they are served `no-store` and
`nosniff` as the type their extension names. No attachment is in a world's
pack, its document or a page a player loads: the import pack and its routes
never read the attachment lists, and attaching never changes a pack, a
document or a revision. Nothing about a file is logged. A file is admitted by
name, size and the scan of its first bytes (a program is refused whatever it
is called), and its stored object is deleted when no list names it.

Checks: `coax/web/api/test_world_files.py` (nobody 401; a stranger, a page
of another site and a stranger's project list 403; a member and an admin
read; the bytes refused to anyone else; a program called an image refused and
deleted; attaching leaves `verse.world_packs` and `verse.world_docs` as they
were and the player's pack holds no attachment byte; the service log names no
file); `world-files.coax` and `metaverse-engine/editor/file-store.coax`
tests (the rule, the limits, the scan, a freed file).

## A world names a script by hash, and the server stores only modules that pass the check

A world's content tables name each script it runs by the SHA-256 of its
module (#13467; `metaverse-engine/script/rows.coax`,
[docs/protocol/world-scripts-v1.md](docs/protocol/world-scripts-v1.md)). The
API stores a module's bytes under that hash, a world at a time
(`verse.world_scripts`, migration 0186; `coax/web/api/scripts.c`).

- **Only what passes the check.** A stored module went through the same
  check and load a page and a room run (`metaverse-engine/script/host.coax`
  `sh_load`). A refused module is answered 422 with the refusal's name and
  the byte, and no row is written.
- **The manifest is the server's.** It is written from the checked bytes.
  The author gives the name and the state record's fields, and nothing else
  they send is believed. A field outside the module's own state record is
  refused.
- **Who may store.** The world's owner or one of its admins, or a site
  admin, logged in, from this site (`wsp_may`). Today's one world kind,
  `splice-bots`, has no owner, so today that is the site's admins. Who may write
  the rest of a world's document is unchanged.
- **Stated limits.** 64 stored modules a world, 200 uploads and 100 compiles
  an author a day, 20 reports an account a day. Each is in
  `world.scripts.v1` and in the refusal it causes. A full store collects
  only modules that no revision names and that are a day old.
- **An edit names only what is stored.** A script row that newly names a
  hash the world has not stored, or one that is switched off, is refused
  after the realm's own check, in words. `world.edit.v1` is unchanged.
- **Authoring only.** Storing a script writes authored state. Nothing in
  these routes runs a script or reaches a character record or the ledger.

Checks: `coax/bin/coax test coax/web/api/script-policy.coax --library` and
`coax/bin/coax mutate coax/web/api/script-policy.coax --library --root-only`
(zero survivors); `coax/bin/coax test coax/web/api/script-store.coax --library`
("a module is stored only when the check passes", "an edit may newly name
only a module the world stores that is not switched off");
`coax/bin/coax test metaverse-engine/script/store.coax --library`;
`coax/web/tool.sh metaverse-engine/tools/script-store-profile.coax proto/world/script-store-v1.json`;
`coax/web/api/tests/script_checks.coax` through `coax/web/api/test.sh` (the
routes against a throwaway API and Postgres).

## A script's new version may do less, never more, unasked

A publish copies dev's script rows to live (#13467). Before it does, each
dev row is compared with live's rows of the same name
(`coax/web/api/script-store.coax` `wsp_publish`, `wsp_asks`;
`metaverse-engine/script/manifest.coax` `sm_growth`).

- **What needs a confirmation.** A row whose name live does not have, and a
  row whose module has a handler, a power or a call that no live module of
  its name has.
- **What needs none.** A version that only drops handlers, powers or calls,
  and a hash that live already runs.
- **The confirmation is by hash.** The publisher repeats the batch with
  `?confirm=HASH,HASH`. A hash confirms only itself. Without it the publish
  is refused, 409, with each script and exactly what it adds, so a page can
  show the change. Nothing is published.
- **A row with no stored module is not published** (422).

Checks: `coax/bin/coax test coax/web/api/script-store.coax --library` ("a
publish asks about each new or growing script by hash, lists what it adds,
and goes on once each is confirmed");
`coax/bin/coax test coax/web/api/script-policy.coax --library` ("a publish
asks about a row that is new or grows, unless live runs that hash or the
publisher named it");
`coax/bin/coax test metaverse-engine/script/manifest.coax --library` ("the
publish rule"); `coax/web/api/tests/script_checks.coax`.

## Other authors' scripts are compiled in a job with no secrets, and only the checked module is trusted

The compiler, clang and the linker are not a sandbox (#13467;
[coax/script-profile.md](coax/script-profile.md)). A source someone else
wrote is compiled only by the compile job
(`metaverse-engine/tools/script-job.coax` through `coax/web/script-job.sh`),
which the API asks the compile service `oa-script-job` to run when
`OA_SCRIPT_JOB_URL` names it (production, `coax/web/script-job/`), or runs
itself when `OA_SCRIPT_JOB` names it (a developer's machine).

- **The source names no file.** Before any compiler runs, a source is
  refused unless every `#word` in it is on the job's list, with `#load` only
  as the whole line `#load "api.coax";` (or `#load "../api.coax";`). No
  `#import`, `#import_proto`, `#foreign` or `#export`.
- **An empty directory, hard limits, no environment.** The job compiles in a
  new directory that holds only the source and the script API. It has 60
  seconds, 60 CPU seconds a process and files of 16 MiB, and the limits are
  said in every refusal. The API passes it no variable but where the tools
  are; the wrapper starts it with `env -i`.
- **The network is denied where the platform can** (`sandbox-exec` on
  macOS, a network namespace on Linux), and
  `OA_SCRIPT_JOB_SANDBOX=required` refuses to run where it cannot. The
  compile service sets `closed`: without a namespace the job runs only when
  the machine reaches no address outside, so a service deployed with its
  egress open compiles nothing.
- **Its answer is not trusted.** The server checks the module's bytes as it
  checks any upload and writes the manifest from them. From the job it takes
  the script's name and state fields only. A job that lies about the bytes
  changes nothing.
- **A server with no job compiles nothing** (503, `no-compiler`).
- **Not enforced, and stated.** The wrapper does not hide the file system
  from reads, and macOS enforces no memory limit. The compile service runs
  the job as `nobody`, in an image that holds only the job's files, with its
  egress denied and a service account that holds no role (the metadata server
  still answers, with that account only).

Checks: `coax/web/tool.sh metaverse-engine/tools/script-job-check.coax`;
`coax/bin/coax test metaverse-engine/tools/script-job.coax --library` ("the
job hands the compiler only a source that loads the script API and uses the
listed directives"); `coax/bin/coax test coax/web/api/script-store.coax --library`
("a job that lies about the bytes changes nothing" is part of "a module is
stored only when the check passes"); `coax/web/api/tests/script_checks.coax`
(the compile route through the compile service, on a machine with the Wasm
toolchain).

## A script can be reported and switched off

Any account that is logged in reports a stored script with a short reason
(#13467; `verse.script_reports`). A site admin reads the open reports and
keeps the script or switches its module off by hash, with a reason
(`verse.script_off`).

- **Off everywhere.** A module that is switched off is not served (410, with
  why), is not stored again and is not named by any new edit, in every
  world.
- **A world keeps its revision.** Published rows that name the module stay.
  `world.scripts.v1` says the script is off and why, and the page and the
  room skip it. A thing whose script is skipped walks its plan.
- **Reports are limited.** 6 a minute and 20 a day an account, with a reason
  of at most 160 plain bytes.
- **It can be switched back on.**

Checks: `coax/bin/coax test coax/web/api/script-policy.coax --library` ("a
compile needs a job ...; a report a reason, a stored module and the limits",
"a store is held to the switch-off, the day's uploads and the world's room");
`coax/web/api/tests/script_checks.coax` ("a module that is switched off is
not served", "no new edit names a module that is switched off", "the
world's published revision stays as it was").
