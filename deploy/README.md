# Deployment assets

The files here are the source of truth for how `relay.openagents.com` and
the worker beside it run. Do not keep private copies of them on the host;
install them from a checkout and edit the installed environment files only.

| Path | What it is |
| --- | --- |
| `systemd/nostr-relay.service` | The hardened relay unit. |
| `nostr-relay.env.example` | The relay's environment template. |
| `caddy/`, `nginx/` | Public TLS termination in front of the relay. |
| `backup/` | The relay's Postgres and media backup pair, with its timer. |
| `systemd/coder-worker.service` | The hardened worker unit, same shape as the relay's. |
| `systemd/coder-worker-executor.conf` | Drop-in for a worker that answers through a local executor. |
| `coder-worker.env.example` | The worker's environment template, with the credential decision in its comments. |

The relay's runbook is
[`docs/deployment/runbook-debian-vps.md`](../docs/deployment/runbook-debian-vps.md).
The rest of this page is the worker.

## The worker

`coder-worker` is the fulfillment end of the relay door: it subscribes to
the relay for kind-`25900` job requests that name its public key, decrypts
each one, answers through a door, and publishes the reply. The relay stores
none of it, so a worker that is not connected when a request goes out never
sees it. That is why the worker is a service and not a shell session.
[`docs/coder/measurements/relay-transport.md`](../docs/coder/measurements/relay-transport.md) is the
measured proof of the transport;
[NIP-CJ](../nips/openagents/NIP-CJ.md) is the wire contract.

The unit follows `nostr-relay.service` line for line where the two agree: a
dedicated system user, an `EnvironmentFile` under `/etc`, an immutable
release directory with one `current` symlink, `Restart=on-failure` with a
start-rate limit, and the same hardening block. Where they differ is
network: the relay may bind one localhost port and reach nothing else, and
the worker may bind nothing and reaches out twice, to the relay and to the
door.

### The credential and admission decision

Three settings carry the decision the issue asked for, and the environment
template states it in its comments so the installed file carries it too:

- **One worker key.** `CODER_WORKER_SECRET` is the service's identity. Its
  public key is what a customer sets `CODER_WORKER` to.
- **One door key.** `CODER_DOOR_KEY` is the gateway credential the service
  spends. The relay sees ciphertext and knows nothing about who pays, so
  the spend control lives entirely in the worker.
- **An explicit allowlist.** `CODER_WORKER_ALLOW` names every customer the
  worker answers. Anyone else is refused with the typed code
  `not_admitted`, never silently, because a silent refusal reads as an
  outage from the terminal. A worker with `CODER_WORKER_ALLOW` unset
  answers whoever finds its public key, and public keys are not secret:
  never run one on a public relay.

### 1. Build and install the binary

Build from a checkout with the pinned toolchain
(`rust-toolchain.toml`), on the host or on a builder of the same
architecture:

```sh
cargo build --locked --release -p coder --bin coder-worker
cargo build --locked --release -p capability --bin capability-trust
```

Install under an immutable release directory with one `current` symlink,
as the relay does. The `capability-trust` binary is only needed for the
executor door; it does no harm beside a gateway worker.

```sh
sudo useradd --system --home /var/lib/coder-worker --shell /usr/sbin/nologin coder-worker
sudo install -d -o root -g root -m 0755 /opt/coder-worker/releases/<VERSION>
sudo install -o root -g root -m 0755 target/release/coder-worker \
  /opt/coder-worker/releases/<VERSION>/coder-worker
sudo install -o root -g root -m 0755 target/release/capability-trust \
  /opt/coder-worker/releases/<VERSION>/capability-trust
sudo ln -sfn /opt/coder-worker/releases/<VERSION> /opt/coder-worker/current
```

### 2. Generate the worker key

The worker does not generate its own key: it reads `CODER_WORKER_SECRET`
from the environment and refuses to start without it. Generate the secret
once, without letting it touch shell history or a log:

```sh
sudo install -d -o root -g coder-worker -m 0750 /etc/coder-worker
sudo install -o root -g coder-worker -m 0640 deploy/coder-worker.env.example \
  /etc/coder-worker/coder-worker.env
sudo sed -i "s|<YOUR_WORKER_SECRET>|$(openssl rand -hex 32)|" \
  /etc/coder-worker/coder-worker.env
```

Then replace the remaining placeholders with `sudoedit`: the door key, and
the allowlist. Confirm none remain:

```sh
sudoedit /etc/coder-worker/coder-worker.env
if sudo grep -q '<' /etc/coder-worker/coder-worker.env; then
  echo 'ERROR: unresolved placeholder remains' >&2
  false
fi
```

The public key is learned from the worker itself. On every start it prints
its identity as the first line of its log, as 64 hex characters:

```text
worker  3b332fd8…
relay   wss://relay.openagents.com
door    live (google/gemini-3.8-flash, lane gemini)
jobs    4 at once; more are refused busy
admits  1 customer(s)
waiting for jobs
```

That hex is the value a customer puts in `CODER_WORKER`; the terminal
accepts it as hex or as an `npub`. Read it from the journal after the
first start (step 3), and treat `admits  every customer` in that log as a
misconfiguration to fix before anything else.

### 3. Install and verify the unit

```sh
sudo install -o root -g root -m 0644 deploy/systemd/coder-worker.service \
  /etc/systemd/system/coder-worker.service
sudo systemd-analyze verify /etc/systemd/system/coder-worker.service
sudo systemctl daemon-reload
sudo systemctl enable --now coder-worker
sudo journalctl -u coder-worker -n 20 --no-pager
```

`systemd-analyze verify` prints nothing when the unit is well formed. The
journal's first lines are the block quoted in step 2; `worker  <hex>` is
the public key to hand out, and `subscribed; jobs arrive live from here`
means the relay accepted the worker's NIP-42 authentication and its
subscription.

