# Caller examples

Runnable examples against a deployed gateway. Set the endpoint and a
credential first — a key comes from the operator's `tenant-keys issue`
and stays out of every command line:

```bash
export OPENAGENTS_BASE_URL="http://127.0.0.1:8080"
export OPENAGENTS_API_KEY="oak_acme.…"
```

## curl

```bash
# The doors this credential can reach.
curl -s "$OPENAGENTS_BASE_URL/v1/models" \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY"

# One call — a state and the questions file beside this README.
curl -s "$OPENAGENTS_BASE_URL/v1/systemone" \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -H "Idempotency-Key: ticket-t-1001" -H "X-Attempt: 1" \
  -d "$(jq -n --arg state "$(cat ticket.txt)" \
       '{model: "shared-kev", state: $state, questions: '"$(cat questions.json)"'}')"
```

## oak

```bash
# One state, on the command line.
oak ask --questions questions.json --model shared-kev \
  "I was charged twice on the March invoice."

# A batch, ordered, four calls in flight.
oak ask --questions questions.json --model shared-kev \
  --input ndjson --concurrency 4 --request-id batch-2026-10 \
  < tickets.ndjson

# Only the rows whose winning probability fell under 0.7.
oak ask --questions questions.json --model shared-kev \
  --input ndjson --uncertain-below 0.7 < tickets.ndjson \
  | jq 'select(.uncertain == true)'
```

`docs/decision-models/caller.md` covers input modes, exit codes, retries,
and what each answer field means. These examples run against any door
speaking the contract — a deployed gateway, a local `kev-serve`, or
TypeSafe's `api.typesafe.ai` with a `ts-` key.
