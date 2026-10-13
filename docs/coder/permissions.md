# Coder's permission and its floors

Coder has one permission today, `bypass`: an agent's tools run without
asking (owner decision 2 of #11451). Each harness honors it with its own
modes. On top of it, the same floors hold on every harness (#11475).

This table is the one place it is written down. `crates/coder-new/src/floors.rs`
holds the same table (`floors::TABLE`), and tests fail when a harness's launch
drifts from its row or when this page stops naming what the table says.

| Harness | Permission | The harness's own modes | Where the floors hold |
| --- | --- | --- | --- |
| Coder's loop | `bypass` | no approval gate | Coder's tool gate: the Run tool's commands and file reads |
| Claude Code | `bypass` | permissionMode=bypassPermissions, allowDangerouslySkipPermissions | a PreToolUse hook Coder installs in the session |
| Codex | `bypass` | --dangerously-bypass-approvals-and-sandbox, approval_policy="never" | a PreToolUse hook Coder passes with -c hooks.PreToolUse, run without hook trust |

## The floors

These are held for the person on every harness:

- a production deploy (`openagents deploy production`, `wrangler deploy`,
  `gcloud run deploy`, `fly deploy`, `vercel --prod`);
- merging a pull request (`gh pr merge`, or the merge API);
- reading or changing a secret (`gcloud secrets`, `security … -w`, `op read`,
  `gh secret`, `wrangler secret`, and reading `.env`, `.secrets/`, private
  keys or credential files);
- a payment (`openagents wallet send` or `pay`, `lncli`, `bitcoin-cli`,
  `stripe … create`);
- a DNS change (`gcloud dns record-sets`).

Pushing commits to `main` is not a floor: this repository's agents commit
straight to `main`.

The checkout's approval policy (`.openagents/approvals.json`,
`crates/coder-new/src/risk_policy.rs`) gives each its rule. A file can make a
merge run freely or refuse it outright, but never lowers a production deploy,
a secret, a payment or DNS below asking, because a file a model can write must
not open the owner's gate.

When a rule asks and someone is at the terminal, the person sees one plain
choice on the approval card, such as "Let Claude (Haiku 5.5) merge a pull
request?", with the exact command. With nobody to answer, and always inside
Codex, the call is held: the agent is told to finish what it can and hand
that step back to the person.

## Pins

A person can pin a harness, and a model on it, for one repository, one kind
of work, or everywhere, by asking Coder ("always use Claude Opus for code in
this repo") or by editing `routing-preferences.md` in Coder's settings
folder. A pin decides ahead of the router's own steps and records as `pin`.
`/preferences` lists each pin in plain words. A harness the person said never
to use is never started; with none left, the message is refused and stays in
the input.
