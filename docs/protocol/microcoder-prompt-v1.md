# Microcoder prompt v1

Each step, Microcoder sends the model three things: a system prompt, an
answer schema and the state. The model answers with one action. This page
names the bytes, because the model's prompt cache serves a request's start
only while it is byte for byte the last request's start. A changed byte in
the prefix is a changed price.

- Profile and goldens: [`proto/coder/microcoder-prompt-v1.json`](../../proto/coder/microcoder-prompt-v1.json)
- Check: `coax/web/tool.sh coax/lib/microcoder/prompt-profile.coax proto/coder/microcoder-prompt-v1.json`
- Registry row: `microcoder-prompt-v1` (step C11, [#13250](https://github.com/OpenAgentsInc/openagents/issues/13250))

## The three parts

| Part | Code | In the profile |
| --- | --- | --- |
| System prompt | `MC_SYSTEM` in [`coax/lib/microcoder/loop.coax`](../../coax/lib/microcoder/loop.coax) | `system` |
| Answer schema, sent as `next_action` | `MC_SCHEMA` in [`action.coax`](../../coax/lib/microcoder/action.coax) | `schema_name`, `schema` |
| State | `mc_state_json` in [`state.coax`](../../coax/lib/microcoder/state.coax) | `states` |

The state is one JSON object. Its members, in order:

| Member | Meaning |
| --- | --- |
| `environment` | Facts about the machine and the workspace, as the caller gave them |
| `task` | The user's message |
| `route` | The routing decision, as given |
| `left_out` | How many of the earliest steps no longer fit. Only when some don't |
| `earlier` | Older steps, compacted. Only when some were compacted |
| `actions` | Recent steps, whole: `step`, `rationale` and `results` |
| `notes` | What the loop wrote for itself |

A result is one of:

- `{"edit": PATH, "applied": true}` or `{"edit": PATH, "failed": WHY}`
- `{"command": TEXT, "exit": N, "output": TEXT}`, with `"timed_out": true` when it was killed
- `{"command": TEXT, "refused": WHY}`
- `{"view": PATH, "content": TEXT}`, or `{"view": PATH, "missing": WHY}` for a file that is not there or is too large to show

The profile's `states` show each exactly.

## The answer

The model answers with an object that matches `schema`:
`rationale`, `edits` (`path`, `old`, `new`), `commands`, `view`, `finished`,
`reply` and `ask` (`none` or `question`). `mc_action_parse` reads it and
refuses an answer that fails the schema or its meaning (a finished step
with no reply).

## What the check holds

- The profile is the bytes the code gives for one fixed session of six
  steps: the state before the first step and after each. `--write` printed
  it from the code. The check builds it again and compares every byte.
- Each state begins with the state before it, up to the end of that
  state's steps. This is the prefix the cache serves.
- Each of the profile's `replies` reads as an action.

The model is the only reader of these bytes, so the goldens are a record,
not a second implementation. To change the prompt on purpose, run
`prompt-profile.coax --write`, commit the new profile with the change, and
say in the commit what the change costs the cache.
