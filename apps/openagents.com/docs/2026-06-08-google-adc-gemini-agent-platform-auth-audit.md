# Google ADC Gemini Agent Platform Auth Audit

Date: 2026-06-08

## Executive summary

OpenAgents product surface can sustainably call Gemini through Google Gemini Enterprise Agent
Platform only if Google authentication moves from the current API-key shape to
a workload identity shape.

The current OpenAgents product surface image-generation implementation in
`workers/api/src/image-generation.ts` calls
`generativelanguage.googleapis.com` with `x-goog-api-key` from the
`GEMINI_API_KEY` Worker secret. That was a reasonable first slice for
`/images`, but it is not compatible with the current organization policy shown
in the Agent Platform settings screen: API keys are disallowed, and
Application Default Credentials (ADC) are the recommended path.

The command shown by Google:

```sh
bash <(curl -sSL https://storage.googleapis.com/cloud-samples-data/adc/setup_adc.sh)
```

is useful for local operator setup and a workstation smoke test. It is not a
production credential for the Cloudflare Worker. It installs or locates
`gcloud`, asks for a Google Cloud project ID, runs
`gcloud auth application-default login`, sets the ADC quota project, enables
`aiplatform.googleapis.com`, and tests a bearer-token call to:

```text
https://aiplatform.googleapis.com/v1/projects/<project>/locations/global/publishers/google/models/gemini-2.5-flash:generateContent
```

The sustainable production answer is therefore:

- local development can use ADC from `setup_adc.sh` or
  `gcloud auth application-default login`;
- production inference must run from an identity-bearing environment, or from
  a broker that runs in one;
- OpenAgents product surface must stop treating a Google API key as the Gemini production authority;
- access tokens must be short-lived and automatically refreshed by ADC,
  metadata-server credentials, service-account impersonation, or Workload
  Identity Federation;
- raw Google tokens, refresh tokens, service account keys, provider payloads,
  and model prompts must stay out of D1, browser state, public sync, docs,
  issue comments, and logs.

The cleanest production architecture for the current OpenAgents product surface/Cloudflare stack is
a narrow Google inference gateway running on Cloud Run, GKE, or another Google
Cloud resource with an attached service account. OpenAgents product surface calls that gateway; the
gateway uses ADC to call Agent Platform Model APIs. If we need Cloudflare-only
direct calls later, use Workload Identity Federation only after we establish a
stable external identity token with a public issuer/JWKS and bounded claims.
Do not use a service account key as the normal escape hatch.

## Source facts

Google's Agent Platform ADC page says Gemini on Gemini Enterprise Agent
Platform can authenticate with a Google Cloud API key or ADC, recommends API
keys for testing, and recommends ADC for production. It also requires project
selection, billing, the Agent Platform API, and the Google Cloud CLI before
local ADC setup.

Google's general ADC documentation says ADC looks for credentials in this
order:

1. `GOOGLE_APPLICATION_CREDENTIALS`;
2. a local ADC file created by `gcloud auth application-default login`;
3. an attached service account from the metadata server.

The same ADC docs explicitly warn that service account keys are a security
risk and are not recommended. They also state that attached service account
credentials are the preferred production method on Google Cloud resources.

Google's Vertex AI / Agent Platform auth docs say REST calls can authenticate
with gcloud credentials or ADC, that local service account impersonation can
create an ADC file for supported client libraries, and that workloads on
Google Cloud should use the attached service account on the compute resource.

Google's REST auth docs show two production-relevant token sources when
`gcloud` is not available in the runtime: service-account impersonation and the
metadata server. The metadata server returns bearer tokens for supported
Google Cloud runtimes. The IAM Credentials `generateAccessToken` API defaults
short-lived service account access tokens to one hour when no lifetime is set.

Google's Workload Identity Federation docs say external or multicloud
workloads can access Google Cloud by exchanging federated identity credentials
instead of using service account keys. WIF supports OIDC/SAML-based identity
providers, follows OAuth 2.0 token exchange through Google's Security Token
Service, and can use direct resource access or service account impersonation.

Cloudflare Worker secrets are encrypted bindings exposed to Worker code as
`env`, but Cloudflare's docs also warn not to store sensitive values in plain
`vars`. Cloudflare Service Bindings can isolate one Worker from the public
Internet, but they only solve Worker-to-Worker calls. They do not give a
Cloudflare Worker a Google metadata server or Google ADC identity by
themselves.

