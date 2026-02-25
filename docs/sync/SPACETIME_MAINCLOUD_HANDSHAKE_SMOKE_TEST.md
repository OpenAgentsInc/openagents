# Spacetime Maincloud Two-Client Handshake Smoke Test

Date: 2026-02-25
Status: active operator check
Owner lanes: Runtime, Desktop, Infra

## Purpose

Provide an immediate, repeatable check that two clients can connect to the same Maincloud database and that active connection count is observable.

## Automated Entry Point

```bash
scripts/spacetime/maincloud-handshake-smoke.sh --db "$OA_SPACETIME_DEV_DATABASE"
```

## Preconditions

1. You are logged in: `spacetime login`
2. Database has a module with:
   1. `active_connection` table
   2. `client_connected` reducer inserting/updating sender identity rows
   3. `client_disconnected` reducer deleting sender identity rows
3. Current dev database identity:
   1. `c2003d5910b79ed770057c5ccff7af287ba3463569b04e9d827b5cdc949294f9`

## Command Sequence

```bash
DB="c2003d5910b79ed770057c5ccff7af287ba3463569b04e9d827b5cdc949294f9"

spacetime subscribe "$DB" "SELECT * FROM active_connection" \
  --server maincloud --anonymous --timeout 20 --print-initial-update --yes >/tmp/oa-spacetime-sub1.log 2>&1 &

spacetime subscribe "$DB" "SELECT * FROM active_connection" \
  --server maincloud --anonymous --timeout 20 --print-initial-update --yes >/tmp/oa-spacetime-sub2.log 2>&1 &

sleep 4

spacetime sql "$DB" "SELECT COUNT(*) AS connected_clients FROM active_connection" --server maincloud
spacetime sql "$DB" "SELECT connection_id, identity FROM active_connection" --server maincloud

wait

spacetime sql "$DB" "SELECT COUNT(*) AS connected_clients FROM active_connection" --server maincloud
```

## Expected Result

1. While both subscriptions are active:
   1. `connected_clients` should be `2`.
2. After both subscriptions time out/disconnect:
   1. `connected_clients` should return to `0`.

## Verified Run (2026-02-25)

Observed:

1. During active subscriptions: `connected_clients = 2`
2. After disconnect: `connected_clients = 0`

This confirms basic Maincloud handshake visibility and lifecycle cleanup behavior.
