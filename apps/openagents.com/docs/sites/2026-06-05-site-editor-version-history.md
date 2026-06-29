# Site Editor Version History

Issue: `#168`

The Site editor sidebar now includes a customer-safe version history panel.
Each history row can show:

- whether the revision is current or prior;
- the revision ID;
- formatted creation time;
- a safe origin summary from the feedback/adjustment that produced the
  revision, or from safe version metadata for initial prompts;
- formatted origin time when one is known;
- review/build status;
- the dedicated version URL when available; and
- a button that starts a follow-up comment from that revision context.

The backend projection follows the version lifecycle instead of guessing from
text. For follow-up revisions, it uses feedback linked to an
`adjutant_adjustment_request` whose `resulting_version_id` matches the Site
version. For initial revisions, it falls back to customer-safe metadata fields
such as `originSummary`, `customerPromptSummary`, or `feedbackSummary`.

Safety boundary:

- Origin text is normalized, length-bounded, and dropped if it appears to
  contain provider secret material.
- The projection does not expose raw prompts, runner payloads, source archive
  keys, build logs, provider refs, or private run IDs.
- Prior revisions keep their dedicated `/versions/<version_id>` URLs rather
  than pointing every row at the stable live slug.
