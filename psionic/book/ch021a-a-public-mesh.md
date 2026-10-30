# 21a. A public mesh

A Pylon can answer a signed encrypted request without opening a public model
port. [Chapter 20](ch020-the-api-fans-out.md) supplies the buyer and
[chapter 21](ch021-check-the-mesh.md) separates accepted work from independently
checked observations. This chapter puts that native buyer behind the site's
existing account boundary so users and agents can make a real decision call.

The [connect-a-pylon quickstart](https://openagents.com/docs/connect-a-pylon)
is the user-facing source of truth. The
[public SystemOne profile](../../docs/protocol/systemone-public-mesh.md)
defines the precise edge contract and limits. This chapter explains the trust
boundaries; changing one line of operator configuration is not cryptographic
proof of a stronger trust level.

## Join with a learned checkpoint

The installer supports the original Elephant checkpoint as well as the small
toy. Follow the quickstart's hardware requirements before the 18.8 GB download:

```shell
curl -fsSL https://openagents.com/pylon/join.sh -o /tmp/join-pylon.sh
bash /tmp/join-pylon.sh --model elephant-1 --self-test
```

The native device probe compiles the real Metal or CUDA kernels and checks the
Coax memory plan before fetching weights. The download manifest is emitted from
the same Coax target registry used by model admission, including immutable
revision, artifact hashes and checkpoint identity. Interrupted downloads resume;
an unsupported device or failed checksum never selects the toy automatically.
The explicit small alternative is `--model psionic-toy --self-test`.

The engine authenticates the original files and passes the numerical doctor
before the Pylon advertises capacity. Then `--self-test` starts a fresh native
buyer pinned to this new provider's key and exact checkpoint. It sends an
encrypted decision through the relay and checks the returned provider and model.
A local health response or a call answered by an operator's existing GPU is
insufficient proof of this join.

The self-test leaves the Pylon serving until it is stopped. It does not grant
the new provider admission to the hosted API's operator-approved routes. The
quickstart remains the source of truth for prerequisites, cache/state locations,
stop commands and troubleshooting. Report setup failures or unclear instructions
in [the repository's issue tracker](https://github.com/OpenAgentsInc/openagents/issues).

## Keep the layers separate

The public route is `POST /api/v1/systemone`. Its JSON is the existing NIP-DEC
request, including an explicit model, state and questions. It is separate from
`/api/v1/inference`, which is the metered Vertex Chat Completions service.
A request cannot choose an upstream URL, a checkpoint or a provider key.

The public edge admits an existing browser session and same-origin POST, or
an agent's existing NIP-98 request signature. The latter must bind the exact
body and is consumed once. A fresh valid Nostr key is enough for this bounded
free demo; it grants neither project membership nor paid credit. The existing
C/SQL API boundary still verifies those signatures and sessions. New admission,
quota and route/response binding are authored in Coax `#authority` policy.
The host contributes network, database, process and existing-auth effects.

After admission, the edge contacts one private Cloud Run fleet with its own
IAM service identity. The fleet discovers a configured provider over the relay,
verifies its signed beacon, encrypts the request and validates the result.
The Pylon talks only to its local learned-model backend. Incoming user cookies
or NIP-98 credentials never travel to the Pylon or model process.

## Pin actual models

`OA_MESH_ROUTES` maps a public model name to a private fleet URL, an exact
`name@sha256:<checkpoint>` identity and a provider public key. Each fleet must
pin the same identity and provider. Operators supply these rows, never a user
request, project title or relay label. A returned 200 with another model,
checkpoint or provider is refused. The fleet also checks the full answer
against the original typed questions before the public edge can return it.

Distinct checkpoints run behind distinct fleets; a pool containing Elephant
and Mammoth does not make those models interchangeable. Native failover keeps
an exact identity. The private services require IAM invoker authorization;
only the public API's runtime service account receives that permission.
`--listen-host 0.0.0.0` is an explicit socket effect for that private container,
while ordinary fleet commands continue to listen on 127.0.0.1.

## Bound free work

The Coax policy admits 12 requests per minute and 120 per UTC database day for
one identity, at most 1200 per day globally. It admits one active request per
identity and four globally. All API replicas serialize the count-read/admit/
lease transaction through a database advisory lock. That lock ends before any
network/model work. Completion releases the slot; a crashed worker's 60-second
lease also expires. The API bounds request bodies to 16 KiB and responses to 256 KiB, and waits
at most 30 seconds for the fleet. An upstream refusal never silently switches to Vertex.

These are demo bounds, not a payment authorization or defense against every
Sybil. Accepted attempts count even if a provider later fails. Operator-owned
fixed GPU capacity and the global cap bound this profile. A paid mesh requires
its own accounting and authorization work rather than relabeling these calls.

## Make a request

Write a NIP-DEC body to a file, then use the existing NIP-98 client helper:

```shell
coax/web/api/mesh-client.sh --new-demo-key /tmp/decision.json
coax/web/api/mesh-client.sh --staging --new-demo-key /tmp/decision.json
```

The helper creates a disposable key, signs the exact body with the API's
existing `sign.c` format, calls only the fixed production/staging origin and
removes the disposable key afterward. `--key FILE` uses an explicit existing
Nostr key instead. A browser uses its already signed-in site session. Neither
mode changes Pylon key custody, wallet balances or the production signer gate.

## Check the boundary

The executable policy is
[`mesh-policy.coax`](../../coax/web/api/mesh-policy.coax). It checks HTTP
bounds, authentication observations, quotas, deployment routes, identity
bindings and response substitution. Its root-only mutation run retains the
shared JSON checks and requires zero survivors. The explicit generated C/header
cache is compared to Coax compiler output and carries source/dependency/artifact
hashes; the existing API container needs neither Python nor a compiler.

[`mesh-test.sh`](../../coax/web/api/mesh-test.sh) creates fresh keys, a throwaway
PostgreSQL database and a fake fleet. It tests real signatures, body binding,
replay, CSRF, session expiration, caps, concurrent lease ownership, redirects,
oversized replies and model/provider substitution under ASan/UBSan. The native
socket effects have a separate sanitized bind/argv test. These checks qualify
the boundary, not GPU deployment. Record an actual authenticated cloud call and
its returned model/checkpoint/provider separately before calling a service live.

A valid signature proves control of a key. A selected checkpoint identifies
what the configured provider says it serves. Neither proves machine ownership,
hardware isolation or confidential inference. Confidential Pylons need verified
attestation and a distinct trust profile before the API may claim those things.