Primary docs:

- Google Agent Platform ADC setup:
  https://docs.cloud.google.com/gemini-enterprise-agent-platform/models/start/gcp-auth
- Google ADC behavior:
  https://docs.cloud.google.com/docs/authentication/application-default-credentials
- Google ADC setup overview:
  https://docs.cloud.google.com/docs/authentication/provide-credentials-adc
- Google REST authentication:
  https://docs.cloud.google.com/docs/authentication/rest
- Google Agent Platform / Vertex AI authentication:
  https://docs.cloud.google.com/gemini-enterprise-agent-platform/machine-learning/authentication
- Google Workload Identity Federation:
  https://docs.cloud.google.com/iam/docs/workload-identity-federation
- Google IAM Credentials `generateAccessToken`:
  https://docs.cloud.google.com/iam/docs/reference/credentials/rest/v1/projects.serviceAccounts/generateAccessToken
- Gemini API OAuth quickstart:
  https://ai.google.dev/gemini-api/docs/oauth
- Cloudflare Worker secrets:
  https://developers.cloudflare.com/workers/configuration/secrets/
- Cloudflare Service Bindings:
  https://developers.cloudflare.com/workers/runtime-apis/bindings/service-bindings/
- Google API key and authorization key management:
  https://docs.cloud.google.com/docs/authentication/api-keys
- Google service account organization policy constraints:
  https://docs.cloud.google.com/organization-policy/restrict-service-accounts
- Google organization policy constraint reference:
  https://docs.cloud.google.com/organization-policy/reference/org-policy-constraints

## Current OpenAgents product surface state

Relevant current files:

- `workers/api/src/image-generation.ts`
- `workers/api/src/image-generation-routes.ts`
- `workers/api/src/image-generation.test.ts`
- `workers/api/src/image-generation-routes.test.ts`
- `docs/gemini.md`
- `docs/autopilot-tasks/done/2026-06-04-gemini-image-generation-implementation.md`

The current implementation:

- stores generated image bytes in the `ARTIFACTS` R2 bucket;
- exposes authenticated, operator-gated generation through
  `POST /api/images/generate`;
- serves generated images through authenticated stable URLs;
- normalizes provider errors into typed Effect errors;
- keeps the Google API key out of the browser;
- calls the Google AI Developer API endpoint:
  `https://generativelanguage.googleapis.com/v1beta/models/<model>:generateContent`
  or `:predict`;
- sends the credential as `x-goog-api-key`;
- uses the Worker secret name `GEMINI_API_KEY`.

The mismatch:

- the Agent Platform settings screen says API keys are disallowed;
- the current code path depends on an API key;
- a local ADC file created by `setup_adc.sh` is not available inside the
  deployed Cloudflare Worker;
- Cloudflare Workers do not have Google Cloud's attached-service-account
  metadata server;
- copying the local ADC JSON, user refresh token, or a service account key into
  Cloudflare secrets would create a long-lived secret-management problem rather
  than a durable ADC production path.

## Screenshot finding: why project admin still cannot edit

The policy shown in the operator screenshot is:

```text
constraints/iam.managed.disableServiceAccountApiKeyCreation
Display name: Block service account API key bindings
Policy source: Inherit parent's policy
Status: Enforced
Action: Deny
```

That is an Organization Policy Service constraint. It is not the same thing as
ordinary project IAM administration. A user can be a project owner, project IAM
admin, billing admin, or API keys admin and still be unable to change this
policy if they do not have Organization Policy permissions on the selected
resource.

The console tooltip in the screenshot is the direct reason the edit is blocked:
the account needs permissions such as:

- `orgpolicy.policy.get`;
- `orgpolicy.policies.create`;
- `orgpolicy.policies.delete`;
- `orgpolicy.policies.update`.

Google names `roles/orgpolicy.policyAdmin` as the predefined role containing
the relevant organization-policy management permissions. Because the policy
source says "Inherit parent's policy", the effective denial comes from a
parent organization or folder. To override it at the project, the operator must
have Organization Policy Administrator on the project or another scope allowed
to set an override. To change the inherited source itself, the operator needs
the role at the folder or organization that owns the inherited policy.

This also means "I am admin" can be true for the project while still not being
enough. Google Cloud separates project administration from organization policy
administration.

