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
