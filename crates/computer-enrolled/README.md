# Enrolled workspace computers

`computer-enrolled` implements Core computer operations through the existing
signed NIP-HOST direct and relay bindings. Create attaches a logical computer
to one shared workspace. Delete detaches that computer. Neither operation
deletes files, stops the host, changes power state, or revokes another grant.

The initial host implementation supports Unix hosts with an available Coder
filesystem boundary. Commands run offline, without inherited environment or
credentials, with a five-minute maximum duration, a 512 MiB memory limit,
and bounded output. The host admits at most four unfinished commands. File
transfers use relative paths and a 4 MiB limit; the host rejects links,
special files, parent traversal, and roots that expose its access store.

Only Core is advertised. Suspend, resume, fork, snapshot, cloud images,
resource sizes, chat turns, and credential injection are unavailable. The
`workspace` size label identifies an existing folder; it promises no CPU or
memory allocation. This connection does not measure hardware, electricity,
network charges, or compute usage. An offline host returns an unavailable or
unknown result; work never moves to a cloud backend automatically.

## Configure an authorized workspace

The host operator selects an existing host workspace and its account. Its label
must match the resident host's configured workspace map. On that host, issue a
single-use invitation with the new binary:

```sh
coder-access --state /path/to/host/access invite-scoped \
  --relay wss://relay.example \
  --account ACCOUNT_ID --workspace project --root /path/to/project
```

The default rights are `observe,operate`. Use `--rights observe` for read-only
access. Existing NIP-HOST clients redeem the invitation and receive the encrypted,
host-signed `Access` record. Keep the device secret outside configuration and
source control. An older host cannot issue this required scope feature.

The computer service operator registers the verified record separately from
cloud targets in `OA_COMPUTERS_REGISTRY_FILE`:

```json
{
  "default": "existing-cloud-target",
  "policy": "registry-v1",
  "deployments": [
    {
      "id": "my-project",
      "kind": "enrolled",
      "enrolled": {
        "account": "ACCOUNT_ID",
        "host": "HOST_PUBLIC_KEY_HEX",
        "workspace": "project",
        "access_file": "/private/path/access.json",
        "device_key_env": "OA_PROJECT_DEVICE_KEY"
      }
    }
  ]
}
```

Include the existing cloud deployment in that array. Enrolled targets cannot
be the default and never participate in automatic placement. The service
verifies the record against the device secret, then checks the account, host,
and workspace. The public request selects the registered target name:

```json
{"placement":"enrolled","enrolled_host":"my-project","idempotency_key":"attach-1"}
```

Use identified commands with relative working directories and explicit timeouts
of at most 300 seconds. Command IDs and complete request hashes survive host
and service restarts. Lost replies can be observed or retried with the same
identity. An uncertain launch from an earlier host process reports `Lost` and
never starts again. Cancellation confirms process exit before reporting success.
Logical expiry detaches access; each running command keeps its own deadline.
Revoking the scoped grant also stops its commands. A renewed grant keeps the
same account, root identities, rights, and epoch.

The service's stop, wake, and chat-turn endpoints refuse workspace attachments.
Independent command completion does not create a chat checkpoint. After detach,
use a new create identity to attach again. Retain the original host state:
removing its command book removes the evidence that prevents a command replay.