## What this policy actually blocks

This constraint blocks API keys bound to service accounts. Google's current
API key docs call these "authorization keys." They are API keys that
authenticate as a service account and behave similarly to a long-lived access
token.

Important distinction:

- a standard API key associates the request with a project for quota/billing
  but does not authenticate a principal;
- an authorization key is bound to a service account and does authenticate as
  that service account;
- Agent Platform / Vertex AI (`aiplatform.googleapis.com`) and Gemini API
  (`generativelanguage.googleapis.com`) support authorization keys;
- Google cautions not to use authorization keys in production for APIs that
  create or manage Google Cloud resources;
- the managed org policy is enforced by default for service-account-bound API
  keys.

The constraint reference says enforcement disables service-account-bound API
keys unless the key has non-empty API targets and all targets are in the
policy's `allowedServices` parameter. The simpler documented API-key setup
path says to set the constraint to `false` before creating an authorization
key. If the console exposes `allowedServices`, the narrower security posture is
to allow only the exact service needed, for example `aiplatform.googleapis.com`
or `generativelanguage.googleapis.com`, instead of disabling enforcement
globally. If the console does not expose that parameter cleanly, use the
documented narrow project override and require explicit API targets on the key.

## How to enable API key access if allowed

Use this only as a conscious exception. The production recommendation in this
audit remains ADC or workload identity.

Required roles:

- `roles/orgpolicy.policyAdmin` at the project, folder, or organization scope
  where the policy override or inherited policy will be changed;
- `roles/serviceusage.apiKeysAdmin` on the project to create/manage API keys;
- `roles/serviceusage.serviceUsageViewer` on the project if using the console
  to add API restrictions;
- permission to use or bind the target service account if creating an
  authorization key.

Console path for an org-policy admin:

1. Open Google Cloud Console.
2. Switch the resource picker to the target project, folder, or organization.
3. Go to IAM and Admin -> Organization Policies.
4. Search for `Block service account API key bindings`.
5. Click `Manage policy`.
6. Select `Override parent's policy`.
7. Add a rule with `Enforcement` set to `Off`, or configure the allowed service
   parameter if the console exposes a narrow `allowedServices` edit.
8. Use `Test changes` if available.
9. Click `Set policy`.

Equivalent documented gcloud shape:

```yaml
# spec.yaml
name: projects/openagentsgemini/policies/iam.managed.disableServiceAccountApiKeyCreation
spec:
  rules:
    - enforce: false
```

```sh
gcloud org-policies set-policy spec.yaml --update-mask=spec
```

Use `organizations/<org-id>` or `folders/<folder-id>` instead of
`projects/openagentsgemini` only if you intentionally want the change at that
broader scope. Prefer the narrowest scope that solves the problem.

After the policy allows the key, create a restricted key. For a standard
Gemini Developer API key:

```sh
gcloud services api-keys create \
  --display-name=openagents-gemini-standard \
  --api-target=service=generativelanguage.googleapis.com
```

For an Agent Platform / Vertex AI authorization key bound to a service account:

```sh
gcloud beta services api-keys create \
  --display-name=openagents-agent-platform-auth-key \
  --api-target=service=aiplatform.googleapis.com \
  --service-account=openagents-gemini-inference@openagentsgemini.iam.gserviceaccount.com
```

Then add application restrictions where possible. For a server-side key, prefer
server/IP restrictions only if the runtime has stable egress IPs. Cloudflare
Workers generally do not give a simple stable per-Worker egress IP, so API
restriction to the exact Google service is mandatory but not enough for strong
production security. That is another reason ADC remains the better production
path.

Do not create an unrestricted key. Do not commit the key. Do not paste it into
docs, D1 rows, issue comments, logs, or browser-visible responses. If a key is
created for a migration bridge, record an owner, expiry date, rotation plan,
and removal condition.

## 2026-06-08 CLI follow-up result

The operator authenticated locally with `gcloud` as `chris@openagents.com` and
the active project `openagentsgemini`.

Observed project:

```text
projectNumber: 157437760789
projectId: openagentsgemini
name: OpenAgentsGemini
parent organization: 831063912314
organization display name: openagents.com
```

The first `gcloud org-policies describe ... --effective` attempt failed
because `orgpolicy.googleapis.com` was disabled on the project. The CLI then
enabled it:

