# Published Coax compiler builds: the release hosts

How the published builds (`coax/RELEASE.md#published-builds`) are produced, and how to
set up the release hosts again. Set up 2026-11-02.

## What runs

| Platform | Release host | How |
| --- | --- | --- |
| `darwin-arm64` | The owner's Mac | launchd agent `com.openagents.coax-publish` runs `publish-host.sh --every 600` in its own clone, `~/.cache/coax-publish/openagents`. Log: `~/work/coax-publish.log` |
| `linux-x86_64` | GCE VM `coax-publish-linux` (us-central1-a, n2d-standard-16, Debian 12) | Off between builds. When the Mac sees a compiler change it starts the VM (`COAX_PUBLISH_LINUX_VM`). The VM's startup script, `coax/publish-vm-startup.sh`, publishes the static musl build if it is missing, then powers off. Log: `/var/log/coax-publish.log` on the VM |

Builds land at
`gs://openagentsgemini-cli-releases/coax/builds/<source>/<platform>.tar.gz`, public
read at `https://storage.googleapis.com/openagentsgemini-cli-releases/coax/builds/...`,
with `.sha256` beside each. A published name is never overwritten. Every machine's
`coax/build.sh` installs the matching build in seconds (a fresh worktree with an empty
cache: 17 to 18 s). Bootstrapping instead takes about 90 minutes on CoderOS's 28 cores
(84 of them in `stage0 mutate lib/compiler/checker.coax`), about 67 minutes on the Mac's
18 cores, and several hours on a 4-core container.

Cost: the VM bills only while it builds, roughly an hour per compiler change (about
$0.70 on n2d-standard-16), plus its 60 GB disk while stopped. CoderOS also published a
`linux-x86_64` build on 2026-11-02 by hand; it is not needed as a service.

## Check it

```sh
export PATH="$HOME/google-cloud-sdk/bin:$PATH" CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
gcloud storage ls -r gs://openagentsgemini-cli-releases/coax/builds/
launchctl print gui/$(id -u)/com.openagents.coax-publish | head
tail ~/work/coax-publish.log
gcloud compute ssh coax-publish-linux --zone us-central1-a --project openagentsgemini --command 'sudo tail /var/log/coax-publish.log'
```

A build for one platform with no build for the other means one host failed: read its log.
If `published.sh pack` refuses with "surviving mutants", main has a survivor in the
compiler. Don't publish around it; the next compiler commit that fixes it publishes.

## Set it up again

1. **The bucket** (once): `gcloud storage buckets create gs://openagentsgemini-cli-releases --location us-central1 --uniform-bucket-level-access`, then
   `gcloud storage buckets add-iam-policy-binding gs://openagentsgemini-cli-releases --member=allUsers --role=roles/storage.objectViewer`.
2. **The Linux VM:**

   ```sh
   gcloud compute instances create coax-publish-linux --project openagentsgemini --zone us-central1-a \
     --machine-type n2d-standard-16 --image-family debian-12 --image-project debian-cloud \
     --boot-disk-size 60GB --boot-disk-type pd-balanced \
     --service-account oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com --scopes cloud-platform \
     --metadata-from-file startup-script=coax/publish-vm-startup.sh --labels purpose=coax-publish
   ```

   Its first boot installs Nix, then builds. Touch `/etc/coax-publish-stay-on` on the VM to
   keep it up for debugging. After editing the startup script, push it again with
   `gcloud compute instances add-metadata coax-publish-linux --zone us-central1-a --metadata-from-file startup-script=coax/publish-vm-startup.sh`.
3. **The Mac's launchd agent:** `~/Library/LaunchAgents/com.openagents.coax-publish.plist`
   runs, under `/bin/bash -c`:

   ```sh
   export PATH="$HOME/google-cloud-sdk/bin:/opt/homebrew/bin:/usr/bin:/bin:/usr/sbin:/sbin"
   export CLOUDSDK_CONFIG="$HOME/work/.secrets/gcloud-sa-config"
   export COAX_PUBLISH_LINUX_VM=us-central1-a/coax-publish-linux
   exec "$HOME/.cache/coax-publish/openagents/coax/publish-host.sh" --every 600
   ```

   with `RunAtLoad`, `KeepAlive`, `Nice 10`, `LowPriorityIO`, and stdout and stderr to
   `~/work/coax-publish.log`. Load it with
   `launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.openagents.coax-publish.plist`.
   `publish-host.sh` updates its clone to `origin/main` each round, including its own
   scripts.

## Don't

- Don't publish from a dirty tree, or remove `COAX_NO_PUBLISHED`/`COAX_BUILD_FRESH` from
  `publish-host.sh` (a publisher must bootstrap, never re-publish a download).
- Don't delete or overwrite anything under `coax/builds/` in the bucket.
- Don't use GitHub Actions (INVARIANTS.md).
- Don't print `CLOUDSDK_CONFIG`'s key or tokens.
