# Chat worker usage stats

The chat worker has no usage limit
([#10120](https://github.com/OpenAgentsInc/openagents/issues/10120)).
Instead, it records every job it reads, so usage questions are one command
away. This page describes the log and the queries.

## The usage log

`coder-worker` appends one JSON line per job to
`/var/lib/coder-worker-chat/usage/YYYY-MM-DD.jsonl`, one file per UTC day
(`coder::relay::usage`). The directory is `CODER_WORKER_USAGE_DIR` when it is
set, else `usage/` in the systemd unit's state directory
(`StateDirectory=coder-worker-chat`), so the shipped unit logs with no extra
setting. `CODER_WORKER_USAGE_DIR=off` turns the log off. The worker's start
lines name the directory (`usage   one line per job in …`).

A line holds no message text: only ids, words from fixed vocabularies,
counts, and times.

| Field | Meaning |
| --- | --- |
| `time` | When the job arrived, RFC 3339 UTC with milliseconds. |
| `key` | The caller's public key (hex), the request's verified signer. The website signs each visitor with a key of its own. |
| `kind` | `turn`, `rank`, `probe`, `delegation`, or `unread` (did not decrypt). |
| `client` | The client's word, such as `openagents-mobile`, when it sent one. |
| `surface` | `phone`, `desktop`, `terminal`, or `web`, from the request's context. |
| `route`, `tier`, `answer` | The chat router's route, the tier it served, and the bank entry (`id@version`) that supplied text. |
| `model`, `door` | The model the result names (`bank:…` for a prepared answer) and the door that served it (`openrouter.ai`, `ai-gateway.vercel.sh`, `bank`). |
| `jev_door`, `jev_model` | The Jev door that judged the turn and its model. |
| `first_token_ms`, `total_ms` | Time from arrival to the first words, and to the end. |
| `tokens_in`, `tokens_out`, `cost_usd` | What the door reported, when it did. |
| `outcome`, `code` | `answered`, `failed` (the worker's own door failed, `internal`), `refused` (a typed code such as `busy` or `stale`), or `unfinished`. |
| `bytes_in`, `bytes_out` | The request's ciphertext length and the reply's text length. |

## Pulling stats

On the worker VM (`oa-coder-worker-1`), as root or `coder-worker`:

```sh
sudo /opt/coder-worker/chat/coder-worker usage
```

`usage` prints a table: jobs, answered, failed, refused, distinct keys,
tokens in and out, and the median time to first words and to the end, one
row per group and an `all` row. It takes these options:

| Option | Effect |
| --- | --- |
| `--by key\|surface\|route\|model\|day\|kind\|outcome` | What a row is. The default is `day`. |
| `--since YYYY-MM-DD` | Read day files from that date on. |
| `--json` | Print the rows as JSON. |
| `--dir DIR` | Read another directory, such as a copy. The default is `CODER_WORKER_USAGE_DIR`, else `usage/` in `STATE_DIRECTORY`, else `/var/lib/coder-worker-chat/usage`. |

Example queries:

```sh
# Jobs per day.
sudo /opt/coder-worker/chat/coder-worker usage --by day

# This month, by surface: phone, desktop, terminal, web.
sudo /opt/coder-worker/chat/coder-worker usage --since 2026-10-01 --by surface

# The heaviest callers.
sudo /opt/coder-worker/chat/coder-worker usage --by key | head -20

# Which models answered, and their tokens.
sudo /opt/coder-worker/chat/coder-worker usage --by model

# Why jobs did not end in an answer.
sudo /opt/coder-worker/chat/coder-worker usage --by outcome
```

Read the files directly for anything the table does not group by, for
example with `jq`:

```sh
# Website jobs that took longer than 10 seconds.
# The directory is 0750 coder-worker, so the glob expands as root.
sudo sh -c 'cat /var/lib/coder-worker-chat/usage/*.jsonl' \
  | jq -c 'select(.surface == "web" and .total_ms > 10000)'
```

To query from a development machine, copy the directory down with
`gcloud compute scp --recurse --tunnel-through-iap` and run
`coder-worker usage --dir ./usage` from a checkout build.

## First release

Release `95d447a4b5` (2026-10-01) put the log live: the environment file
gained `CODER_WORKER_OPEN=1` in place of the quota line (the quota file line
is commented out), `CODER_WORKER_JOBS=64`, and
`CODER_WORKER_USAGE_DIR=/var/lib/coder-worker-chat/usage`; the previous file
is kept as `coder-worker-chat.env.bak-before-10120`. Four scratch turns from
`openagents chat --scratch` then read back as:

```text
surface   jobs  answered  failed  refused  keys  tokens_in  tokens_out  first_p50_ms  total_p50_ms
terminal     4         4       0        0     4       2403         238           763          1607
all          4         4       0        0     4       2403         238           763          1607
```

## Retention

The worker keeps 30 days of day files, today included, and deletes older
ones (#11042): once at start, and again when the first job of a new UTC day
is written. `CODER_WORKER_USAGE_DAYS` sets the window (a whole number of
days) or `forever` keeps every file; any other value stops the worker. The
start lines say the window (`usage   … kept 30 day(s)`) and how many files a
start deleted. Only `YYYY-MM-DD.jsonl` files are touched, so an archive or a
note in the directory stays. Copy day files elsewhere before they age out if
a longer history is needed; `--since` queries cannot reach past the window.
Each line is a few hundred bytes, so a million jobs is a few hundred
megabytes.
