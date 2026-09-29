# OpenAgents product knowledge

Short, sourced entries about the OpenAgents product that the chat answers
product questions from: the `product.kb` route of the
[chat router](../../docs/coder/design/2026-09-28-chat-router.md#knowledge-routes-product-and-codebase).
`crates/knowledge/src/product.rs` loads and checks them, and
`crates/coder/src/product_kb.rs` retrieves them.

Each file is a knowledge entry of kind `product` with an id under
`openagents.`, written from this repository's public documents:

- **Sourced.** `provenance.cites` lists the repository paths the entry was
  written from, primary source first. Every path must exist. Never cite a
  private repository, a local file outside the checkout, or a URL.
- **`answer`.** A short, complete reply (at most 600 characters) in the
  OpenAgents voice: "we", never "I" or "me". The chat shows it whole only
  when Jev judges that it fully answers the message. Put interface labels
  that contain "my" in bold (**My reports**).
- **`applies_when`.** The questions the entry answers, written for the
  embedding search and for Jev, with what it does not cover when a
  neighbor is close.
- **Body.** The answer, then details a grounded reply may use, then the
  sources.
- **Admission.** An entry is served only when `status: admitted`. Its
  `evidence` records the review against its sources. The answer text is
  user-facing copy: a change bumps `version` and needs the owner's review.

Test the corpus with `cargo test -p knowledge product`. The held-out
questions are in `crates/coder/fixtures/product-kb/questions-v1.json`; do
not tune entries against them.
