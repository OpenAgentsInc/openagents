# Meeting follow-up template

Use this local operations workflow to turn one authorized meeting note into
cited action items for a human to review. It reuses the public Rust/Wasm
[action-items guest](../../crates/plugin-action-items/) and its existing
[build receipt](build-receipt.json).
[Provenance](provenance.json) names exact package, program, and guest digests.
Version 0.1.0 is a source package, not a signed or published marketplace release.
The [retained synthetic comparison](evidence/README.md) records the actual local
CLI result and its separate protected check.

The selected workflow drafts a review list. It does not send messages, create
tickets, schedule meetings, verify identities, or approve the work described by
an item. Notes and output remain untrusted data. The human owner reviews every
owner, due date, and source citation before a separate authorized handoff.

## Start a new customer task

Use an `openagents` binary built from the commit containing this template and
`coder::workflow_template` (REV-45, #10852). This local Unix runner needs no model
credential. Other hosts can inspect the package; its snapshot commands require
this native binding. Team distribution, paid catalog entries, and remote
operation remain separate capabilities.

1. Agree to the workflow, human owner, input/disclosure rights, reviewer,
   retention, support, and protected success criteria through the existing
   [pilot](../../docs/sales/pilot-kit.json) and
   [handoff](../../docs/sales/delivery-kit.json) kits. Keep real agreements
   private. The command checks operator declarations, not genuine consent.
2. Create a mode-0700 customer task directory. Put only this customer's
   separately authorized UTF-8 notes in a mode-0600 file. The file may have any
   name; the guest receives it under the fixed logical name `meeting.md`.
   Prepare the input snapshot:

   ```sh
   openagents plugin template prepare --package /path/to/plugins/meeting-followup \
     --source /private/task/notes.md --task task-reference --customer customer-reference \
     --owner responsible-human --recipient local-reviewer --permission-epoch 1 --json
   ```

3. Privately save the returned `snapshot` object as `snapshot.json`, mode 0600.
   Review its exact task, customer, owner, reviewer, source, and release digests.
   Use its `snapshot_digest` for one deliberate run:

   ```sh
   openagents plugin template run --package /path/to/plugins/meeting-followup \
     --source /private/task/notes.md --snapshot /private/task/snapshot.json \
     --approve REVIEWED_SNAPSHOT_DIGEST --recipient local-reviewer \
     --permission-epoch 1 --json
   ```

4. Privately save the returned `report` object, mode 0600. An independent
   checker freezes the expected items, owners, due dates, exact source lines,
   unassigned count, and completed-item count in the
   [protected check format](../../crates/coder/fixtures/workflow-template/protected.json).
   Select its raw SHA-256 digest before checking:

   ```sh
   openagents plugin template check --source /private/task/notes.md \
     --snapshot /private/task/snapshot.json --report /private/task/report.json \
     --protected /private/checker/expected.json --protected-sha256 FROZEN_SHA256 --json
   ```

The commands write only stdout. Capture and retention are your separate local
actions; protect redirected files before putting customer data in them. A
passing check is a local attributable comparison, not remote attestation or
customer acceptance. Retain failed attempts and arrange a new reviewed run;
the runner has no cache, prior-customer store, or automatic retry.

## Inputs, checks, and limits

The guest sees exactly one complete captured file and empty request text.
Protected expectations, customer references, approval records, other customer
files, credentials, home configuration, and private configuration stay outside
its snapshot. A changed input, task, owner, customer, package, or program needs a
new snapshot approval. Changed recipients or revoked rights require a new
current permission epoch and approval; the runner has no consent service that
can discover changes for you. Approval never transfers to another task.

Notes must fit 60 KiB. The existing program host enforces 50 million fuel units,
8 MiB guest memory, 64 KiB output/read bounds, 128 KiB module bytes, one step,
and a 10-second deadline per attempt. The guest has no write, network, process, or spend
imports. All three commands are `ReadOnly`: running Wasm computes over immutable
memory, and the runner persists nothing. Local installation uses the existing
`plugin install` command and leaves the package off; installing or enabling it
does not approve a customer's input snapshot.

Only explicit item markers, supported headings, or recognized named statements
produce items. Unassigned owners remain unknown; due dates remain literal text,
not calendar dates or time zones. Completed checkboxes are counted and excluded.
Empty notes are a checked empty result, not useful customer work. Truncated,
unread, missing, failed, or partial output fails protected acceptance. File
paths must contain no symlinks; private records require a private parent and
mode-0600 regular files. Changes or unsupported release bytes refuse execution. The program's
`captured_input` bound refuses generic `plugin run`, installed-plugin reuse, or
ordinary program selection; this native snapshot runner supplies the approved
captured input. An older host that does not enforce this bound also refuses it.

The comparison executes the same guest with empty text and no file handles.
It measures cited extraction with and without file access. It does not measure
model quality, time saved, paid usage, or genuine department adoption. Owning
Cargo fixtures independently cover a second synthetic customer, rights/input
changes, denial, cancellation, injection-like notes, protected labels outside
scope, and failed or partial results. Public examples are synthetic only.

## Reuse and support

Reuse the public package under its [Apache-2.0 license](LICENSE) and preserve its notices.
Filled agreements, pilot artifacts, identities, customer rules, notes, and
configuration remain private. Before copying any genuine pilot learning into a
new package, complete the handoff kit's exact material rights and privacy review;
publication requires a separate owner approval and the existing plugin
freeze/test/review/publish flow. Do not publish this local check as a Gym result
or paid customer result. Keep the human support owner and end date in the private
handoff, stop on unknown permission or failed checks, and delete retained customer
copies at the agreed date. [Owner qualification](../../NEEDS_OWNER.md) records
the genuine agreement, disclosure, publication, and adoption steps.
