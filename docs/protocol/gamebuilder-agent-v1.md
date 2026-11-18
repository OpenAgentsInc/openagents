# A GameBuilder build agent, v1

GameBuilder (`gamebuilder.org`, a listed first-party site) starts an agent
job when a person accepts its offer to build something: one piece of work on
one game, in the person's words. This is its first slice. It is one durable
account turn ([account-chat-turn-v2alpha1](account-chat-turn-v2alpha1.md)),
run by the hosted runner (`oa-agent`) like any durable turn. The agent has no
tools: its only effect is its answer's plan lines, which the GameBuilder page
checks against the engine's property registry and the level's limits before it
makes any.

| Method and path | Request / response |
| --- | --- |
| `POST /api/gamebuilder/agents/v1` | Start request → 202 ControlReply (`turn_id`, `state`) |
| `GET /api/gamebuilder/agent/v1?id=TURN` | The durable Turn: state, the answer so far, usage |
| `GET /api/gamebuilder/agent/events/v1?id=TURN&after=N` | Its EventPage after a cursor |

The start request, every field present and nothing else:

```json
{"schema": "gamebuilder.agent.start.v1", "operation_id": "gb_...",
 "game": "gamebuilder.org world", "job": "disable physics",
 "brief": "the level as it stands and the plan lines the agent may write"}
```

`operation_id` is 1–80 bytes and makes a retry the same turn; `game` 1–64
bytes; `job` 1–1,200; `brief` 1–14,000. `gba_submit`
(`coax/web/api/gamebuilder-agent-wire.coax`) writes the SubmitRequest: a new
conversation whose first block names the game and the job and says the agent
has no tools, and whose second is the brief. Anything else is 400.

Admission: any logged-in person's own account, on the team beta or not
(since 2026-11-17; `act_funded_turn`, `account-chat-policy.coax`). Nobody
logged in is 401. The limits are the durable turn's: at most 32 queued or
running turns an account (429, in words with the number), the fixed model's
2,048 output-token cap, and `inference.c`'s free key and daily quotas (500
requests and $2.00 an account a day). Nothing is charged. The turn is stored
as admitted for a build agent (`chat.turns_v2.admitted_for =
'gamebuilder-agent'`, migration 0214), and that stored word is what admits it
again when the hosted runner takes it up (`ctw_grant`), where no route or
`Origin` is known. Every other durable turn keeps the team beta's funding
(INVARIANTS.md, "A GameBuilder build agent is any person's own account's").
A limited session (an email login on an account with a key
or passkey) may start one: `ap_limited_write` names this path, and the durable
submit honours it only on this path (INVARIANTS.md, "A limited session writes
only the account chat's stateless completions and a build agent"). A POST
whose `Origin` is another site is refused.

Reads are the durable turn's own reads, checked for the owner each time.
`gamebuilder.org`'s load balancer sends these three paths here
(`scripts/web-deploy.sh domain` in the GameBuilder repository).

Not yet: a list of an account's agents, Stop and messages from the page,
claims on a region of a level, per-agent undo, and an agent that reads the
level from a server copy rather than the brief.
