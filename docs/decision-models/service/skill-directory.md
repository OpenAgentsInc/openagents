# The skill directory

`gateway`'s skill surface publishes a versioned directory of reusable
`SKILL.md` documents. An author submits a bounded Markdown document
with its authorship, license and rights, category and tags, semver,
content digest, and publication consent; the service runs a recorded
review pipeline and publishes the versions that pass. Callers browse,
search, and fetch published versions with stable URLs and explicit
installation instructions. The surface mounts only when `gateway.json`
names a `skills` document, which `Config::check` binds to `accounts`:
a submission binds the account that sent it.

## Submissions

`POST /v1/skills` accepts the submission as a JSON document:

```json
{
  "name": "ticket-triage",
  "version": "1.0.0",
  "license": "MIT",
  "category": "support",
  "tags": ["triage", "routing"],
  "consent": true,
  "markdown": "---\nname: ticket-triage\ndescription: ...\n---\n\n# ...",
  "evidence": {"suite": "...", "report": "...", "digest": "..."}
}
```

The author authenticates as an account — an `oak_` key bound to one or
a `sess_` session token. An anonymous session answers
`membership_required`; a missing credential answers `unauthenticated`.
`consent` is required and means the author grants the deployment
permission to publish the document under its declared license.

Bounds the book enforces:

| Bound | Value |
| --- | --- |
| `name` | `[a-z0-9][a-z0-9-]*`, at most 63 bytes |
| `version` | `MAJOR.MINOR.PATCH` with an optional pre-release, at most 32 bytes |
| `license`, `category` | Non-empty, bounded; category is lowercase alphanumerics and dashes |
| `tags` | At most 8, each lowercase alphanumerics and dashes |
| `markdown` | At most `skills.max_body_bytes` (default 64 KiB) |
| `evidence` | Optional — a pinned suite, report, and digest |
| Per author | `skills.submissions_per_day` (default 20), `skills.pending_per_author` (default 10) |

A name belongs to its first submitter: only that account may submit
further versions of it. Resubmitting the identical `(name, version,
digest)` returns the existing submission — `retry` semantics while the
version is under review, `duplicate` once it resolved. The same
`(name, version)` at a different digest answers `version_conflict`:
an author supersedes by bumping the version, never by rewriting it.

## The review pipeline

Every submission runs three recorded stages, in order. Each stage
records its reviewer, policy version, outcome, score, rationale, cost,
and failure detail on the version's review list — the same list
`GET /v1/skills/{name}/versions/{version}/review` publishes.

1. **`static`** — mechanical checks under
   `openagents.skill-static.v1`: YAML frontmatter with a `name`
   matching the submission and a non-empty `description`, plus a
   credential screen that refuses live-shaped literals — `oak_`
   tokens, `sess_` tokens, and PEM private keys. A failure rejects
   without spending a model call.
2. **`decision`** — one call to the configured `skills.review`
   backend under `openagents.skill-review.v1`. The document travels
   as `state` under judgment, never as instructions. Three typed
   questions answer: `safe` (the document avoids credential theft,
   exfiltration, destruction, and policy evasion), `coherent` (it
   reads as a self-contained skill), and `quality` (a five-level
   rubric, normalized to `0–1`). The stage records the model's id,
   the three answer values, the normalized score, and the token cost.
3. **`reasoning`** — the admission synthesis, under the same policy:
   `safe ≥ 0.8`, `coherent ≥ 0.5`, and `quality ≥
   skills.admit_score` (default `0.6`). All three pass and the
   version publishes; any failure rejects with the gates named in the
   resolution.

A backend failure records an `error` outcome, not a rejection — the
submission stays `under_review` and an identical resubmission retries
the outstanding stages from the record. A model review is a recorded
judgment, not a security guarantee: the static screen catches
credential-shaped literals, and the host's execution policy still
governs anything a published skill tells an agent to do.

Submitted Markdown is inert on every path: the adapter stores it by
digest and serves it as `text/markdown`; nothing interprets it.

## The published catalog

The public reads need no credential and resolve published versions
only.

| Route | Answer |
| --- | --- |
| `GET /v1/skills` | Published entries — `q` substring search, `category`, `tag`, and `author` filters, `sort=name\|recent`, `limit` bounded at 100, keyset `cursor` |
| `GET /v1/skills/{name}` | The entry: latest published version, every published version, and the install block |
| `GET /v1/skills/{name}/versions/{version}` | One pinned version's record |
| `GET /v1/skills/{name}/versions/{version}/SKILL.md` | The version's raw Markdown, digest-verified on every read |
| `GET /v1/skills/{name}/versions/{version}/review` | The version's recorded review stages |
| `POST /v1/skills/{name}/versions/{version}/withdraw` | The author pulls the version |

A newer published version marks older ones `superseded_by` without
hiding them — a pinned version URL keeps resolving. Each version
document separates `review` (the model's assessed quality and its
policy) from `evidence` (a pinned suite and report, or
`"measured": false` when none exists) — assessed quality and measured
performance are never folded together.

The install block is data: the stable Markdown URL, the version URL,
and written instructions a reader follows by choice. Browsing the
catalog never installs or executes anything.

## Author and operator views

`GET /v1/submissions` is the caller's own submission list in every
state — including rejected versions the public catalog never shows,
each with its resolution and review record. `POST
/v1/submissions/{id}/appeal` files the author's appeal against a
rejection.

Moderation is an operator act through `skills-moderate`, never HTTP:

```text
skills-moderate list      --registry DIR [--state STATE]
skills-moderate show      --registry DIR --name NAME --version VER
skills-moderate takedown  --registry DIR --name NAME --version VER --reason TEXT
skills-moderate reinstate --registry DIR --name NAME --version VER
skills-moderate admit     --registry DIR --name NAME --version VER --reason TEXT
skills-moderate evidence  --registry DIR --name NAME --version VER \
                          --suite SUITE --report REPORT --digest DIGEST
```

`admit` records a `moderation` stage and publishes through the same
gate — it answers an appeal; `takedown` removes a published version
from discovery; `reinstate` reverses it; `evidence` attaches a
measured report after publication. Every act appends to the book's
audit trail with its actor, kind, and reason.

## Configuration

```json
"skills": {
  "max_body_bytes": 65536,
  "submissions_per_day": 20,
  "pending_per_author": 10,
  "admit_score": 0.6,
  "review": {
    "endpoint": "http://127.0.0.1:9080",
    "model": "kev-0.6b",
    "timeout_ms": 30000
  }
}
```

The admission policy installs into `skills.json` at genesis, so a
reopened store admits under the same declared bounds; the review
stage's recorded policy version pins the question set a score was
produced under. `skills` requires `accounts`; `Config::check` refuses
the pairing otherwise.

## Refusals

| Code | Status | Cause |
| --- | --- | --- |
| `unauthenticated` | `401` | No bearer credential, or a dead session |
| `membership_required` | `403` | An anonymous session holds no account |
| `no_account` | `403` | The key authenticates but binds no account |
| `forbidden` | `403` | Another account owns the name or submission |
| `invalid_submission` | `400` | A field failed its bound |
| `consent_required` | `400` | `consent` was absent or false |
| `version_conflict` | `409` | The `(name, version)` exists at another digest |
| `rate_limited` | `429` | The day's submission bound is reached |
| `too_many_pending` | `429` | Too many submissions sit under review |
| `invalid_state` | `409` | The version's state forbids the act |
| `unknown_skill` | `404` | No published version resolves the name |
| `skills_unavailable` | `503` | The store or an object could not be read |
