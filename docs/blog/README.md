# Blog drafts

Posts written for openagents.com before the site has a blog. Drafts live here,
outside `apps/openagents-web/`, because everything in the web app is served
and released with the site; a draft there would go public on the next
release.

| Post | Author | Status |
| --- | --- | --- |
| [Attention that cannot miss](2026-10-29-attention-that-cannot-miss.md) | Claude | Draft: waiting on owner review |

## Format

- **File name:** `YYYY-MM-DD-slug.md`.
- **Front matter:** the site's page fields (`title`, `description`, `url`), plus
  `author`, `date` and `status`. The `url` is the address the post will have
  once published.
- **Copy:** plain and concrete. Numbers must come from a check in the
  repository, or be labelled historical with their source named.

## Publishing a post

Nothing here is published automatically. The owner decides whether and when a
post goes out.

1. **Owner review.** Read the draft and edit or reject it. A post by an AI
   author stays attributed to that author. If its substance changes, the
   author revises it, or the byline changes to say who did.
2. **Recheck facts.** Before release, rerun the checks behind the post's
   numbers (for the first post, `psionic/tests/check_trace.sh`) and confirm
   that its links resolve on `main`.
3. **Place the page.** When the site has a blog route, copy the post to
   `apps/openagents-web/blog/<slug>/index.md`, keeping the Markdown twin
   convention of `llms.txt`. Then:
   - add a line to `apps/openagents-web/llms.txt`;
   - drop `status` and set `date` to the publication date;
   - draw the visible page in Coax, since nothing visible on the site is HTML
     (`INVARIANTS.md`).
4. **Release.** Release through the normal site path (`coax/web/release.sh`).
   Set the status in the table above to **Published** with the date and
   address.
5. **Sharing.** The owner posts the link from OpenAgents accounts. Claude
   doesn't post to external services.

## Requests from the author of the first post

- **Byline:** keep "By Claude, an AI model made by Anthropic" and the opening
  note explaining that the owner asked Claude to choose a chapter. Readers
  should know who wrote it and why.
- **Credit:** keep the credit to Percepta's published work. The construction
  is theirs.
- **Honesty:** keep the slowdown (about 1,300 times slower than a CPU
  interpreter) and the learned executors' zero score. The post's argument
  depends on stating its limits.
- **Ask before changing claims:** if the published numbers change, update the
  post, or ask Claude to, rather than leaving an old claim on the site.
