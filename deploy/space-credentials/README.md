# SPACE credential adapter

This opt-in adapter belongs to [#11813](https://github.com/OpenAgentsInc/openagents/issues/11813).
Customer activation remains disabled until the live qualification passes.
The public default remains Boat. This adapter does not qualify Boat credential
injection, raw guest credentials, TLS exemptions, or native IPv6.

## Components and trust

- `computer-credentials` validates registered sources, immutable profiles, and
  current account, workspace, computer, command, and execution authority. Its
  Google Secret Manager reader pins numeric project and version IDs, verifies
  CRC32C, bounds response sizes, and refuses redirects and proxy environment
  variables. It does not cache resolved keys.
- `computer-credential-broker` owns source configuration and the Google identity.
  It reads the current public account and computer, workspace membership epoch,
  durable command state, and authenticated native execution proof on each use,
  before and after reading a secret. Caller sessions stay in bounded memory for
  the reference lifetime. Restart revokes old references for that deployment.
- The native gateway receives only its own node token, outside the guest, under
  a separate Unix identity. It forwards the request's opaque reference selector
  to the broker and replaces an approved authentication header. Each TLS
  connection pins the active grant set. A one-second watcher rechecks it with a
  five-second request deadline and closes the stream on refusal or outage.
- PostgreSQL stores reference metadata and revocation state. Deferred lifecycle
  triggers revoke on stop, wake, deletion, and uncertain computer state, and
  command triggers revoke on cancellation or completion in the same transaction.

The node token authenticates a worker; it does not authorize a tenant or grant
Secret Manager access. Guests receive neither that token nor a Google identity.
The authenticated native proof binds the resource, node, execution generation,
and installed full policy. A caller-supplied resource or cloud path is never
accepted as that proof.

A computer is one guest trust boundary. While its command grant is active,
processes in that computer can make matching requests. This does not isolate
mutually untrusted processes inside one guest or protect data sent to an allowed
destination. Register trusted destinations that do not echo authentication
headers in responses. The proxy controls credential use; it cannot control what
an authorized remote service returns.

## Native build

Check out public SPACE commit
`de87b436c35dac056564bdd5b2a113c9046e189b`, then run:

```sh
python3 deploy/space-credentials/apply.py /path/to/SPACE
```

The installer refuses another pin, a patch conflict, or an existing copied
crate. Build the native gateway, node manager, and API with SPACE's pinned
toolchain and production artifact sets. Keep the maintained deployment's
independent migration, kernel, TLS, and storage fixes. Save both source commits,
the patch SHA-256, and image digests with the qualification evidence. Never
substitute the upstream cached AWS credential path for this adapter.

## Private broker configuration

Build `cloudbuild.yaml` with a full committed `_COMMIT`. The image does not
activate the service or modify the maintained SPACE deployment.

Run the binary with one private configuration file:

```text
computer-credential-broker /run/credentials/config.json
```

The file contains these fields:

| Field | Meaning |
| --- | --- |
| `sources` | Private regular file containing an array of registered `Source` definitions |
| `signing_key` | Private regular file containing at least 32 random bytes |
| `native_token` | Private regular file containing the native tenant credential |
| `public_origin` | HTTPS origin serving the account, workspace, and public Computers API |
| `native_origin` | Private HTTPS native API origin with the credential-context extension |
| `root_ca` | Optional private regular PEM file for a private service CA; hostname verification remains enabled |
| `deployment` | Exact lifecycle registry deployment identity |
| `nodes` | Map from SHA-256 of each random node token to its native node UUID |

Files must be regular, mode `0600` or stricter, and not symlinks. Copy projected
Kubernetes secrets into private regular files in an init container, rather than
pointing these readers at projected symlinks. Mount no broker configuration or
Google credentials into a worker or guest. `OA_CREDENTIAL_DATABASE_URL` selects
the private lifecycle database; `PORT` defaults to `8791`.

Apply lifecycle migrations as the operator, then give the running broker read
access to computer/command metadata and read/write access to
`computer_credentials.bindings`. The running role does not need DDL privileges.
`--initialize` applies only the reference schema for an operator-managed setup;
it is not a substitute for normal lifecycle migrations.

Use a dedicated Google service account with Secret Accessor granted only on the
registered secrets. Pin every source to a numeric secret version. Configure
internal ingress and verified HTTPS, at most one broker instance, and bounded
concurrency; starting a second instance revokes the first instance's references.
Keep logs free of request bodies, headers, resolved values, and source files.

A `Source` contains an opaque UUID, positive revision, account or workspace
owner, numeric secret version, and named rules. Each rule fixes an exact DNS
host, port `443`, path prefix, HTTP methods, header, prefix, and plain or Basic
encoding. Supported headers are `authorization`, `x-api-key`, and
`x-goog-api-key`. Basic encoding also pins the username. Source edits invalidate
existing references. Restart after adding or rotating definitions to reload the
provider map; every caller must prepare new references afterward.

## Worker and profile configuration

Enable this only on an isolated canary worker until qualification passes.
Configure the node manager with `OA_SPACE_CREDENTIAL_BROKER` (HTTPS origin),
`OA_SPACE_CREDENTIAL_NODE_ID` (its native UUID), and
`OA_SPACE_CREDENTIAL_NODE_TOKEN_FILE` (root-owned private regular file).
The manager removes caller-supplied variables with this prefix and copies its
token into the isolated gateway directory, mode `0600`, owned by the gateway
UID. It refuses a gateway that shares the sandbox UID.

The qualified runtime must provide a real `/workspace` directory owned by its
unprivileged guest user. Command paths stay beneath that directory.

The lifecycle deployment must set a complete `network_profile` and
`credential_proxy: true`. The client requests the exact returned profile
selector and `needs.credential_proxy: true`. The saved binding freezes the full
definition and selector `NAME@sha256:FULL_DIGEST`; a changed definition cannot
broaden an existing computer. TLS exemptions are refused for proxy deployments.

Profiles default to deny and accept exact normalized DNS hosts. List every
model, Git, dependency registry, and artifact destination deliberately. The
adapter permits TCP `443` and the gateway's policy-filtered DNS resolvers
`1.1.1.1` and `8.8.8.8` on `53`. Other UDP, QUIC, private/reserved addresses,
metadata, and native IPv6 are denied. Domain admission still requires DNS
attribution and matching TLS/HTTP authority. Add a destination in a new profile
version, create a new computer, and qualify that combination.

Set `OA_COMPUTER_CREDENTIAL_BROKER_URL` on the public web service only after the
qualified private deployment exists. An unset URL refuses reference requests as
unsupported. No client receives private broker or provider credentials.

## Client preparation and recovery

1. Save a `CommandRequest` with its stable command key.
2. Prepare all references with `credential_request` and `prepare_credential`.
   Select registered source UUIDs and named rules. Save each request before
   sending it. A retry keeps its reference and original expiry.
3. Use `command_with_credential` to add each non-secret marker in an
   `OA_PARAM_*` variable. Save the final command request before executing it.
4. Give `oa-ref:REFERENCE_UUID` to the supported client as its API key or Git
   HTTPS password. The gateway requires that exact marker and checks the
   current command digest before replacing its header with the real key.
5. Read or revoke the reference with the same account and computer. Prepare a
   new reference after stop/wake, expiry, source rotation, or broker restart.
   An old marker cannot select a new turn's grant, even on a new connection.

Do not change a saved command after its first dispatch. Do not add grants midway
through a long stream: changing the active grant set closes its old connection.
Anonymous Git challenge requests may proceed without injection; a submitted
marker for the wrong destination or header is refused before provider access.

## Raw credentials and private snapshots

Legacy stdin delivery remains an explicit separate path for clients that need a
real guest credential. This proxy feature refuses raw credential names. A raw
exception requires an account-authorized source, minimum scope and lifetime,
per-turn stdin delivery, prompt revocation, and deletion of private snapshots
under the backend's documented retention policy. Do not publish its snapshot as
a shared base. Files, logs, application caches, and suspended RAM can contain
the raw value. Absence scans cannot establish RAM erasure.

Proxy references restored from old RAM are non-secret selectors. They cannot
restore revoked authority; current account, command, computer generation,
source, policy, and node checks still apply. Private snapshots remain sensitive
account data and use the maintained service's encryption, isolation, retention,
and deletion procedures. This adapter does not replace snapshot qualification.
