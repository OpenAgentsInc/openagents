# Grid Foreman host

The Grid Foreman is the giant Foreman robot on the Grid homepage
([plan](../foreman/grid-foreman-plan.md), #12444, #12445). One process,
`grid-foreman serve`, holds its Nostr key, walks its patrol, and publishes its
pose and status line on `wss://relay.openagents.com` in the `verse-bare`
world, as a player would. Every browser draws that one key as the Foreman, so
every viewer sees it in the same place.

## Where it runs

| Item | Value |
| --- | --- |
| VM | `oa-foreman-1`, `e2-small`, `us-central1-a`, Debian 12, no external address (Cloud NAT) |
| Service account | `oa-grid-foreman@openagentsgemini.iam.gserviceaccount.com`, which can read only the secret below |
| Secret | `grid-foreman-nostr-key` in Secret Manager: 64 hex characters |
| Public key | `2d9fad1a83555dc66198d94d58a24ea1541ba8857d2407c456635a3421456f36` (`grid_foreman::PUBKEY`) |
| Unit | `deploy/systemd/grid-foreman.service`, `Type=notify`, `WatchdogSec=120`, `Restart=always` |
| Key fetch | `deploy/grid-foreman/fetch-key.sh`, run as root by `ExecStartPre`, writes `/var/lib/grid-foreman/key` (0600) |
| Binary | `/opt/grid-foreman/current/grid-foreman`, a static `x86_64-unknown-linux-musl` build |

The host listens on nothing. It opens the relay connection and reads
`https://openagents.com/v1/foreman/status` once a minute for the issue counts
it shows at the terminal.

## Build and deploy

Build from a commit rebased on `origin/main`, copy the binary to a
commit-named path, and install it:

```sh
commit=$(git rev-parse --short HEAD)
cargo zigbuild --locked --release -p grid-foreman --features host \
  --bin grid-foreman --target x86_64-unknown-linux-musl
mkdir -p "$OPENAGENTS_SCRATCH/grid-foreman-$commit"
cp "$CARGO_TARGET_DIR/x86_64-unknown-linux-musl/release/grid-foreman" \
  "$OPENAGENTS_SCRATCH/grid-foreman-$commit/"
gcloud compute scp --tunnel-through-iap --zone us-central1-a \
  "$OPENAGENTS_SCRATCH/grid-foreman-$commit/grid-foreman" \
  deploy/systemd/grid-foreman.service deploy/grid-foreman/fetch-key.sh \
  oa-foreman-1:/tmp/
gcloud compute ssh oa-foreman-1 --tunnel-through-iap --zone us-central1-a -- "
  sudo install -d -m 0755 /opt/grid-foreman/$commit &&
  sudo install -m 0755 /tmp/grid-foreman /opt/grid-foreman/$commit/grid-foreman &&
  sudo install -m 0755 /tmp/fetch-key.sh /opt/grid-foreman/fetch-key.sh &&
  sudo ln -sfn /opt/grid-foreman/$commit /opt/grid-foreman/current &&
  sudo install -m 0644 /tmp/grid-foreman.service /etc/systemd/system/ &&
  sudo systemctl daemon-reload && sudo systemctl enable --now grid-foreman &&
  sudo systemctl restart grid-foreman && readlink /opt/grid-foreman/current"
```

The first install also creates the service user:
`sudo useradd --system --home /var/lib/grid-foreman --shell /usr/sbin/nologin grid-foreman`.

Check it after a deploy: `journalctl -u grid-foreman -n 20` shows
`serving as <pubkey>` and `online`; `readlink /opt/grid-foreman/current`
names the commit.

## Stop it

`gcloud compute ssh oa-foreman-1 --tunnel-through-iap --zone us-central1-a --
sudo systemctl stop grid-foreman`, or stop the VM with
`gcloud compute instances stop oa-foreman-1 --zone us-central1-a`. Within 10
seconds, every page draws the Foreman parked at the terminal with `BACK SOON`.

## Rotate the key

1. Make a new key without printing it: `grid-foreman new-key --state DIR`
   prints only the public key.
2. Add it as a new version of `grid-foreman-nostr-key`
   (`gcloud secrets versions add grid-foreman-nostr-key --data-file DIR/key`),
   then delete `DIR/key`.
3. Set `grid_foreman::PUBKEY` and the relay's `GRID_FOREMAN` to the new public key, or set the relay's
   `NOSTR_RELAY_WORLD_POPULATION_EXEMPT`, and deploy the website and relay.
   Until then, `OPENAGENTS_FOREMAN_PUBKEY` on the website names it.
4. Restart the service, which fetches the latest version.

## The relay

The relay's world population cap counts the Foreman as a player unless its key
is in `NOSTR_RELAY_WORLD_POPULATION_EXEMPT` (a comma list of public keys),
which defaults to the Foreman's key. An
exempt key is never refused for a full world and never counted, so the Foreman
never takes a visitor's place.

### Deploying the relay for the exemption

The exemption ships in the relay image from `6500eed27a` on (#12445). It
matters only when the Grid is full, so a relay deploy can wait for a quiet
window. Follow [the relay runbook](runbook-cloud-run.md), steps 2 to 4,
from a clean checkout of `main` at or after that commit:

```sh
TAG=$(git rev-parse --short=10 HEAD)
gcloud builds submit --project openagentsgemini \
  --config deploy/nostr-relay.cloudbuild.yaml --substitutions _TAG=$TAG .
gcloud run deploy openagents-nostr-relay \
  --image us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/nostr-relay:$TAG \
  --no-traffic --tag next --region us-central1 --project openagentsgemini
# The runbook's step 3 smoke checks against the next tag, then:
gcloud run services update-traffic openagents-nostr-relay \
  --to-revisions <new-revision>=100 --region us-central1 --project openagentsgemini
```

No environment change is needed: `NOSTR_RELAY_WORLD_POPULATION_EXEMPT`
defaults to the Foreman's key. Afterwards,
`nak req -k 33301 -a 2d9fad1a83555dc66198d94d58a24ea1541ba8857d2407c456635a3421456f36 -l 1 wss://relay.openagents.com`
still shows its state.

## Commands to agents (#12448)

The Foreman starts coding agents that open draft pull requests, under the
rules in [the plan](../foreman/grid-foreman-plan.md#commanding-agents-p4).
Commands are on by default (owner, 2026-10-22); every guardrail stays:
signatures, the two actions, the caps, the admin pause, the audit log, and
never merging, deploying, or spending.

| Variable on `openagents-web` | Value |
| --- | --- |
| `OPENAGENTS_FOREMAN_COMMANDS` | `off` refuses every command with `foreman_off`; anything else, or unset, is on |
| `OPENAGENTS_FOREMAN_OWNER` | Optional: the account ID whose computers run the agents. Unset, it is the admin who last published the desktop Foreman's record (`openagents foreman publish`) or asked for an agent |
| `OPENAGENTS_FOREMAN_WORKER_AGENT` | Optional: that account's saved agent that does the work; it must not have standing authority. Unset, the site saves "Grid Foreman worker" there (Claude Code, the computer's own access, no standing authority) |
| `OPENAGENTS_FOREMAN_WORKER_HOST` | Optional: the computer. Unset, the account's online computer that runs Claude Code, preferring the `openagents` project |
| `OPENAGENTS_FOREMAN_WORKER_PROJECT` | Optional: the app project, with `_WORKER_HOST` |
| `OPENAGENTS_FOREMAN_ORIGINS` | Optional: origins a command may be signed for (default `https://openagents.com`) |

- `GET /v1/foreman/commands` shows whether commands are on, whether a
  worker account is known, today's counts, the agents the Foreman started,
  open requests, and the audit log.
- An admin pauses everything with `POST /v1/foreman/commands/pause`
  `{"paused": true}`, and asks for an agent on an issue with
  `POST /v1/foreman/commands/requests` `{"issue": N}`; that admin's account
  becomes the worker account unless `OPENAGENTS_FOREMAN_OWNER` is set.
- The host sends at most one command a minute: an admin's request first,
  then a draft pull request ask for an agent that has worked three hours
  without one, then the least recently updated unassigned issue it hasn't
  tried. `--no-auto-start` on its unit's command line stops that last one.
  It sends nothing until the site knows a worker account.
- The site refuses past 5 starts and 10 draft asks a UTC day, 3 running
  agents, or one command a minute. Merging, deploying, and spending stay
  with an admin.

### Standing objectives

The server imports `docs/foreman/objectives.json` from its build and keeps
immutable catalog revisions under `grid-foreman/objectives/catalog-HASH.json`.
`grid-foreman/objectives/attempts.json` saves each exact assignment before
sending it to the app. Concurrent requests and retries reuse its request ID,
brief, agent revision, computer, and project. Corrupt or unavailable history
stops objective selection instead of resetting it.

`GET /v1/foreman/commands` adds `eligible_issues`, `next_exploration`, and
`objective_catalog_revision`. A failed or incomplete GitHub read returns
`eligible_issues: null`; the host does not interpret that as an empty backlog.
A complete read is shared for 60 seconds. The pinned host signs
`agent.explore` with the server-selected objective, catalog revision, and
request ID. The server rechecks its selection and saves the assignment
before dispatch. One objective exploration runs at a time, within the existing
start limits and pause control. Issue #12469 remains excluded.

Configure the worker account, online device ID, and project ID with the
variables above. `OPENAGENTS_FOREMAN_GITHUB_TOKEN` supplies authenticated
repository reads. Objective definitions do not provide merge, deploy, or
spend authority. The current path uses app assignments; native WORK/AUTO
publication and persistent Kiln execution are separate integrations.
