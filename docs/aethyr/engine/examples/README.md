# Synthetic contract examples

These examples are portable design fixtures. They contain no credentials, vendor
art, implementation code, or runnable game engine.

| Example | Meaning |
| --- | --- |
| [World](world.json) | A semantic single-storey cottage with two rooms, one shared partition, an entrance and an internal opening |
| [Command](command.json) | Open the shared partition while retaining named room regions |
| [Release](release.json) | Wire-valid manifest pinning this synthetic world and non-executable engine/compiler fixture descriptors |
| [Channel](channel.json) | Initial channel body selecting the synthetic release digest |
| [Signature statement](signature-statement.json) | Exact identity statement that an authorized signer would sign; no fabricated signature is included |
| [Fixture engine](engine-fixture.json) | Explicit non-executable compatibility descriptor, not a Wasm engine |
| [Rights fixture](rights-fixture.json) | Synthetic input inventory; it grants no engine-code or third-party asset license |

`tools/check_spec.py` validates shapes, references in this bounded structural
example, and digest consistency. Its canonicalization subset is deliberately
limited to the ASCII/integer-valued synthetic fixtures; production uses full JCS.
The example is **not an admitted publishable release**: the engine descriptor is
not executable, no admission evidence or authorized signature exists, and no
platform has been qualified. A loader must refuse to treat it as a running game.

Hashes cover canonical JSON body bytes without a trailing newline, not the
indented files' bytes. Raw compiled artifact digests cover raw blob bytes; see
[the contract](../contracts.md) for that distinction.
