# Custom agents

A custom agent is an agent you define: a name, a description,
instructions, and the coding agent it runs on, such as Claude Code or
Codex, with an optional model. You save it to your account, and then any
chat can use it.

## Make one on your phone

1. In [OpenAgents for iPhone](/docs/iphone), start a new session.
2. In the session's agent menu, **Create agent…** opens the form.
3. Give it a name and instructions, pick what it runs on, and save it.

**Edit agent…** in the same menu changes it later. A chat
that runs a custom agent says so under its title and in the composer.

The agent runs on a computer: one you [connected](/docs/connect-a-computer),
or OpenAgents Cloud. That computer needs the coding agent it asks for
installed and signed in. When it isn't, the agent uses the fallbacks you
listed, and then the computer's own default.

## Foreman

Foreman is our own custom agent. It looks after the work on the
OpenAgents repository: it reads the issues and chats, hands work to other
agents, and checks in on them. We publish Foreman's definition, so you
can start your own copy and change its instructions and model.

A copy keeps Foreman's instructions, but some of what Foreman does, such
as steering other chats and keeping its own log, still only works for
Foreman itself.

## Publish and copy

A definition you publish can be read by anyone, signed in or not, and
anyone can start their own copy of it. Only you add new versions. These
use the API:

| Request | Does |
| --- | --- |
| `POST /v1/agent-definitions` | Save a new agent to your account. |
| `GET /v1/agent-definitions` | List your agents. |
| `PUT /v1/agent-definitions/{id}` | Save a new version. |
| `POST /v1/agent-definitions/{id}/publish` | Publish it. |
| `POST /v1/published-agents/{id}/fork` | Start your own copy of a published agent. |

The API is at `https://api.openagents.com`, and the
[SDK reference](/docs/sdk) has every field.

Next: [OpenAgents Terminal](/docs/terminal).
