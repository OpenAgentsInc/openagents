# 1.0 launch-day operations

How we watch the 1.0 launch, what we check, and how each platform goes back
if something breaks (#11105). Every service name below was read from the
repository docs and checked read-only against the project on 2026-10-09.

Every `gcloud` command here uses the automation account, so nothing asks for
a sign-in mid-launch:

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
```

Project `openagentsgemini`, region `us-central1`, zone `us-central1-a`.

## What runs where

| Service | Where | Owning doc |
| --- | --- | --- |
| Website (openagents.com) | Cloud Run service `coder`; container `web` plus the `coder-serve` sidecar | [openagents-web.md](../../deployment/openagents-web.md) |
| Web staging (site, gateway, chat worker) | Cloud Run service `openagents-web-1-staging`, three containers; smoke: `scripts/smoke/staging.sh` | [deploy/staging](../../../deploy/staging/README.md) |
| Chat worker | VM `oa-coder-worker-1`, unit `coder-worker-chat`, release symlink `/opt/coder-worker/chat` | [chat-worker.md](../../deployment/chat-worker.md) |
| Relay (the chat worker's job channel) | Cloud Run service `openagents-nostr-relay` | [runbook-cloud-run.md](../../deployment/runbook-cloud-run.md) |
| Gateway: account service and inference gateway | `openagents-gateway.service`, releases under `/opt/openagents-gateway/` with a `current` symlink. Production host not stood up yet (#11094) | [gateway deployment](../../decision-models/service/deployment.md), [inference gateway](../../inference/gateway.md) |
| api.openagents.com today | URL map `one-production-url-map`, path matcher `api`: default backend `one-production-api-backend`, pay routes to `oa-pay-front-backend` | [pay-host.md](../../deployment/pay-host.md) |
| Coder (terminal) | `gs://openagentsgemini-cli-releases/coder/`, pointers `coder.stable` and `coder.rc` | `scripts/release/coder.sh` |
| Desktop | `gs://openagentsgemini-oa-updates/desktop/{macos,linux,windows}/`, one signed `manifest.json` per OS | [desktop release](../../desktop/release.md) |
| iPhone | TestFlight, external group "OpenAgents Beta Testers" | `scripts/release/testflight.sh` |
| Android | Signed APK linked from `/download` | #11093, #11103 |

State on 2026-10-09 (read before launch, it will change): production web
traffic is 100% on `coder-web-w11-1d126aad2b-20261008202124`; the chat worker
runs release `42fe20c01b` with `bbed5d89af` kept; `coder.rc` is `1.0.0-rc.5`
and `coder.stable` doesn't exist yet; the macOS and Linux desktop manifests
name the unreleased September 30 `1.0.0` upload, and there is no Windows
manifest yet.

## Rollback, per platform

Write down the "previous" value for each row in the checklist's step 1,
before anything moves. Every rollback below needs it.

### Web

Move all traffic back to the revision that served before the deploy:

```sh
gcloud run services update-traffic coder \
  --region us-central1 --project openagentsgemini \
  --to-revisions <previous-revision>=100
```

Find `<previous-revision>` before the deploy with:

```sh
gcloud run services describe coder --region us-central1 \
  --project openagentsgemini --format='value(status.traffic)'
```

It takes effect in seconds. It also brings back the previous `/download`
page, so the download links go back with it.

### Chat worker

The previous release stays in `/opt/coder-worker/releases/`. Point the `chat`
symlink back and restart:

```sh
gcloud compute ssh oa-coder-worker-1 --zone us-central1-a \
  --project openagentsgemini --tunnel-through-iap --command '
  sudo ln -sfn /opt/coder-worker/releases/<previous> /opt/coder-worker/chat &&
  sudo systemctl restart coder-worker-chat &&
  sudo journalctl -u coder-worker-chat -n 20 --no-pager'
```

Never touch `coder-worker.service` or `/opt/coder-worker/current`; that is
the executor worker. If the environment file changed in the release, restore
its `.bak-<release>` copy in `/etc/coder-worker/` first.

### Gateway (account service and inference gateway)

Point `current` back at the previous release and restart:

```sh
sudo ln -sfn /opt/openagents-gateway/<previous> /opt/openagents-gateway/current
sudo systemctl restart openagents-gateway
```

The registry formats only append, so the previous release reads what the new
one wrote. The host is named in #11094 once it's up; add it here then.

### Terminal (Coder)

Point the stable channel back at the last good published version:

```sh
scripts/release/coder.sh --point-channel stable --version <previous>
```

New installs and anyone who reruns the installer get `<previous>`. The script
refuses a version that isn't published for all seven platforms, and a
release candidate on `stable`. Published versions are never replaced; a fix
ships as `1.0.1`.

1.0.0 is the first stable release, so rolling it back means removing the
stable pointer (the installers then fall back to `rc`) and pointing `rc`
back at `1.0.0-rc.5`, which `--point-channel` can't name because it was
published in the older separate-files layout:

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
B=gs://openagentsgemini-cli-releases/coder
gcloud storage rm $B/coder.stable
printf '1.0.0-rc.6\n' >/tmp/coder.rc && gcloud storage cp /tmp/coder.rc $B/coder.rc \
  --content-type=text/plain --cache-control='public, max-age=60'
```

Then set `CODER_VERSION` in `download.rs` back to `1.0.0-rc.6` (and
`coder_release_commands.txt` with it) if it was moved, and deploy the
website. Never point a channel at 1.0.0-rc.5: it has no `coder login`.

### Desktop

Before publishing 1.0.0, save each live manifest:

```sh
for os in macos linux windows; do
  gcloud storage cp gs://openagentsgemini-oa-updates/desktop/$os/manifest.json \
    gs://openagentsgemini-oa-updates/desktop/$os/manifest.pre-1.0.0.json
done
```

(Windows has no manifest yet; that line fails and there is nothing to save.)

To roll back, copy the saved file back over `manifest.json` for that OS.
Rolling back the web revision puts the previous download links back. The app
refuses an older version than it runs, so people who already installed the
bad build stay on it until a fixed `1.0.1` manifest goes out; the rollback
stops the bad build from spreading.

### iPhone

App Store Connect > Apps > OpenAgents > TestFlight > iOS builds > the 1.0
build > **Expire Build**. Testers can't install it after that; the previous
build stays available if it hasn't expired. This is an owner step.

### Android

Put the previous APK's link back on `/download`. Rolling the web revision
back does this. Keep every APK under its own versioned file name so the
previous one is still there.

## Log and alert checks

All read-only. Run them after each deploy and then every 30 minutes on
launch day.

### Website

```sh
# errors in the last 30 minutes
gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="coder" AND severity>=ERROR' \
  --project openagentsgemini --freshness=30m --limit=50 \
  --format='value(timestamp,resource.labels.revision_name,textPayload,jsonPayload.message)'

# 5xx answers
gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="coder" AND httpRequest.status>=500' \
  --project openagentsgemini --freshness=30m --limit=50 \
  --format='value(timestamp,httpRequest.status,httpRequest.requestUrl)'

# which revision has the traffic
gcloud run services describe coder --region us-central1 \
  --project openagentsgemini --format='value(status.traffic)'
```

Then open <https://openagents.com>, <https://openagents.com/download>, and
<https://openagents.com/docs> and send one chat message.

### Chat worker

```sh
gcloud compute ssh oa-coder-worker-1 --zone us-central1-a \
  --project openagentsgemini --tunnel-through-iap --command '
  sudo journalctl -u coder-worker-chat --since -30min --no-pager | tail -50;
  systemctl show coder-worker-chat -p ActiveState -p WatchdogTimestamp -p NRestarts'
```

A healthy worker logs only `gym records` every 10 minutes and `renewed the
jobs subscription` every 45 minutes between jobs. Repeated `relay: …
reconnecting` lines mean the relay is the problem; a climbing `NRestarts` or
an old `WatchdogTimestamp` means the worker can't hold its subscription.

From a checkout, one end-to-end answer:

```sh
openagents chat --scratch --json "Write a haiku about rain"
```

Exit code 0 means an answer came back.

### Relay

```sh
gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-nostr-relay" AND severity>=ERROR' \
  --project openagentsgemini --freshness=30m --limit=50 \
  --format='value(timestamp,resource.labels.revision_name,textPayload)'
```

### Gateway and api.openagents.com

```sh
# backends behind api.openagents.com
gcloud compute backend-services get-health one-production-api-backend --global --project openagentsgemini
gcloud compute backend-services get-health oa-pay-front-backend --global --project openagentsgemini

# 5xx at the load balancer for api.openagents.com
gcloud logging read 'resource.type="http_load_balancer" AND httpRequest.requestUrl:"api.openagents.com" AND httpRequest.status>=500' \
  --project openagentsgemini --freshness=30m --limit=50 \
  --format='value(timestamp,httpRequest.status,httpRequest.requestUrl)'
```

On the gateway host, once #11094 names it: `GET /healthz` answers when the
process is up, `sudo journalctl -u openagents-gateway --since -30min
--no-pager` shows its log, and the inference dashboard is at
`/admin/inference` with the admin token. The inference meter raises its own
alerts there when a prepaid balance runs low.

### Account service

The account service is the gateway (above). On the website, check sign-in by
hand: **Log in** > **Continue with GitHub** brings you back signed in, and
`coder login` reaches the [/device](https://openagents.com/device) page and
finishes.

### Downloads

```sh
B=https://storage.googleapis.com/openagentsgemini-cli-releases/coder
curl -fsS $B/coder.stable; curl -fsS $B/coder.rc
for os in macos linux windows; do
  curl -fsS -o /dev/null -w "$os %{http_code}\n" \
    https://storage.googleapis.com/openagentsgemini-oa-updates/desktop/$os/manifest.json
done
```

Every link on `/download` should answer 200 with the right version (#11103).

## Launch-day checklist

In this order. Stop at the first step that fails and roll that platform back.

1. **Write down every "previous".** Web revision with 100% traffic, chat
   worker release, gateway release, `coder.stable` and `coder.rc`, the three
   desktop manifests (save them as above), the current iPhone build number,
   the current APK file name.
2. **Owner go** for each release: web (#11094), terminal (#11091), desktop
   (#11092), iPhone and Android (#11093).
3. **Web**: deploy to the `new` tag with no traffic, check the tag URL, then
   move traffic. Run the website checks.
4. **Chat worker**, if it has a new release: install beside the old one,
   move the symlink, restart, run the scratch chat.
5. **Gateway** (account service): run the gateway and account checks.
6. **Terminal**: on the release Mac,
   `scripts/release/coder.sh --version 1.0.0 --publish --channel stable --publish-installers`
   (it moves `coder.stable` and `coder.rc` to 1.0.0), read `coder.stable`
   back, install on one Mac with the one-line installer, run `coder --version`,
   then set `CODER_VERSION` to `1.0.0` and deploy the website again
   ([terminal release](../../release/terminal.md#publishing-100)).
7. **Desktop**: publish per OS, read each manifest back, download each file
   from `/download`.
8. **Android**: put the APK on `/download`, install it on a phone.
9. **iPhone**: the build is in the external group; install it from
   TestFlight on a phone.
10. **Smoke test** every platform (#11102).
11. **Owner posts** the launch post (`post.md`) once everything in it is live.
12. **Watch** for the next 24 hours: the log checks every 30 minutes for the
    first 4 hours, then every 2 hours.

## Watch rota

For the owner to fill in.

| Window (UTC) | Who watches | Who can roll back | Reach them at |
| --- | --- | --- | --- |
| Launch to +4 h | | | |
| +4 h to +12 h | | | |
| +12 h to +24 h | | | |

Rollbacks that only the owner can do: iPhone (App Store Connect). Any web
step the automation account is refused on runs as `chris@` (the web
revision `replace` has been refused `actAs` before; see
[openagents-web.md](../../deployment/openagents-web.md)).
