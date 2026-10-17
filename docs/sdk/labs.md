# Labs API

The Labs API lives at `https://api.openagents.com/v1/labs`. Its
[OpenAPI document](openapi.labs.json) comes from the Rust handler types.
The [Labs roadmap](../labs/README.md) tracks kind and application integration.

Document requests use an account-authorized bearer credential. `space` is `me`
(the authenticated account) or `project:PROJECT_ID`. Send it in the query for
reads and deletes, and in JSON for writes. Project viewers read, editors
change, and owners delete. Account IDs supplied by a caller grant no access.
Experimental kinds are available only to the Computers test/admin audience.
The Echo fixture qualifies this API. CAD uses its existing Pro audience;
saved designs remain readable and deletable after Pro ends.

CAD accepts `part.v1` and `assembly.v1`. The document ID must match the
content's `id`. `cad_from_json` saves a checked document, `cad_drag_face`
accepts the viewer's named face and distance, and `cad_export` requests an
export job. Changes and jobs use the existing server child process, queue,
75-second deadline, and 640 MiB memory limit. Cards link to the existing CAD
page. Imported sources and derived files are marked `no_training`.

Create a document with `id`, `kind`, `schema`, `title`, `content`, and `summary`.
An existing ID returns `409`; deleted IDs cannot be reused. Changes name an
immutable `base`, `change`, and `summary`. Successful saves return `201`, stale
bases return `409`, and kind refusals return `422`. Mutating tools also require
`base`; retrying a completed change cannot apply it twice. Reads never start
jobs. Kinds identify their document references; each referenced space and kind
is authorized before saving or returning version content.

Job creation requires `Idempotency-Key`. Reuse the key and exact request after
response loss. Changed input under the same key returns `409`; identical work
on the same version shares its job. Poll the job or cancel it explicitly.
A lost started worker leaves an unknown result rather than rerunning the work.

Pages contain `data` and `next`, with at most 100 entries. Events use `since`
and ordered cursors. `Accept: text/event-stream` streams batches and accepts
`Last-Event-ID`; the stream rechecks access and ends after five minutes.

Direct source uploads accept base64 JSON files up to 512 KiB. Artifacts inherit
`no_training` from their version's marked inputs. Metadata returns a 60-second
signed Cloud Storage URL when IAM signing is configured. When IAM signing is unset, metadata returns
an authenticated content URL with `expires_seconds: null`. Both support byte
ranges. Never put a private-local source file in an upload.

JSON bodies are capped at 1 MiB. Kind manifests supply smaller document,
argument, version, and hourly-job limits. Cards read up to 1,000 matching jobs;
larger histories use the paged job list. Errors use `{error: {code, message}}`;
`429` and `503` include `Retry-After`. Coder 1.0 routes remain unchanged.

Deployment uses `OPENAGENTS_WEB_LABS_DATABASE_URL` as a PostgreSQL URL.
Run `python3 deploy/labs/database.py staging` (or `production`) to create
a URL-form secret from the existing account database credential. The original
secret stays in place. Each Labs runtime receives access to its own new secret.

Production signs file links with the keyless `openagents-labs-files` service
account. Run `deploy/labs/production-signing.sh` after the signing role exists;
it grants the runtime signing access and limits the signer to the bucket's
`labs/` files. `WEB_DEPLOY_TAG=labs-11874` gives a concurrent deployment its
own candidate URL when running `scripts/deploy/web.sh promote`.

Review existing CAD history with `scripts/dev/labs-cad-review.py --bucket
BUCKET --evidence FILE`. Its output contains counts and digests only. After
the owner approves a scope's review checksum, run the image's `lab-cad-copy
--bucket BUCKET --account ACCOUNT --approved-checksum SHA256` (add
`--project PROJECT` for a project) with the production runtime identity,
Cloud SQL attachment, and Labs database and bucket configuration. The command
checks the reviewed checksum, preserves the original objects, and imports
the same version numbers and content digests. Retries verify completed
versions before continuing. [The production review](labs-cad-migration-review.json)
records the history awaiting approval.

## Rust client

The [`openagents-sdk` Labs module](../../crates/openagents-sdk/README.md#labs)
uses an authenticated session: `let labs = client.labs(&session)?`. It re-exports
`lab-kit` resource and request types. Bind writes with `labs.bind(body)`;
document and job references carry the account, deployment, and space.

Use `edit` to distinguish a saved version from a stale base or a kind refusal.
Prepare a job with `job_request`, save its serializable record before sending,
and reuse that record after response loss. `wait` returns the observed job
state; cancellation stops the local wait. `cancel_job` is a separate remote
request. Event readers resume from the last delivered cursor, and artifact
readers support full downloads and exclusive Rust byte ranges such as `0..4`.

Clones share account changes. Call `set_session` or `clear_session` before
showing another account; old reads and streams then stop. Keep application
caches separate by account and clear private views on sign-out. The optional
`labs-cad` feature calls the shared portable checks for previews. It does not
replace a server rebuild or grant permission to save a design.

## Outbound workers

Use the account-bound Labs client's `enroll_worker` to issue a job-only key for
one space, with a lifetime of one minute to one day. The issuing sign-in and
current project edit access are checked on every worker request. Use
`client.labs_worker(&enrolled.token)` for poll, start, heartbeat, finish, and
self-retirement. This client cannot sign in as an account or read documents.

Worker calls send once. Save the assignment and execution marker before
starting a handler, then save its finish request before sending it. An identical
finish retry after a lost reply does not authorize another execution. A finish
may report `done`, `failed`, or `uncertain`; only `done` accepts outputs.
Keep the SDK credential record in caller-owned private storage, bound to its
deployment. `clear_credential` stops local clones; `retire` revokes the remote
worker, and the caller removes its saved key separately.

The [`labs-worker` guide](../../crates/lab-jobs/README.md) describes installation,
private input checks, process limits, and recovery. Worker messages have a
12 MiB wire limit, with at most 16 outputs and 8 MiB of decoded output bytes.
Job usage records report observed time; they do not invent a charge.

## Saved chat cards

History messages include up to 16 typed Lab cards in `cards`. Reading a card
checks access to its saved document and version again; sharing a chat does not
grant access to its Lab files. An unavailable document becomes a Report card.
Artifact reads use the reader's own credential.

The history SDK exposes each card with an account-bound `document_ref()`.
Unknown card views fall back to Report. The browser composes cards through `lab-ui`'s shared Rust Native view. Desktop, phone, and terminal adapters use the same view as they are added;
media surfaces resolve its opaque resources through the Labs client.

[History-card source checks](lab-history-evidence.json) record the local qualification.
