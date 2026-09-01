# Gym and trace auth: the two-token reality

The CoderBench pipeline talks to two API surfaces with two different
credential requirements, and the CLI sends whichever bearer is stored for
the selected origin to both. This page says exactly what each surface
needs and how to hold both at once.

## What each surface requires

| Surface | Routes | Requirement |
| --- | --- | --- |
| Gym lifecycle | `POST /api/v1/gym/runs/start`, `POST /api/v1/gym/runs/:id/trials`, `PATCH /api/v1/gym/runs/:id`, `GET /api/v1/gym/runs` | Bearer with the `forge:write` scope **and** live operator (admin) standing, re-checked server-side on every call |
| Trace upload | `POST /api/v1/traces` (also driven by `openagents trace upload` and `openagents gym corpus import`) | Bearer with the `chat:account` scope |

Holding `forge:write` without operator standing still refuses: the scope
names what the token may ask for, and the server separately checks who is
asking. The refusal code for that case is `not_operator`.

## One token that carries both

Scopes are additive on one token, so the ordinary setup is a single login
that requests both:

```
openagents auth login --scope forge:write --scope chat:account
```

`--scope` is repeatable; the server settles the granted set and `openagents
auth status` reports it. The token is stored per origin, so run the login
against the origin you will bench (`--api-url https://openagents.com`, or
`--profile production|staging|local`). Headless machines use
`openagents auth login --headless` then `--resume`, or paste a token with
`openagents auth login --token-stdin`.

The scope does not create operator standing. If the account is not an
operator/admin on that origin, `forge:write` still refuses gym writes —
that is an account-standing change on the server, not a CLI flag.

## The pre-run check

`openagents gym run run <suite>` now preflights before registering
anything: it sends the cheapest authenticated gym GET
(`GET /api/v1/gym/runs?limit=1`) and translates a 401/403 into one refusal
naming the missing scope and the login command above. A run therefore
fails in the first second with a fixable sentence instead of surfacing
`not_operator` deep inside a Harbor run. Transport failures and 5xx do not
block the run — the suite-script path already degrades to post-hoc posting
when registration fails.

`bench/run-suite.sh` and the in-container `openagents coder --api-url`
both take the bare origin (no `/api/v1`); the CLI derives it once and
passes it through, so the same stored token works for registration on the
host and for the coder inside the container.
