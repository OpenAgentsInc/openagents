# Character checkpoint receiver, first part of #12838

The inactive, additive version-1 receiver is implemented and tested. Room
producers and restore tickets still use the previous format; this record does
not qualify live crash recovery. The remaining gates are listed in
[`character-checkpoints-v1.md`](../verse/character-checkpoints-v1.md).

Checked in the isolated `openagents-codex-qual` worktree on macOS, based on
`888031a31b`. Builds run at nice 15 with one Coax build/check worker. No
production resources, deployment scripts or visible browsers were used.

- `coax/bin/coax mutate coax/web/api/checkpoint-policy.coax --root-only --jobs 1`:
  **138 mutants, 138 killed, zero survivors**. This includes independent
  input boundaries, owner/process/revision decisions and receipt names.
- `coax/bin/coax test coax/web/room/room.coax`: **540/540 passed**. Room sources
  are unchanged from this passing run through the base above.
- Scratch Postgres API room section: **125 checks, zero failures**, including
  13 new checkpoint checks. The native API builds with ASan/UBSan and
  `-Wall -Wextra -Werror`. Separate SQL connections see committed whole state.
- Scratch private pack section: **16 checks, zero failures**.
- The selected scratch API run completes with `api: checks passed`, including
  **98/98 CLI caller checks**, the anime director/selector checks and the
  engine-pin, shapes, edit-log and realm walkthrough checks. No sanitizer
  diagnosis is reported.
- The unfiltered `coax/web/api/test.sh` was attempted. Its prelude passed
  (199 and 200 NIP-98 checks, deployment command/verification tests); the API
  oracle had **1,149 passing checks and no reported failures** when stopped
  within its 15-minute limit. It was waiting in `test_realms.py`'s
  `talk_checks` through `b_call`, following local storage request timeouts.
  **This is not a full API gate pass.** The room/pack sections above run on
  a fresh scratch database with
  `API_TESTS_ONLY=room_checks.coax,verse_packs.coax`; the already-passing
  prelude was not repeated.
- `coax/web/tool.sh tools-check`: reports five pre-existing missing headers
  in `verse/tools/humans/{face_rig,metahuman,outfits,pack_human,shape}.py`.
  This part introduces only Coax, SQL and documentation files and edits the
  existing native SDK adapter.

Receiver evidence covers signature/shape refusal, an unissued owner,
committed acceptance, lost-response retry with the original acceptance time,
changed-payload conflict, process binding, XP regression refusal, revision
ordering, takeover fencing, new-owner revision reset and complete dead/place/
build state read by another database connection. No actual room or database
kill/restart was performed for this receiver-only part.
