# Artanis-as-a-Service Phase-1 Smoke Repo

This is a public, self-contained fixture repo for invited Codex fleet testers.
It is intentionally small enough for a first Khala -> Pylon -> Codex assignment
to finish quickly while still proving the full loop:

- a human-readable backlog item,
- a deterministic code change target,
- a local verification command,
- public-safe evidence to cite in a demo.

The fixture never requires secrets, private repositories, wallet material, raw
provider payloads, or the user's default `~/.codex` home.

## Tester Task

Ask the fleet to verify or re-implement the behavior in `src/backlog.js`:

> Ensure each `buildFleetPlan()` account row includes `riskLevel`. Accounts with
> `readiness: "ready"` are `"low"` risk, all other accounts are
> `"needs-attention"`. Keep the summary counts unchanged.

## Local Verification

From the fixture root:

```sh
bun test
```

Expected result after the task is complete:

```text
2 pass
```

## Public Proof Checklist

A completed tester run should record only public-safe refs:

- the public repo and pinned commit used for the run,
- the issue or task summary,
- the verification command (`bun test`),
- the assignment ref and durable request id,
- the closeout status and public result refs,
- the before/after public token-counter values.

Do not paste agent tokens, Codex auth paths, raw SDK events, raw terminal output,
private prompts, wallet material, or local workspace paths into public reports.