The worker exits when the relay closes the socket, and `Restart=on-failure`
with `RestartSec=2` brings it back. Because NIP-CJ kinds are ephemeral, a
request published in that gap is lost and the terminal reports
`worker_absent` after 30 seconds; nothing is queued for the restart to
drain.

To watch the hardening score beside the relay's:

```sh
sudo systemd-analyze security --no-pager coder-worker.service
```

### 4. Approving a capability for the executor door

A gateway worker is done at step 3. The alternative door answers jobs
through an approved local executor instead of the gateway — read
[`docs/coder/guides/worker-executor.md`](../docs/coder/guides/worker-executor.md) for
what that door is and the layout its boundary accepts. On a host, three
things change.

First, the environment names the executor instead of a key. In the
installed environment file, comment out `CODER_DOOR_KEY` and uncomment the
executor block; setting both is refused at start. The block points the
worker at a checkout's `capabilities/` directory, a work directory, a
writable grant, and the trust store, all under `/var/lib/coder-worker`
except the checkout.

Second, the approval is recorded as the service user, into the store the
service will read, with the same three directories the environment names.
`capability-trust approve` pins the manifest's digest and the adapter's
canonical path and contents, so run it as `coder-worker` and against the
same paths the unit sees:

```sh
sudo install -d -o coder-worker -g coder-worker -m 0750 \
  /var/lib/coder-worker/exec /var/lib/coder-worker/jobs \
  /var/lib/coder-worker/.openagents
sudo -u coder-worker env HOME=/var/lib/coder-worker \
  CODER_CAPABILITY_TRUST=/var/lib/coder-worker/.openagents/capability-trust.json \
  /opt/coder-worker/current/capability-trust approve devin-local \
    --in /opt/openagents --writable /var/lib/coder-worker/jobs
sudo -u coder-worker env HOME=/var/lib/coder-worker \
  CODER_CAPABILITY_TRUST=/var/lib/coder-worker/.openagents/capability-trust.json \
  /opt/coder-worker/current/capability-trust list
```

Keep the trust store in its own directory. The delegation boundary seals
the directory that holds the store, and a writable grant inside a sealed
directory is refused with `boundary_unavailable: writable path
/var/lib/coder-worker/jobs overlaps protected path /var/lib/coder-worker`.
A store at `/var/lib/coder-worker/capability-trust.json` seals the whole
state directory; one under `.openagents/` seals only that subdirectory.

`/opt/openagents` stands for the checkout that holds the manifest; the
adapter it names must be installed where the manifest expects it and
reachable by the service user. The Devin CLI example in
`worker-executor.md` also copies the CLI's credentials into the writable
grant and trusts the work directory once; do that as `coder-worker` with
the same `XDG_DATA_HOME` the environment file sets.

Third, the unit gets the executor drop-in. The executor runs each job in a
`bwrap` mount namespace, and the base unit's `RestrictNamespaces=true`
forbids exactly that, so the drop-in lifts it and opens the checkout
read-only:

```sh
sudo install -d -m 0755 /etc/systemd/system/coder-worker.service.d
sudo install -o root -g root -m 0644 deploy/systemd/coder-worker-executor.conf \
  /etc/systemd/system/coder-worker.service.d/executor.conf
sudo systemd-analyze verify /etc/systemd/system/coder-worker.service
sudo systemctl daemon-reload
sudo systemctl restart coder-worker
sudo journalctl -u coder-worker -n 20 --no-pager
```

The journal's `door` line reads `executor (devin-local)`. A refusal at this
point is one of the typed ones `worker-executor.md` lists under
"Failures you will meet"; the terminal reports each as
`the worker declined (...)` rather than timing out.

### 5. Smoke test from another machine

The acceptance test is a turn that completes over the relay from a machine
that is not the worker's host. On that machine, with `coder` built and the
customer's key admitted in `CODER_WORKER_ALLOW`:

```sh
env -u CODER_DOOR_KEY -u CODER_AI_GATEWAY_KEY \
  CODER_WORKER=<worker hex or npub> \
  CODER_RELAY=wss://relay.openagents.com \
  coder -p --json "Reply with exactly the word pong"
```

The relay leg refuses an environment that also names a door key, which is
why the two are unset first. A completed turn exits `0` with `pong` in the
report; on the host, the journal shows the job's `answered in <n> ms`
line. The failures worth knowing by their `cause` field:

| `cause` | `refusal` | What it means here |
| --- | --- | --- |
| `worker_absent` | `null` | Nothing signed by the worker's key came back in 30 s. The service is down, or `CODER_WORKER` names a different key than the journal's `worker` line. |
| `worker_declined` | `not_admitted` | The service is up and the customer's key is not in `CODER_WORKER_ALLOW`. |
| `worker_declined` | `busy` | Every `CODER_WORKER_JOBS` slot is taken. |
| `relay_unreachable` | `null` | The relay, not the worker. Check `nostr-relay` and the proxy first. |

To record a measurement for `relay-transport.md`, run the same prompt
alternately with and without `CODER_WORKER` set, each with its own
`--trace` file, and report the medians the way that page does. The
worker's `answered in <n> ms` line is the upstream time to subtract from
the client's wall time; what remains is the relay round trip.

### Upgrading

Install the new release beside the old one, move the symlink, and restart:

```sh
sudo ln -sfn /opt/coder-worker/releases/<NEW_VERSION> /opt/coder-worker/current
sudo systemctl restart coder-worker
```

A restart drops the socket for about two seconds; requests published in
that window are lost, as described in step 3. The identity does not
change, so customers keep the same `CODER_WORKER`.
