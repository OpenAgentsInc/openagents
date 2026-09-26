# The Gym building

The Gym brings run observation and explicit run requests into Verse. Walk east
from the spawn plaza, past the workbench, and enter the building marked **GYM**.
Its doorway faces the plaza. Inside, approach the boards to inspect recorded
Microcoder experiments and Terminal-Bench evaluations. The transport also
accepts explicitly configured model-training summaries as a secondary source.

The building is shared desktop/iOS geometry. Its walls have separate collision
footprints and an open doorway; the low walls and open roof preserve the
third-person view. The boards show source observations, not an independently
verified leaderboard. A completed process does not establish a passing task.

## Loading follows your location

The Gym's connection is inert until the active player enters its interior.
While inside, a separate Rust worker requests a bounded snapshot every five
seconds. It validates the host signature, recipient, grant, request, lifetime,
and response bounds before returning data to the frame loop.

Leaving, backgrounding, or disposing the surface drops the worker and its
socket. A reply from a previous visit cannot update a later visit. Re-entry
refreshes the board. Retained values are labeled stale while observation is
paused or unavailable. Closing a board panel while remaining in the building
keeps its observations active; walking out ends them.

This scope is independent of multiplayer presence, public world chat, and the
existing XP quest board. Those services retain their own lifecycles. Entering
the Gym does not run a model, start training, or launch a benchmark.

## Open the boards

On iOS, approach the bulletin boards and tap **Use Gym board**. The connection
panel displays the phone's Verse public key. Configure a Gym host for that key
using the [host setup commands](../../crates/gym-bridge/README.md), then paste
its `gym-connect:` code. The validated connection stays in device-only
Keychain storage. Chat-reader pairing grants no Gym rights.

On desktop, save the host's connection code in a regular file and pass
`--gym-connection /absolute/path/to/connection.txt` to Verse. Enter the Gym and
press `G`. The file is first read on entry; naming it does not start a socket at
application launch. `F5` reloads it while the board is open.

| Desktop key | Action |
| --- | --- |
| `G` or `Esc` | Open or close the board while inside |
| `Tab` | Switch runs and recipes |
| `Up` / `Down` | Select a row |
| `Enter` | Open details; confirm a reviewed recipe |
| `Backspace` | Return to the list |
| `Page Up` / `Page Down` | Scroll details |
| `Y` | Retry an uncertain launch with its original ID |

Each distinct launch consumes its review. Starting another run requires
selecting and reviewing the recipe again.

## Read the boards

The primary boards cover Microcoder and Terminal-Bench. Source categories stay
separate:

- **Agent:** retained Microcoder runs from host-selected directories.
- **Evaluation:** retained Terminal-Bench trial records.
- **Training:** operator-provided model-training summaries with their observed
  time and bounded metric series.

Select a row to inspect its progress, recorded time, reported cost, metric
history, and provenance. Progress keeps its denominator; unknown cost or time
is unavailable rather than zero. Recent file activity is not proof that a
process remains alive. Training curves are the publisher's recorded series,
not samples manufactured by the renderer. The host declares incomplete scans
and stale sources instead of presenting them as a complete live inventory.

The iOS panels use native controls and charts anchored to the world board.
Desktop provides a keyboard-controlled board over the same Rust state. Large
snapshots are read when the board revision changes, not serialized with every
mobile render frame. Gym-specific presentation and permissions stay outside
the generic Rust Native crate.

## Request a run

A separate Gym connection grants observation and a specific set of recipe
revisions. The computer's operator chooses the source directories, recipes,
client identity, grant lifetime, and execution limits. Existing Codex/Claude
chat pairing does not grant Gym access or execution rights.

Select an enabled recipe, review its details and limits, then explicitly
confirm it. The phone sends the recipe identity and revision, never a shell
command or new arguments. The host checks the current grant and recipe, records
the logical request durably, and supervises the configured executable. An
operator can expose a training runner or evaluation runner through this path;
the bridge itself does not implement a model-training algorithm.

The host's wall-time and launch-count limits are enforced. A recipe cannot
claim an enforced dollar cap when its underlying runner has no corresponding
budget mechanism. Missing provider costs remain unknown.

A lost response is **unknown**, not evidence that nothing started. Within the
current app session, **Retry** reuses the same retained request identity. It
does not create a second run. Leaving the Gym stops observation, not an already
admitted host process. A fresh application launch does not automatically resend
an uncertain request; inspect the host's retained run state before explicitly
starting new work.

The [Gym bridge reference](../../crates/gym-bridge/README.md) defines host setup,
connection grants, source formats, supported limits, and the supervised launch
contract. The [NIP-EVAL profile](../../nips/openagents/NIP-EVAL.md) describes the
encrypted messages. Live board claims are separate from immutable completed
evaluation publications.

## Scope of verification

Implementation checks use generated run records, a local relay fixture, and
harmless local executables. Synthetic preview curves are labeled and do not
count as training or benchmark evidence. Native rendering and actual transport
are tested separately. No model, training, or Terminal-Bench workload is
launched as part of developing this feature.
