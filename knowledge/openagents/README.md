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
- **Actionable.** When an `answer` tells the reader to do something, it
  carries the exact page as an `https://` link or the one command to run in
  backticks (`curl -fsSL https://openagents.com/cli/install.sh | bash`,
  `coder login`). The corpus check refuses an answer that gives an
  instruction with neither. Short numbered steps only when more than one
  step is truly needed.
- **`in-app`.** Tag an entry whose `answer` walks through screens of the
  OpenAgents phone or desktop app (Account > Computers, the Wallet's Send).
  The screen is the action there, so the check allows it, and the
  website's chat, where those screens are not, never shows it whole.
- **`off-computer`.** Tag an entry whose `answer` assumes the chat is not
  on a computer ("we can't reach your computer"). A chat on the computer
  Coder runs on (the desktop app, or `openagents chat`) never shows that
  answer whole; the entry still grounds the model's reply there.
- **Admission.** An entry is served only when `status: admitted`. Its
  `evidence` records the review against its sources. The answer text is
  user-facing copy: a change bumps `version` and needs the owner's review.

Test the corpus with `cargo test -p knowledge product`. The held-out
questions are in `crates/coder/fixtures/product-kb/questions-v1.json`; do
not tune entries against them.
