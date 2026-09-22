# OpenAgents relay retention

The relay stores protocol records. It does not execute them.

## What is kept

| Kind | Class | Retention |
| --- | --- | --- |
| `30180`, `30181`, `30182`, `30184`, `30185`, `30186` | Addressable | The latest event for each author, kind, and `d` tag. Replacement does not run a program or install a release. |
| `3184`, `3185`, `3186`, `3187` | Regular | Every accepted event stays. A later event does not erase a release, revocation, migration, or run record. |
| `25900`–`25920`, `26900`–`26920`, `27000`–`27020` | Ephemeral | Fan-out only. The relay does not keep job state. |

Private capability policy (`30181` with `oa:cap-policy:private:v1`) and run records (`3187`, `30186`) are readable by the author and the single `p` recipient. Other readers, including a relay operator who is neither, get no match from `REQ`, id lookup, `COUNT`, search, or live fan-out.

## What an acknowledgement means

`OK true` means the relay accepted the event for storage or fan-out. It does not mean a worker admitted the job, a program ran, or a release was installed.

`EOSE` means the stored query has been sent. It does not mean a run journal is complete, a gap is filled, or an effect finished.

NIP-40 expiration drops delivery of an event. It does not stop a process a worker already started.

## Advertised roles

`NOSTR_RELAY_OPENAGENTS_PROFILES=true` adds `nip-cap-v1`, `nip-prg-v1`, `nip-ext-v1`, and `nip-run-v1` to NIP-11 `supported_extensions`. Leave it unset until this relay build is the one whose admission tests you have run. The names are not numeric `supported_nips`.

Public CAP, PRG, and EXT bodies are parsed at admission. Private policy and run content are checked as envelopes only. The relay does not decrypt them and does not treat ciphertext as a program.

Publication of a private policy or a run record requires NIP-42 authentication by the event author.
