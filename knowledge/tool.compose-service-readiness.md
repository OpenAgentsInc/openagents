---
id: tool.compose-service-readiness
version: 1
kind: tool
title: Start and check a Docker Compose app by readiness, not by container start
summary: >-
  A started container is not a ready service. Validate the merged file with
  docker compose config, gate dependents on healthchecks with
  condition: service_healthy, wait with up --wait, and probe services by
  service name and container port from inside the project network.
tags: [docker, docker-compose, healthcheck, multi-service, readiness]
applies_when: >-
  Building, fixing, or checking an application made of several Compose
  services (web, worker, database, cache, proxy) where one service depends on
  another being reachable.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Docker, Compose file reference: services, depends_on (short and long syntax, condition: service_healthy) and healthcheck"
    - "Docker, docker compose up reference: --wait, --build, --force-recreate"
    - "Docker, docker compose config reference"
    - "Docker, Networking in Compose: services reach each other by service name on the default network"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

Compose starts containers in dependency order, but the short form of
`depends_on` only orders *creation*: a web process can start before its
database accepts connections, fail once, and exit or cache a broken state.
To wait for readiness:

- Give the dependency a `healthcheck` whose `test` uses a command that exists
  in *that* image (`pg_isready -U user`, `redis-cli ping`, `mysqladmin ping`,
  or a small `python -c`/`wget -qO-` probe; many slim images have no `curl`).
  Set `interval`, `timeout`, `retries`, and a `start_period` long enough for
  first-boot initialization.
- Use the long form in the dependent: `depends_on: {db: {condition:
  service_healthy}}`. `service_completed_successfully` is the form for a
  one-shot migration or seed job that must finish first.
- Start with `docker compose up -d --build --wait` so the command returns only
  when services are running and healthy, and fails if one is not.

Addressing is the other common fault. On the project network a service is
reached at `http://<service-name>:<container-port>`; `localhost` inside a
container is that container itself. The `ports:` mapping (`"8080:80"`) only
matters from the host. Environment files: `.env` in the project directory feeds
`${VAR}` interpolation in the Compose file, while `env_file:` feeds a
container's environment; they are different mechanisms.

Useful checks: `docker compose config` (the merged, interpolated model; it
fails on schema errors), `docker compose ps` (state and health),
`docker compose logs --no-color --tail=200 <svc>`, and
`docker compose exec <svc> sh -c '...'` to probe from inside. `down -v` also
deletes named volumes, which resets databases; use it deliberately.

## How to check

From a clean state (`docker compose down -v`, then `up -d --build --wait`),
exercise the real user path end to end through the published port: a request
that touches the web tier, the database, and any worker or cache. Then restart
one dependency (`docker compose restart db`) and confirm the app recovers
rather than staying broken, and that data that should persist survives a
`down` without `-v`.
