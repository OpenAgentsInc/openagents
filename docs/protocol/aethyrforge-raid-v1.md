# Local Metaverse Engine cultist event v1

`GET/POST /api/playtest/forge/raid` is available only to the local desktop's
development admin, with exactly one active Everglade room. Commands follow
`proto/aethyrforge/raid-command-v1.schema.json`. A POST requires the exact GET
`version` as `base`; stale and exhausted revisions refuse without changes.

- `start`: immediately starts the planned standing siege, including its town
  watch, Darian Odiodei and three ward stones. Optional `count` sets the cult
  fighter target (default 30); an already active event refuses a second start.
- `double`: doubles the cult fighter target and musters missing fighters now.
- `wave`: adds `count` fighters to the cult target (default 12), mustering now.
- `set`: sets the cult fighter target to the required `count`, mustering missing
  fighters or withdrawing excess fighters. The watch's target remains 24.
- `stop`: calls off the event and removes its actors. Authored NPCs remain.

The last three army operations require the siege phase; they refuse during
the stand at the camp. Targets are 1..60. The local event may use the room's
96 creature slots, with 24 reserved for the watch and four for the leader and
stones. Every non-event creature, including dead authored NPC definitions,
keeps its slot. A command whose entire target cannot fit refuses atomically;
counts are never silently clamped. Public realm raids retain their 64 limit.

GET returns the command revision, phase, active flag, cult/watch targets and
live counts, wards, leader state, front and cumulative cult fighters sent.
The existing replicated event and creature messages display the real siege.
Event state is transient: reopening starts with no local event. Authored NPC
definitions and orders remain saved separately. Local event control overrides
the development event poll until the room process exits.

`aethyrforge-scene raid` reads state. `aethyrforge-scene raid 'JSON'` supplies
the revision and checks the readback. Unknown POST outcomes require a fresh
read before another command, especially another wave or doubling.