```sh
gcloud services enable orgpolicy.googleapis.com --project=openagentsgemini
```

Enabled API verification showed:

```text
aiplatform.googleapis.com    ENABLED
apikeys.googleapis.com       ENABLED
orgpolicy.googleapis.com     ENABLED
serviceusage.googleapis.com  ENABLED
```

The effective project policy was then readable and showed:

```yaml
name: projects/157437760789/policies/iam.managed.disableServiceAccountApiKeyCreation
spec:
  rules:
    - enforce: true
```

The account had `roles/owner` on the project, but a first attempt to create a
project-level override failed:

```text
Permission 'orgpolicy.policies.create' denied on resource
'//cloudresourcemanager.googleapis.com/projects/157437760789'
```

Trying to grant `roles/orgpolicy.policyAdmin` at the project also failed
because Google does not support that role on the project resource:

```text
Role roles/orgpolicy.policyAdmin is not supported for this resource.
```

The account did have organization-level authority:

```text
roles/advisorynotifications.viewer
roles/resourcemanager.organizationAdmin
```

The CLI added the needed organization-policy admin role at the organization:

```sh
gcloud organizations add-iam-policy-binding 831063912314 \
  --member='user:chris@openagents.com' \
  --role='roles/orgpolicy.policyAdmin' \
  --condition=None
```

After that, the project-level override succeeded:

```yaml
name: projects/157437760789/policies/iam.managed.disableServiceAccountApiKeyCreation
spec:
  rules:
    - enforce: false
  updateTime: '2026-06-08T14:22:56.238379Z'
```

Final verification:

```yaml
# Project effective policy
name: projects/157437760789/policies/iam.managed.disableServiceAccountApiKeyCreation
spec:
  rules:
    - enforce: false
```

```yaml
# Organization effective policy remains enforced
name: organizations/831063912314/policies/iam.managed.disableServiceAccountApiKeyCreation
spec:
  rules:
    - enforce: true
```

Interpretation: API key binding is now allowed for the
`openagentsgemini` project by project-level override, while the parent
organization remains secure-by-default for other projects.

Existing API key inventory was checked without printing key strings. The
project already has several keys, including restricted Generative Language API
keys and one key named `ONYXANDPRO` restricted to
`aiplatform.googleapis.com` and `generativelanguage.googleapis.com`. No new key
string was printed or committed during this follow-up.

## What "authenticate once" should mean

"Authenticate once" must not mean "mint one token and expect it to run
forever." Google bearer tokens are intentionally short-lived. The durable goal
is:

- one human or administrator establishes the workload identity and IAM grants;
- production code runs as that workload identity;
- access tokens are obtained and refreshed automatically;
- humans do not have to re-run `gcloud auth` for normal inference;
- revocation is possible by disabling the service account, WIF provider,
  binding, broker route, or interservice credential;
- the application never stores raw Google credential material in durable
  product state.

For local development, authenticating once means the developer's workstation
has a local ADC file and refresh path. For production, authenticating once
means the runtime has an attached service account or federated workload
identity. These are different credential locations and should not be blurred.

## Recommended production architecture

Use a Google-hosted inference gateway as the first durable production step.

```text
OpenAgents product surface Worker
  -> signed internal inference request
  -> Google Inference Gateway on Cloud Run/GKE/Compute
  -> ADC from attached service account
  -> Agent Platform Model API on aiplatform.googleapis.com
```

The gateway should be narrow:

- accepts only normalized generation requests from OpenAgents product surface;
- verifies an OpenAgents product surface service credential, HMAC, Cloudflare Access policy, mTLS
  client identity, or a later WIF-backed caller identity;
- rejects browser-originated calls directly;
- uses an attached service account such as
  `openagents-gemini-inference@<project>.iam.gserviceaccount.com`;
- grants only the Agent Platform permissions needed for model inference;
- never grants Owner, Editor, Viewer, broad admin, billing admin, or service
  account key admin roles;
- calls the Agent Platform endpoint with
  `Authorization: Bearer <short-lived-token>`;
- records safe receipts: project ref, location, model, latency, normalized
  status, request class, token counts when available, and redacted provider
  request id if available;
- omits raw prompts from public receipts unless an explicit safe projection is
  modeled and tested;
- returns normalized application output to OpenAgents product surface;
- leaves generated image bytes in OpenAgents product surface R2 or moves storage behind a separate
  explicit artifact policy.

