# Ordinary v1 bounded pack loading

The v1 `host.js` path now uses the independently built Coax pack reader.
Pre-v1 retains its existing reader. It copies each admitted stream chunk
directly into a measured arena, including headers split across SDK chunks;
there is no retained chunk list. Private metadata, ciphertext, actual decrypt
output, bounded gzip plaintext and complete digest/layout/catalogue admission
precede any asset publication. Reader cancellation and outstanding crypto or
decoder work retain their exact owners until actual completion.

Completed immutable pack storage can transfer to a matching renderer. Pending
loads retire before engine installation; orphaned packs and GPU recovery
release old source views after their active borrowers finish. Late progress
or completion cannot publish through a new renderer. Progress during recovery
does not append closures to the world's waiting list. Pack sound rows travel
with their source owner. The optional audio descriptor `release` callback runs
in `finally` after the awaited installation, including refusal or retirement;
it cannot detach a compressed pack sound while an SDK decode still owns it.

The physical loopback [receipt](browser-pack-loading/physical/receipt.json)
observes a header delivered as 3/8/1 bytes, complete 512-byte/five-entry
admission, a cut-body refusal, and actual TCP close after cancelling a held
reader. It ends with zero payload owners and only the measured 8 MiB master.
The physical observer kills 43/43 root mutants, without equivalents.

The ordinary current85c explicit-v1 realm
[passes all 23 owner/GPU fields](browser-pack-loading/world-first/receipt.json):
its complete 9,463,680-byte pack loads, actual GPU submissions and live ledger
advance, shader reload retains the same catalogue view, device replacement
detaches the old view and loads again, engine replacement carries the
completed immutable view, and final shutdown releases all pack payload owners.
The ordinary world observer kills 31/31 root mutants. This is the frozen
current85c CPU/explicit-v1 face and admitted3f8 graphics package, not an
unqualified current primary or new FPS/pixel-parity claim. Its source and
artifact identities are in the adjacent observation JSON; the normal run's
driver binary was not independently frozen. Later fault runs additionally
fence their driver source.

Actual ordinary-host [pending retirement](browser-pack-loading/world-pending-enabled/receipt.json)
waits for three admitted bytes, then observes remote close, no published
asset prefix and zero payload owners. A
[cut pack](browser-pack-loading/world-cut-held/receipt.json) marks all four
required parts failed/done, publishes zero bytes and releases all payload
owners. The RAF continues; required-world GPU submission remains refused.
The sharp two-case observer kills 36/36 root mutants.

The real OfflineAudioContext/AudioWorklet
[20-field source-borrow gate](browser-pack-loading/audio-borrow/receipt.json)
holds a completed decoder reply and a late descriptor return across retirement.
The borrowed VPK1 sound bytes remain live until actual installation finishes;
release runs once and both pack and audio owners then reach zero. Malformed
Opus metadata reaches no SDK decoder. The fixture uses a whole Coax-written
VPK1 containing the actual authored `fire-cast-e2.ogg` under a test name.
It uses the already qualified 96,229-byte audio reactor, SHA-256
`4c71171b5025a5ebde14b318ab58d6d05dfe04ebe3870c7c8d8181925da13eb1`;
no live audio device or repeated DSP sample gate is involved. Its observer
kills 28/28 root mutants.

The initially refused fault attempts remain adjacent to the passing receipts:
`world-pending` retired after reader creation without observing fault routing;
`world-pending-bytes` exposed that its Page preload was not active and the
ordinary pack completed instead. The corrected harness enables Page and
observes its injected route before exercising the fault. `world-cut` wrongly
demanded GPU submission with missing required assets; the actual empty asset
state was correct. The final observer records continued RAF and refused
partial-world GPU work. No existing raster, audio or admission bound changed.

Web and desktop build/copy hooks include `build/host-packs.wasm`, 34,411
release bytes under a 42,000-byte app budget. The reactor's genuine b587
compiler receipt, thirteen-source join and exact C/header hashes remain the
[qualified component record](browser-pack-component.md); no producer or
shader emission was repeated. The desktop build tool passes 82 checks; shell
and JS syntax and tools-check pass. Application-owned storage is bounded;
hidden browser/network/SDK buffering and OS/GC RSS are not inferred.

Durable artifacts and frozen SDK copies are under
`~/.openagents/aethyrforge/metal-host-12868/browser-packs/`.
All owned browsers/servers exited. This component leaves public Web opt-in,
desktop default and remaining three-host exits unchanged.
