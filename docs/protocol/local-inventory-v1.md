# Local agent and model inventory v1

The source projection is
[`inventory.proto`](../../proto/openagents/cli/local/v1/inventory.proto).
These are bounded, read-only local CLI JSON reports consumed by desktop setup.
They start no agent, model, microphone, download or provider connection.

`openagents coder agents --json` and `coder/tui/coder.sh agents --json` emit
`openagents.coder.agents.v1`: `ok`, `complete`, `scope:"local_running_processes"`
and `agents`. Each agent carries `engine`, `name`, `pid`, `parent_pid`,
`state:"running"` and `attached:false`. Supported engine names are `claude`,
`codex`, `coder`, `cursor`. An exact executable basename identifies each engine;
Coder's compiler-tagged executable names are also recognized. Discovery reads
`ps` executable names, never arguments, prompts, environment or transcripts.
It bounds output to 1 MiB and 128 agents; time, parsing, capacity or process
failures make the report incomplete and return failure. An incomplete report
must not be displayed as a confirmed empty inventory. A running process grants
no permission and makes no claim of a connected session or attachment to a world.

`openagents models ls --json` emits `openagents.models.v1`: `ok` and `models`.
Every row has `id`, `kind`, `name`, pinned `revision`, resolved `path`, `present`,
weight-file `bytes`, `verified` and `loaded`. The current rows are `parakeet`
(audio) and `gpt-oss-20b` (text). Presence means expected files exist; listing
does not hash or load them. `verified` and `loaded` are false in an ordinary
listing. `models verify parakeet --json` hashes the complete pinned weight and
tokenizer files, checks weight size and returns the same inventory with Parakeet
verified only on an exact match; `ok:false` and failure exit mean verification
failed. Text-model verification is not implemented. Loaded remains false.

The canonical root is `~/.openagents/models`; `OPENAGENTS_MODEL_CACHE` overrides
it and the existing `OPENAGENTS_PARAKEET_MODEL` override selects that exact model.
`models migrate` moves legacy Parakeet, Whisper and original gpt-oss directories
into that root and leaves compatibility symlinks. Existing different copies
refuse; already-identical paths are idempotent. Files are never overwritten.
Weights stay outside Git. The OpenAgents Metaverse Engine saves preferences in its own
`~/.openagents/aethyrforge/setup.json`, separate from these inventories.