Why this is the best first step:

- it matches Google's preferred production ADC path: code running on Google
  Cloud with an attached service account;
- it avoids storing service account keys in Cloudflare;
- it avoids relying on a human's local `gcloud` login;
- it gives Google Cloud IAM and audit logs a normal workload principal;
- it keeps OpenAgents product surface's existing Worker app, auth, R2, D1, and UI surfaces intact;
- it creates a small, testable boundary that can be replaced later by direct
  Workload Identity Federation if Cloudflare runtime identity becomes clean
  enough.

## Direct Cloudflare-to-Google alternatives

### Workload Identity Federation

This is the right long-term shape if OpenAgents product surface must call Agent Platform directly
from Cloudflare without a Google-hosted broker. It requires a trustworthy
external identity credential that Google can validate through WIF.

Minimum requirements:

- a stable issuer and JWKS or SAML metadata accepted by Google WIF;
- claims that bind the credential to the OpenAgents production Worker,
  environment, account, and deployment lane;
- a Google workload identity pool and provider;
- an attribute condition that denies unrelated Cloudflare accounts, preview
  deployments, local dev, and untrusted subjects;
- either direct Agent Platform IAM grants to the WIF principal or service
  account impersonation through `roles/iam.workloadIdentityUser`;
- short-lived Google access token acquisition through Security Token Service
  and, if needed, IAM Credentials `generateAccessToken`;
- token caching with expiry-aware refresh;
- tests that prove the Worker never stores or logs the external assertion,
  federated token, service account access token, or provider response body.

Open question:

- Cloudflare Workers do not automatically expose a Google-compatible workload
  identity token. Cloudflare Access, Zero Trust, or a first-party identity
  issuer may be usable, but that must be designed and proved. Treat this as a
  future direct-auth project, not the immediate fix.

### Service account key in a Cloudflare secret

This would be easy to implement and should remain the explicit non-default.

It would involve storing service account JSON in a Cloudflare secret, signing a
JWT assertion or using `google-auth-library`, minting access tokens, and
calling Agent Platform directly. It is not sustainable for normal production
because service account keys are long-lived high-value credentials, need
rotation, can be used outside the intended runtime if exfiltrated, and violate
the spirit of ADC/keyless operation.

Use only as a time-bounded emergency bridge with:

- a dedicated service account;
- a custom minimal IAM role;
- org-policy exception recorded;
- expiration date;
- key rotation and deletion receipt;
- no D1 persistence;
- no browser exposure;
- no public docs or issue comments containing the key or JSON.

### User OAuth refresh token in a Worker secret

This is also not a good production path. It makes inference depend on a human
account and user refresh token lifecycle. It is acceptable only for local
experiments or a deliberately human-owned operator tool. It should not back
always-on customer or agent-platform inference.

## Endpoint migration

Current OpenAgents product surface uses the Google AI Developer API style:

```text
POST https://generativelanguage.googleapis.com/v1beta/models/<model>:generateContent
Header: x-goog-api-key: <GEMINI_API_KEY>
```

The Agent Platform ADC smoke path uses the Google Cloud / Agent Platform style:

```text
POST https://aiplatform.googleapis.com/v1/projects/<project>/locations/<location>/publishers/google/models/<model>:generateContent
Header: Authorization: Bearer <access-token>
```

Migration requirements:

- add explicit config for `GOOGLE_CLOUD_PROJECT_ID`;
- add explicit config for `GOOGLE_CLOUD_LOCATION`, initially `global` unless a
  model or quota decision requires a regional endpoint;
- add explicit config for `GOOGLE_MODEL_API_BASE_URL`, defaulting to
  `https://aiplatform.googleapis.com`;
- select model IDs from Agent Platform docs or model discovery, not from the
  older `generativelanguage.googleapis.com` assumptions;
- verify Gemini image and Imagen endpoint support on Agent Platform before
  moving `/images`;
- preserve typed response parsing but treat provider response shapes as a new
  contract;
- rename runtime config away from `GEMINI_API_KEY` once the API-key path is
  removed, for example `GOOGLE_GENAI_AUTH_MODE=agent_platform_adc`;
- add tests asserting no `x-goog-api-key` header is sent in ADC mode;
- keep a disabled API-key compatibility path only if the repo records a
  deprecation date and it is gated off in production.

