# Public SystemOne mesh profile

`POST https://openagents.com/api/v1/systemone` is the authenticated public
edge for native Coax Psionic fleets. Its body and successful response are the
existing `openagents.systemone.v1` / NIP-DEC JSON contract. The source-pinned
protobuf projection is `proto/openagents/decision/v1alpha1/systemone.proto`.
This profile changes neither that wire format nor `/api/v1/inference`, the
separate metered Chat Completions/Vertex service. Public protobuf negotiation
is not part of this edge yet.

A request names a model such as `elephant-1`; it supplies `state` and typed
`questions`. It cannot choose a URL, checkpoint or provider. The API forwards
the exact admitted body to a deployment-owned native fleet. The fleet validates
the complete NIP-DEC request, verifies signed discovery, authenticates/encrypts
Pylon traffic, enforces the configured exact model identity/provider and checks
the returned decision. The edge additionally refuses a successful response whose
model, `service.identity` or `service.provider` differs from its configured pins.
No Vertex fallback, model substitution, remote shell or caller-selected proxy
exists on this route.

## Existing identities, two entry points

An agent sends `Authorization: Nostr <base64-event-json>` using the existing
NIP-98 HTTP-auth event (kind 27235), bound to the exact absolute URL, method and
SHA-256 of the exact body. The existing API verifier checks the event ID and
BIP-340 signature. The payload tag is mandatory for this route. Events expire
under the existing 60-second freshness rule and their IDs are consumed once in
`projects.seen`. Retrying requires a fresh event, not a replay. This is identity
authentication, not a paid balance, membership grant or inference attestation.

A signed-in browser can instead use its existing HttpOnly `oa_session` cookie.
Its POST must have `Origin` equal to the deployment's `OA_API_ORIGIN`; absent or
foreign origins refuse. Explicit invalid Authorization never falls back to a
cookie. An explicitly foreign Origin refuses even with a valid signature.
Neither incoming cookie nor incoming Nostr authorization is forwarded upstream.
Responses are private/no-store and vary by Cookie and Authorization.

The compatibility API's current NIP-98 signature verification and session lookup
remain in its existing C/SQL authentication boundary. New HTTP admission,
identity-observation policy, quota decisions, route/identity binding and response
admission are authored in `coax/web/api/mesh-policy.coax`, marked `#authority`.
The existing production signer path is not changed or enabled by this work.

## Free demo bounds

Every identity has the same limits: 12 admitted requests per minute, 120 per
UTC database day and one active request. The service admits at most 1200 per
UTC database day and four active requests globally. An atomic database advisory
lock covers observation, Coax admission and lease recording across API replicas;
it is released before any model execution. Expired 60-second leases no longer
occupy a concurrency slot. Completed calls release their lease and retain status.
Model execution has a 30-second HTTP deadline and a three-second connection
limit. Request bodies are bounded to 16 KiB, matching the native fleet's
16384-byte admission cap and staying below its NIP-44 compatibility envelope;
responses are bounded to 256 KiB. These bounds charge no
sats and create no inference credit. Quotas count admitted attempts, including
failed upstream attempts. There is no administrator exemption.

The native fleet currently treats authenticated remote provider error statuses
as failed attempts: it retries eligible routes or returns 503. It does not yet
propagate a remote context-limit status as 422. An actual HTTP 422 received from
an upstream remains 422 (`unsupported_input`) at this edge; 413 is local byte
admission, not a claim about model context.

HTTP refusals include 401 authentication/replay, 403 cross-site, 400 invalid model
or request, 405 method, 413 body limit, 415 content type, 429 demo quota, 502 invalid
or substituted upstream response, 503 unavailable configuration/provider and504
upstream deadline. A refusal never claims completed inference. The response
identity is provenance of the selected configured provider; it is not proof of
confidential execution, hardware identity or model correctness.

## Operator configuration

`OA_MESH_ROUTES` is a deployment-owned JSON object:

```json
{
  "elephant-1": {
    "url": "https://PRIVATE-FLEET.run.app/v1/systemone",
    "identity": "elephant-1@sha256:CHECKPOINT_SHA256",
    "provider": "PYLON_PUBLIC_KEY_HEX"
  }
}
```

Each native fleet process serves exactly one checkpoint identity and must use
`--provider PYLON_PUBLIC_KEY_HEX` matching this row. The provider is a verified
fresh Pylon beacon, not an arbitrary label. The relay supplies model traffic;
backend inference ports remain private. Run separate fleets for distinct model
identities. A pool of models does not imply interchangeable checkpoints or a
shared model name.

Production fleets are private Cloud Run services. The API obtains an IAM ID
token from its own runtime metadata service with the fleet URL's origin as the
audience. Grant only the API's runtime service account `roles/run.invoker` for
these services. Do not grant unauthenticated invoker. HTTP upstreams are for
loopback tests or an explicitly private network; do not expose backend ports.
The HTTP client allows only HTTP(S), follows no redirects and uses no ambient
proxy. Route JSON is never accepted from a request or a project document.

The API deployer loads `oa-mesh-routes-staging` and
`oa-mesh-routes-production` Secret Manager entries when present, so ordinary
redeployments preserve operator-owned routing. The runtime account also needs
permission to read the relevant config secret. Missing config is a 503 refusal.
Those entries contain routing metadata, not user or provider secret keys.

A fleet defaults to `127.0.0.1`. Its effects-only `--listen-host 0.0.0.0` option
is for the private authenticated service container; it is stripped before the
unchanged Coax CLI receives arguments. Only those two literal addresses are
supported. This flag does not add authentication to a public VM port.

## Source and checks

- `coax/web/api/mesh-policy.coax`: authored policy and executable checks.
- `coax/web/api/mesh-policy.generated.{c,h}`: explicit reproducible C cache for
  the existing C-only API container; regenerate with `mesh-policy.sh --generate`.
  `--check` compares compiler output; `--check-inputs` verifies pinned source,
  dependency and artifact hashes without requiring a compiler at deployment.
- `coax/web/api/mesh.c`: bounded network/DB/process effects and the route.
- `coax/web/api/migrations/0068_mesh_systemone.sql`: observations/leases only.
- `coax/web/api/mesh-test.sh`: fresh database, keys and fake upstream, native
  ASan/UBSan, real NIP-98/session/replay/CSRF/quota/concurrency/substitution tests.
- `coax mutate coax/web/api/mesh-policy.coax --root-only`: zero-survivor policy
  qualification, retaining imported JSON checks. This does not rerun unrelated
  gateway billing, models or old API suites.
- `psionic/platform/fleet-host-test.c`: default/private-container bind and
  malformed/duplicate host-option effects checks under ASan/UBSan.

A passing fake-upstream test is not a cloud inference deployment claim. Record
real authenticated staging/production calls separately with the exact service,
provider and checkpoint identity. The public connect-a-pylon quickstart is the
user-facing source of truth; this document defines the edge's technical profile.
