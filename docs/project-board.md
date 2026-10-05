# The OpenAgents project board

Every open issue in `OpenAgentsInc/openagents` is on the required org project:
**OpenAgents**, <https://github.com/orgs/OpenAgentsInc/projects/19>
(created 2026-10-02, linked to this repository).

The owner also requested a dedicated public project for the terminal effort:
[OpenAgents Terminal and Workbench](https://github.com/orgs/OpenAgentsInc/projects/20).
Its issues stay on both projects. The [terminal issue directory](terminal/issue-roadmap.md)
records the complete backlog, delivery milestones, and native blockers.
Project 20 separates **Today MVP** from every follow-up and conditional
research issue; it does not replace project 19.

## Fields

| Field | Values | Meaning |
| --- | --- | --- |
| Status | Todo, In progress, Blocked, In review, Done | Where the work is. Agents keep it current (rule in `AGENTS.md`). |
| Area | router, terminal, cloud/Boat, payments, plugins, background, wallet/Spark, BYOK, claims/issue-flow, host, docs, other | Owning surface. |
| Priority | P0, P1, P2 | P0 is owner-flagged work, including today's terminal MVP #10642–#10644. Earlier examples include cloud/Boat #10216–#10227 and the router-in-terminal umbrella #10204. |
| Size | S, M, L | Rough effort. |
| Executor | Coder, Subagent, Owner | Coder-sized (`coder-sized` label, `openagents chat work --issues coder-sized`), a full agent, or an owner decision. |
| Blocked by | text, e.g. `#10187 #10189` | Mirror of the open native dependencies, for the board view. |

Dependencies are GitHub's native issue dependencies ("blocked by"), visible on
each issue page. Umbrellas (#10155, #10200, #10204) are blocked by their
children.

## Commands

```sh
scripts/project-status.sh 10187 in-progress          # claim: you started it
scripts/project-status.sh 10190 blocked --blocked-by "10189"   # adds the native dependency too
scripts/project-status.sh 10187 in-review
scripts/project-status.sh 10187 done                 # closing the issue does this on the next sync
scripts/project-sync.sh [--dry-run]                  # reconcile the whole board
```

`project-sync.sh` adds missing open issues as Todo, sets closed issues to
Done, sets issues with a live Coder claim marker (`<!-- openagents-coder-claim`
not followed by `<!-- openagents-coder-release -->`, younger than
`CLAIM_HOURS`, default 6) to In progress, moves open issues with an open native
blocker to Blocked, and moves Blocked issues whose blockers have all closed back
to Todo. It leaves In progress and In review set by hand alone until the issue
closes.

Raw `gh`, if the scripts are not at hand:

```sh
# item id for an issue (adds it if missing)
ITEM=$(gh project item-add 19 --owner OpenAgentsInc --url https://github.com/OpenAgentsInc/openagents/issues/N --format json --jq .id)
# set Status (option ids below)
gh project item-edit --project-id PVT_kwDOBubymc4BlgVZ --id "$ITEM" \
  --field-id PVTSSF_lADOBubymc4BlgVZzhkM-Bg --single-select-option-id 0eb80cca
# native "N is blocked by B"
gh api -X POST repos/OpenAgentsInc/openagents/issues/N/dependencies/blocked_by \
  -F issue_id=$(gh api repos/OpenAgentsInc/openagents/issues/B --jq .id)
```

`gh` needs the `project` scope: `gh auth refresh -s project`.

## IDs

Project: number `19`, node id `PVT_kwDOBubymc4BlgVZ`.

| Field | Field id | Options |
| --- | --- | --- |
| Status | `PVTSSF_lADOBubymc4BlgVZzhkM-Bg` | Todo `5cbde1c1`, In progress `0eb80cca`, Blocked `c7d7f909`, In review `1868394a`, Done `42d2482f` |
| Area | `PVTSSF_lADOBubymc4BlgVZzhkM-Is` | router `9bf776b0`, terminal `dc1847db`, cloud/Boat `4c7a698b`, payments `5c30cd78`, plugins `a8091348`, background `bd6e0434`, wallet/Spark `d1b62b56`, BYOK `5788e489`, claims/issue-flow `113e6bfc`, host `800bbeff`, docs `e0f201f3`, other `1fd13177` |
| Priority | `PVTSSF_lADOBubymc4BlgVZzhkM-Jo` | P0 `92fd0307`, P1 `819f2359`, P2 `1f747d22` |
| Size | `PVTSSF_lADOBubymc4BlgVZzhkM-J8` | S `63dca521`, M `47306871`, L `d5b8f8db` |
| Executor | `PVTSSF_lADOBubymc4BlgVZzhkM-Ls` | Coder `45cd6a93`, Subagent `aa80e9f5`, Owner `2dbe2508` |
| Blocked by | `PVTF_lADOBubymc4BlgVZzhkM-L0` | text |

The issue flow (`openagents chat work`, `openagents issue claim|release`) is
to set Status itself when it claims, releases, and lands (#10203); until that
lands, run `scripts/project-status.sh` by hand.

## Terminal project updates

The existing status and sync scripts target project 19. When an issue belongs
to the terminal project, update its **Status** and **Blocked by** fields on
project 20 as well. Its status names match the global board. Keep native
dependencies authoritative; a related integration is not a completion blocker.

Project 20 has **Delivery**, **Workstream**, **Priority**, **Size**, and
**Scope** fields. Delivery also maps to a repository milestone. Only #10642,
#10643, and #10644 belong to **Today MVP**; the remaining issues do not gate
the first Grid and standalone release. **Conditional research** needs an
explicit measured outcome or recorded decision before implementation becomes
a product commitment.

For a terminal status update, obtain the item ID with `gh project item-add 20`
and use `gh project item-edit` with project ID `PVT_kwDOBubymc4Blzsv`.
Read current field and option IDs with
`gh project field-list 20 --owner OpenAgentsInc --format json`.
Claim the implementation issue before starting it and release its claim when
stopping. Closing code-complete work does not claim an owner-only device,
payment, or store step has passed; record that step in `NEEDS_OWNER.md`.