## IAM and project setup checklist

The Google project must have:

- billing enabled;
- Agent Platform / Vertex AI API enabled, currently `aiplatform.googleapis.com`;
- quota sufficient for selected Gemini models and locations;
- audit logging enabled for IAM, Service Account Credentials, Security Token
  Service when WIF or impersonation is used, and Agent Platform requests where
  supported;
- budget alerts for model spend;
- model-region allowlist documented.

Create a dedicated service account:

```text
openagents-gemini-inference@<project>.iam.gserviceaccount.com
```

Grant only inference-required permissions. Start with Google's predefined
Agent Platform / Vertex AI role only if a custom role is not yet practical,
then reduce to a custom role once the exact `generateContent` permissions are
confirmed. Do not grant Owner, Editor, Viewer, Project IAM Admin, Service
Account Admin, Service Account Key Admin, Billing Admin, or Secret Manager
Admin to the inference runtime.

Local developer setup can use:

```sh
gcloud init
gcloud auth application-default login
gcloud auth application-default set-quota-project <project-id>
```

When testing the production service account from a workstation, prefer
impersonation:

```sh
gcloud auth application-default login \
  --impersonate-service-account=openagents-gemini-inference@<project>.iam.gserviceaccount.com
```

That requires the local principal to have
`roles/iam.serviceAccountTokenCreator` or equivalent
`iam.serviceAccounts.getAccessToken` authority on the service account. Do not
grant this broadly.

## Token and caching policy

Bearer tokens are short-lived. The application should never assume a token
will be valid forever.

For a Google-hosted gateway:

- let Google auth libraries or metadata server ADC mint and refresh access
  tokens;
- cache tokens in process only until `expires_at - 300s`;
- refresh on startup and before expiry;
- on `401` or `403`, force refresh once, then classify as auth failure;
- never write access tokens, refresh tokens, ADC JSON, service account JSON, or
  external assertions to D1, R2, KV, public sync, issue comments, or docs.

For WIF:

- cache the Google access token only until expiry;
- avoid durable storage for external assertions and federated tokens;
- record only redacted token-source class and expiry bucket in logs;
- include replay protection if the external identity token can be replayed.

For local ADC:

- the file at `$HOME/.config/gcloud/application_default_credentials.json` is a
  local credential, not a deploy artifact;
- do not copy it into `.dev.vars`, Cloudflare secrets, source control, or
  docs;
- if local ADC breaks, re-run `gcloud auth application-default login` or the
  Google setup script locally.

## OpenAgents product surface application boundary

The production application boundary should become:

- browser sends prompt/request to OpenAgents product surface only;
- OpenAgents product surface authenticates OpenAgents session and operator/team access;
- OpenAgents product surface writes an internal request receipt without raw credential material;
- OpenAgents product surface calls the Google inference gateway or WIF-backed Google client;
- OpenAgents product surface stores output artifacts under existing `ARTIFACTS` policy;
- OpenAgents product surface returns stable application URLs and normalized metadata;
- OpenAgents product surface logs only normalized provider errors and safe usage metadata.

Do not add:

- Google bearer tokens in browser responses;
- Google credential refs in public sync collections;
- raw provider request/response payloads in D1;
- raw prompts in public receipts;
- provider secrets in route errors;
- keyword/string routing for model selection;
- direct model calls from browser code;
- compatibility shims that silently fall back to API keys in production.

If this changes runtime policy, update `INVARIANTS.md` with a Google inference
credential invariant and add regression tests. A likely invariant:

```text
Production Google model inference must not use API keys or service account
keys. It must use a short-lived bearer token from ADC, attached service
account metadata, service-account impersonation, or Workload Identity
Federation, and credential material must never enter browser, D1, public sync,
docs, issue comments, or logs.
```

## Observability, usage, and billing

The sustainable path needs billing and usage visibility from the first
production slice. Record safe fields only:

- provider: `google-agent-platform`;
- model id;
- location;
- route or workroom kind;
- authenticated user/team/workroom refs;
- generated artifact refs;
- latency;
- request status class;
- retry count;
- token counts from `usageMetadata` when returned;
- image count and byte count;
- normalized safety/block reason when returned;
- redacted provider request id if available.

Do not record:

