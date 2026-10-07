# Exact-key discoverability pilot

Run this pilot only after an owner selects an already authorized public identity,
approves its public profile and exact capability/release links, and consents to
the private evidence record and retention period. This package does not publish.
An owner separately signs and publishes a kind-`0` profile under their identity.
Do not publish as NosFabrica or assume Brainstorm indexes OpenAgents records.

1. Record the approved public key, public profile content and links, consent,
   and any signed profile event in private files. Keep a directory with `0700`
   permissions and files with `0600` permissions on Unix. Use the owner's
   approved storage; do not commit real evidence to this package.
1. Submit an explicitly approved public profile search in Coder, then rank the
   exact key. Retain each complete normalized `Observation` JSON from its tool
   result, preserving source, house attribution, algorithms, expiry, and body
   digests. In headless JSON output, select the completed Brainstorm tool's
   `entry` or `delegation_entry` event and save `entry.output.observation`.
   The tool has `source: "tool"` and `running: false`. Do not use
   `finished.reply`, which is an 8 KiB model-context projection and can omit
   subjects. Retain the full bare observation without projection fields.
   Do not retain an opaque `input_ref` as pilot authority.
1. Copy the structure of `examples/pilot-fixture.json` into a private record.
   Set `basis` to `operator_recorded`, use the exact key, actual consent and
   evidence file digests, and project only that key's fields from each complete
   observation. `absent`, `unknown` zero, `unavailable`, and `reported` are
   distinct. Old observations remain historical evidence with visible expiry.
1. Add only stages for which you have separately consented retained evidence:
   referral, exact guidance install, accepted task, recorded payment, and repeat
   use. Missing stages stay missing. Do not infer them from lookup success.
1. On a supported Unix CLI host built from the REV-37 source, check the private record:

   ```sh
   openagents --json plugin brainstorm-pilot check --input <private-record.json> --sources <private-source-directory>
   ```

The checker reads only these explicit bounded files. It verifies file digests,
exact-key observation projections, public profile signatures when supplied,
recording consent times, and causal stage references. It reads no service,
wallet, pipeline, referral ledger, query history, or default home state.
It creates no public or private history store. Its summary excludes input text,
source content, contact details, and public-key lists.

## Record fields

Use schema `openagents.brainstorm.pilot.v1`. IDs are opaque local tokens, not
customer names or contact addresses. Every `Reference` has a relative `path`
and lowercase `sha256`. The record allows 16 lookups, 64 funnel events, and
16 approved public links. Files allow 256 KiB each, the record 512 KiB, and all
selected sources 2 MiB. Empty evidence, links, and traversal refuse. The checker
runs on supported Unix CLI hosts and checks private source permissions. If you
retain records elsewhere, protect them with that system's private ACLs before
bringing selected copies to a checker host.

`consent` records approval for this private record and `retain_until_ms`.
It is separate from native disclosure and public-profile approval. The checker
refuses after retention expires. Supply `profile_approval` for public links or
a `profile_event`; the latter pins a signed kind-`0` event under the exact key.
Signature verification proves its signer, not publication or indexing.
Put approved links in profile string fields or as exact whitespace-separated
URLs in `about`; a matching URL prefix does not establish the approved link.

Each lookup pins a complete normalized observation. Context wrappers and added
projection fields refuse; stripping truncation metadata does not restore an
omitted subject or establish absence. Copy the target's relevance,
influence, and coverage and every response's endpoint, algorithm, times, and
digests into the record. The checker compares the projection with those bytes.
`discovery` in the summary counts retained lookup records, including absence;
inspect `lookup_coverage` for exact-key success and expiry.

Funnel event variants share `id`, `at_ms`, and `evidence`:

| `stage` | Additional fields and meaning |
| --- | --- |
| `referral` | Optional prior `lookup` ID; an attested referral, not a commission entitlement. |
| `install` | Exact `package` (`publisher:slug`), `version`, `manifest_digest`, and optional `release_id`; guidance installation proves no native enablement or use. |
| `accepted_task` | Distinct `task_id`, opaque `buyer_id`, and `artifact_digest`; pin the customer's actual acceptance evidence. |
| `paid_use` | Prior `accepted_task` event ID, payment `state` (`settled`, `unknown`, `failed`, or `reversed`), optional integer `amount`, and declared `unit`; settled claims require a positive amount and retained settlement evidence. |
| `repeat_use` | Two distinct prior settled `paid_uses` event IDs from distinct accepted tasks for the same opaque buyer ID; pin independent repeat evidence. |

All dates use Unix milliseconds. References name earlier or equal event times.
Each task has one current payment declaration; replace its state and pinned
evidence if settlement fails or reverses. One task cannot count as two paid uses.
Missing or unknown payment evidence
establishes no settlement. Records and successful digest checks remain fixture
or operator claims; they do not independently verify payment, deploy a service,
qualify commercial conversion, or activate another capability.

`joins.pipeline_lead_id` and `joins.referral_attribution_id` are optional
correlation IDs for separately authorized REV-05 and REV-26 adapters. The checker
does not resolve them or require those lanes for native reads. Actual customer
conversion, referral eligibility, attribution, settlement, and repeat activation
need their owning records and owner operating evidence separately.

## Offline example

The `examples/` files use synthetic public keys and an `.example` HTTPS origin.
Copy them into a private scratch directory, set the permissions, and run the
checker there. The consent is fixture data, not a human grant. Search absence
and rank zero deliberately retain limited coverage. The example contains no
referral, install, customer, payment, or repeat event. Tests check these files
without reaching a service or publishing anything.
