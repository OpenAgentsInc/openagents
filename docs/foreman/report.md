# Daily agent measurement report

```sh
openagents foreman report --agent AGENT_ID --day 2026-10-17 --json
openagents foreman report --agent AGENT_ID --day 2026-10-17 \
  --issues 12018,12019,12022 --decision-log /path/to/decisions.jsonl
```

The command only reads data. GitHub reads use the host's `gh` login, a
20-second call timeout, a 120-second total budget, and bounded pagination
and output. An incomplete GitHub read fails the report. The command never
runs a probe, starts a build, or contacts an OpenAgents account.

Without `--issues`, it selects updated issues whose claim names the agent
ID or whose number appears in that agent's observations. Claims recognize
`agent=ID`, `agent_id=ID`, and `session=ID` tokens in the shared claim marker.
`--issues` explicitly attributes selected issues when older claims identify
only a harness session. Issues named in the decision log are also included.

Defaults are `~/.openagents/agents/ID/decisions.jsonl` and
`~/.openagents/logs/build-sweep.log`. Override them with `--decision-log FILE`
and `--build-sweep-log FILE`. For a legacy `foreman/decisions.jsonl`, select
that file explicitly. Entries without an agent ID belong to the requested
ID; other agents' entries are ignored. No state is written.

## Measurements

Dates use `America/Chicago`, with `CDT` or `CST` and DST-aware day boundaries.
Open intervals are clipped to the day and capture time. JSON `null` and
text `unknown` mean a missing measurement; zero means zero recorded events.
Negative issue intervals show recorded steps that occurred out of order.

- Issue dates and claims come from GitHub. First commit uses the earliest
  referenced author date, falling back to the committer date. Commit-message
  references can also describe related issues. Author dates survive rebases.
- Code complete uses the latest `ready` or issue-branch `push` observation.
  Main's merge time is never substituted. A `branch` field also makes the
  collector read that branch's commits, but commit dates don't establish
  push times. First-claim and last-claim durations distinguish takeovers.
- Explicit stalls count their recorded duration. Working chat status counts
  silence beyond 20 minutes, or 10 for the agent's own turn. New output or
  an idle status ends the silence; repeated samples don't count it twice.
- Check-ins compare start and due times. Missing due times mean unknown
  timing; unanswered deadlines are missed. Actions need action observations.
  Legacy text decisions aren't treated as evidence that a command ran.
- Disk intervals use samples below 100 GiB, including before/after sweep
  readings. They end at recovery or capture time. Earlier gaps are unknown.
  Disk is a computer measurement, shared across agents on that computer.
- Production/owner probe writes need `wrote_data: true`; `POST /v1/visitors`
  counts without it because it creates a token. An uncounted repeated watch
  is one recorded incident, not an assertion that it sent one request.
- A permission refusal is a surprise unless an earlier `owner_list` named
  that deploy. Both incident targets are zero.
- Owner waits appear per item. The total is item-minutes; overlaps count
  separately. `coverage`, `sources`, and `notes` describe missing evidence.

## Recorded input

`--fixture FILE` reads only a JSON fixture, with no home or network lookup.
Its fields are `issues` and `commits` (GitHub REST objects), `events`,
`build_sweep_log`, `sources`, `notes`, and optional RFC 3339 `captured_at`.
Issues can include `agent_id` and REST comment objects in `comments`.
JSON inputs are limited to 16 MiB. Include events before midnight for open
intervals. The same event objects work as JSONL in `--decision-log`.

Each event needs `agent_id`, `kind`, and `at` (RFC 3339 or Unix seconds or
milliseconds). Unknown kinds appear in notes; invalid timestamps fail.

| Kind | Additional fields |
| --- | --- |
| `claim`, `ready`, `push` | `issue`, optional `branch` |
| `stall` | `id` or `chat_id`, `own_turn`, optional `ended_at` |
| `chat_status` | `chat_id`, `status` (`working` or `idle`), `last_output_at`, `own_turn` |
| `check_in_due` | Check-in `id` |
| `check_in`, `wake` | Check-in `id`, optional `due_at` |
| `action` | Optional `check_in_id`; otherwise the latest check-in |
| `owner_wait`, `owner_done` | Item `id`; `owner_wait` also needs `what` |
| `owner_list` | `deploys`: array of deploy IDs |
| `deploy_refused` | `deploy`, `permission: true`, `reason` |
| `probe` | `method`, `path`, `target` (`production` or `owner`), `wrote_data`, optional `count` |
| `disk` | `free_gib` |

Legacy `wake`, `decision`, `outcome`, and `authority` entries are accepted.
This command doesn't change engine recording.

The fixture `crates/openagents-cli/fixtures/foreman/2026-10-17.json`, under
agent ID `baseline-2026-10-17`, combines recorded GitHub dates, claim comments,
38 matching commits, the [chat review](chat-review.md), the
[close timeline](../audits/2026-10-17-nostr-identity-close-timeline.md), and the
first build-sweep sample. Audit observations have minute precision. Replay
it with `--fixture` and that agent ID. Tests check the completion delays,
21/33/35-minute takeovers, both stalls, the visitor write, and the permission
surprise within the audit's five-minute tolerance. No host check-ins were
recorded that day.