- access tokens;
- refresh tokens;
- ADC JSON;
- service account key JSON;
- full provider response bodies;
- raw private prompts;
- uploaded private file bytes;
- generated private output in public logs;
- user emails unless already allowed by the target ledger.

Add budget controls:

- per-team daily spend cap;
- per-route rate cap;
- per-model allowlist;
- operator kill switch;
- emergency provider disable flag;
- fallback behavior that returns a clear unavailable state rather than trying
  a forbidden API-key path.

## Failure modes to test

Authentication:

- no ADC or gateway credential available;
- expired access token refresh succeeds;
- expired access token refresh fails;
- service account disabled;
- missing `aiplatform.googleapis.com`;
- missing project billing;
- missing model permission;
- org policy blocks API key and API-key path is not attempted.

Provider:

- `400` invalid request;
- `401` / `403` auth failure;
- `404` model or location not available;
- `429` quota/rate limited;
- `5xx` provider unavailable;
- response contains no candidates/images;
- response contains safety block;
- usage metadata missing.

OpenAgents product surface boundary:

- unauthenticated browser cannot call generation;
- non-operator cannot call generation if route remains operator-only;
- generated artifact read stays authenticated/private;
- logs redact credential-shaped fields;
- public projections reject provider-account refs, Google credential refs, and
  raw provider payloads;
- tests fail if production config uses `GEMINI_API_KEY`.

## Local verification commands

Inspect the setup script without executing it:

```sh
curl -fsSL https://storage.googleapis.com/cloud-samples-data/adc/setup_adc.sh | sed -n '1,220p'
```

Run the Google helper only as an interactive local setup step:

```sh
bash <(curl -sSL https://storage.googleapis.com/cloud-samples-data/adc/setup_adc.sh)
```

Verify local ADC can mint a token:

```sh
gcloud auth application-default print-access-token >/dev/null
```

Smoke Agent Platform REST from a local shell:

```sh
PROJECT_ID=<project-id>
LOCATION=global
MODEL_ID=gemini-2.5-flash
ACCESS_TOKEN="$(gcloud auth application-default print-access-token)"

curl -sS -X POST \
  -H "Authorization: Bearer ${ACCESS_TOKEN}" \
  -H "Content-Type: application/json" \
  "https://aiplatform.googleapis.com/v1/projects/${PROJECT_ID}/locations/${LOCATION}/publishers/google/models/${MODEL_ID}:generateContent" \
  -d '{"contents":[{"role":"user","parts":[{"text":"Reply only with SUCCESS."}]}]}'
```

This smoke proves local ADC and project access. It does not prove the
Cloudflare production Worker can authenticate.

Production verification for the recommended gateway should instead prove:

- the gateway runtime service account is the token source;
- the gateway can call Agent Platform without `gcloud`;
- OpenAgents product surface can call the gateway through the intended internal auth boundary;
- disabling the gateway service account stops inference;
- no `GEMINI_API_KEY` is required for the production route.

## Implementation roadmap

1. Add a new `GoogleModelInferenceService` contract in `workers/api` that is
   auth-mode agnostic and returns normalized text/image/usage output.
2. Add an Agent Platform request builder for
   `aiplatform.googleapis.com/v1/projects/<project>/locations/<location>/publishers/google/models/<model>:generateContent`.
3. Add a gateway-client implementation for OpenAgents product surface production.
4. Implement the Google-hosted inference gateway with attached service account
   ADC.
5. Protect the gateway with a narrow OpenAgents product surface service-auth boundary.
6. Add local ADC provider tests gated behind `RUN_GOOGLE_PROVIDER_TESTS=true`.
7. Add deploy checks proving production mode does not require or send
   `GEMINI_API_KEY`.
8. Migrate `/api/images/generate` from API key mode to gateway/ADC mode.
9. Remove or explicitly deprecate the `generativelanguage.googleapis.com`
   API-key code path.
10. Update `INVARIANTS.md` once production runtime policy changes.

## Decision

Use `setup_adc.sh` to authenticate the local operator once and prove the
Google project can call Agent Platform. Do not treat that local ADC file as
OpenAgents product surface production auth.

For always-on inference, put Google authentication behind a workload identity:
prefer a Google-hosted inference gateway with an attached service account now,
then evaluate direct Cloudflare-to-Google Workload Identity Federation when
OpenAgents product surface has a stable external identity token suitable for Google's WIF.
