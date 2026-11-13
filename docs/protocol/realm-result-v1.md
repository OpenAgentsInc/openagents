# One result for every realm operation

Issues: [#13436](https://github.com/OpenAgentsInc/openagents/issues/13436)
(the shape), [#13435](https://github.com/OpenAgentsInc/openagents/issues/13435)
(effect and approval on each command), [#13437](https://github.com/OpenAgentsInc/openagents/issues/13437)
(every action names what it touched). Source:
[docs/research/summer-what-to-adapt.md](../research/summer-what-to-adapt.md).
Code: `metaverse-engine/editor/op-result.coax` (the one codec and the one list
of codes), `apps/openagents-cli/commands.coax` (`REALM_COMMANDS`),
`apps/openagents-cli/realm.coax` and `realm-world.coax`.
Profile: `proto/world/result-v1.json`, checked by
`coax/web/tool.sh metaverse-engine/tools/op-result-profile.coax proto/world/result-v1.json`.

## Each command says what it does

Every row of `REALM_COMMANDS` has an effect and an approval rule. The CLI's
help and the MCP tool's description end with both, such as
`[write; the owner]`.

| Effect | Means |
| --- | --- |
| `read` | Changes nothing |
| `write` | Changes the realm. Undo takes it back |
| `publish` | Changes what visitors see |
| `player-facing` | Other people see or hear it |
| `spend` | A model is paid for |
| `destructive` | It can't be undone |

| Approval | Means |
| --- | --- |
| `none` | Reads only |
| `owner` | The realm's owner or an admin. The server checks |
| `confirm` | Two steps. Without `--yes` the command does nothing and answers `needs-approval`. With `--yes` it runs |

A check holds that every row that is not a read has a rule, and that every
destructive row is two-step.

## The result

```json
{"schema":"realm.result.v1","state":"done","verb":"world-place","realm":"splice","effect":"write","rev":5,"build":[1,1,2,1],"page":"/worlds/splice","key":"k-1"}
```

Every result names what it touched: the realm, the revision it stands at after
the operation (0 when not known), the pins of the build that reads it (engine,
rules, content, wire) and the page that shows it.

| State | Adds | Means |
| --- | --- | --- |
| `done` | | Saved. `rev` is the new revision |
| `running` | `job` | It goes on. Ask again with the job's ID |
| `needs-approval` | `approval` | Nothing was done. Repeat it with `--yes` |
| `dry-run` | `level` | Nothing was saved. The level says how far the check went |
| `refused` | `code`, `retry`, `words`, `edit` | Nothing was saved. The words say why, naming the rule or the zone |
| `unknown` | `code`, `retry`, `words` | The request was sent and no answer came. It may have been saved |

With `--json` a realm command's report carries it as `"result"`. Without, the
command prints one line, such as
`refused (stale-revision; a retry can help): the world changed since you read it`.

### Dry runs say their level

`--dry-run` works on every command that is not a read.

| Level | Means | Which commands reach it |
| --- | --- | --- |
| `schema` | The arguments are a well-formed request | Every row |
| `world` | Checked against the world as it stands now | |
| `change` | The change was worked out, and is in the answer | |
| `copy` | Carried out whole in a throwaway copy of the world | A world's rows: the realm's own check and the walk of the run both ran |

### Keys

`--key K` works on every command that is not a read. The first result sent
with a key is kept in `~/.openagents/cli/results/K`. A repeat with the same
key answers that result again and sends nothing. A dry run, a wait for
approval and an unknown outcome are not kept, so the key can be used again.

The keys are kept on the computer that sent the command. Two computers that
use one key do not see each other's results; the revision fence is what stops
a repeat there (`stale-revision`).

### Codes

A refusal carries a diagnostic code (`proto/README.md`, "Diagnostic codes").

| Number | Code | Retry | Means |
| --- | --- | --- | --- |
| 1 | `stale-revision` | yes | Made from another revision than the realm's. Read it again, then repeat |
| 2 | `edit-refused` | no | The realm's own check refused an edit |
| 3 | `not-permitted` | no | The caller may not do this to this realm |
| 4 | `not-found` | no | No realm, entity or step has this name |
| 5 | `bad-request` | no | The arguments are not a request this operation takes |
| 6 | `nothing-to-do` | no | Nothing to undo, nothing new to publish |
| 7 | `no-answer` | yes | No answer came. Read the realm's revision before repeating |
| 8 | `operation-failed` | yes | The server or the tool failed. Nothing was saved |

## Pictures say what they are of

A frame taken with an edit (`--shot OUT.png`), `realm shot` and
`coax drive ... screenshot` each say the realm, the revision the page stood
at, the page, the camera and the GPU. A CLI or drive picture is always
headless Chrome's, and says so. A headless picture is not proof of what real
Chrome shows.

## What is not on this shape yet

The older made-realm commands (`realm move`, `realm place` and the rest) still
print their own report. Their two-step rule, dry runs and keys work as above,
and a repeat with a key answers in this shape. They move onto it when made
realms move onto the world document (E13, #13422).
