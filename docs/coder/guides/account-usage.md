# Read Codex account quota

Use the root OpenAgents CLI to read the same account quota that the desktop
app displays:

```sh
openagents accounts usage --device local --account active --json
openagents accounts usage --device DEVICE_UUID --harness codex --account ACCOUNT_ID --json --refresh
```

`--device` is required: use `local` for the connected app engine's device or
an exact device UUID for a paired device. `--account` defaults to `active`;
an explicit ID matches exactly. Missing or multiple matches fail without
switching accounts. Only the Codex harness is supported.

The local OpenAgents app engine must be running. `--ipc-port PORT` selects
its loopback port; otherwise the command uses `OPENAGENTS_IPC_PORT`, its
legacy `ZERON_IPC_PORT` fallback, or the desktop default. Requests for a
remote device use that engine's existing connection to the device.

The default read uses the engine's last saved quota observation. `--refresh`
asks that same engine to refresh **all accounts on the selected host**,
subject to its 30-second minimum interval and provider retry delays. It
does not guarantee a newer observation. The account selected before the
refresh remains selected even if the host's active account changes.

This is account quota, separate from chat context utilization, message
token counts, and the CLI's `capacity` limit book.

## JSON contract

Successful account reads emit one document with schema
`openagents.accounts.usage.v1`. All timestamps named below use UTC.

| Field | Meaning |
| --- | --- |
| `deviceId` | Exact device ID that supplied the account snapshot. |
| `account` | Selected account's `id`, `harness`, `active`, and nullable `planLabel`. |
| `windows` | Every supplied quota window: `label`, `usedFraction` (0–1), `usedPercent` (fraction × 100), and nullable `resetsAt` (RFC 3339). An empty list means no windows were supplied. |
| `usageFetchedAt` | Last successful engine probe, in Unix milliseconds, or `null`. |
| `cachedAt` | Same value as `usageFetchedAt`: when the cached windows were observed. |
| `observedAt` | CLI read time, in Unix milliseconds; this is not a provider fetch time. |
| `ageMs` | `observedAt - usageFetchedAt`, or `null` if never fetched or the fetch time is in the future. |
| `freshForMs` | Freshness age threshold: 300000 milliseconds, matching mobile's five-minute refresh cadence. |
| `state` | `fresh`, `stale`, `neverFetched`, or `unavailable`, as described below. |
| `stale` | `true` unless `state` is `fresh`; one flag for "don't treat this as current". |
| `available` | Whether any windows were supplied. |
| `usageError` | Engine's last usage error, or `null`; last-good windows remain present when a later probe fails. |
| `refresh` | `requested`, `scope` (`host`), `result`, and nullable `error`. |

`fresh` requires windows fetched within five minutes, no usage or refresh
error, and no supplied reset time at or before `observedAt`. `stale` means
windows exist but one of those conditions fails. Freshness describes the
cached observation; it does not promise that a provider's quota is unchanged.
An unknown or future fetch time cannot count as fresh.

`neverFetched` means no windows, successful fetch time, or usage error was
supplied. `unavailable` means no windows were supplied but a fetch time or
usage error exists. Missing data stays empty or `null`; it never becomes
zero usage or an estimated remaining balance.

Refresh results are `notRequested`, `updated` (the selected account's
successful fetch timestamp advanced), `cached` (it did not advance), or
`failed` (the refresh RPC failed). A failed refresh retains the selected
account's preceding snapshot and reports a separate `refresh.error`.
`usageError` continues to describe that snapshot's engine probe. A `cached`
result may mean the engine throttled or deferred the probe; the CLI does
not infer the reason from an unchanged timestamp.

Exit status is **0** when an account was read, including stale,
never-fetched, unavailable, or failed-refresh results. Inspect `state`,
`available`, `usageError`, and `refresh` before deciding to start work.
Exit **1** means selection or initial communication failed; exit **64**
means invalid arguments. Errors under `--json` use the CLI's existing
`{"error":"..."}` format. Transport error details and credentials are
not included.
