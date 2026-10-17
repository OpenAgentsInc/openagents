# Labs API

The Labs API lives at `https://api.openagents.com/v1/labs`. Its
[OpenAPI document](openapi.labs.json) comes from the Rust handler types.
The [Labs roadmap](../labs/README.md) tracks kind and application integration.

Every request uses an account-authorized bearer credential. `space` is `me`
(the authenticated account) or `project:PROJECT_ID`. Send it in the query for
reads and deletes, and in JSON for writes. Project viewers read, editors
change, and owners delete. Account IDs supplied by a caller grant no access.
Experimental kinds are available only to the Computers test/admin audience.
The Echo fixture qualifies this API; CAD integration follows in #11875.

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
