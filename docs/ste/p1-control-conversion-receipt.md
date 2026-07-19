# P1 control conversion receipt

- Date: 2026-07-19
- Issue: #9049
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: complete

## Result

The P1 slices converted 15 control contracts and added four author templates.
The checker passes all converted contracts after the recorded reviewer dispositions.
The protected semantic token check reports no requirement-token reduction.
A tool result does not prove full STE conformance.

## Converted contracts

| Path                                                 | Source SHA-256                                                     | Converted SHA-256                                                  | State   |
| ---------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ | ------- |
| `AUTHORITY.md`                                       | `286201c9a2b67b7798e119a95e5cf2e102b618b1f41333d69a611d524e416f42` | `c27c798bfe197a9c9fb496d0947593756a5014fd33c517c8ee25746754af5e86` | checked |
| `docs/sol/CLAIM_PROTOCOL.md`                         | `c0aab92aa7ebf871d887df5a34b3324e8b794440da2195451901f14546bc2e82` | `153a608c4566fe25a3a599c883db31e52b77e95045d5eae9a8d94cb2bcbc35e3` | checked |
| `packages/assurance-spec/starter-kit/AGENTS.md`      | `c87d1623c41aadb7d46c23d4afbd2362b303622f25d3e0cf0c0bfeb7b7ec09ca` | `2a6cb07bee22d34f7d324a9ee6c9c2d8d524335e08d5feb1da5b80d6140ecaa5` | checked |
| `specs/CONVENTIONS.md`                               | `c5a7da4b74c04bb98a30dc7152360ff252452c3a37f681274cbc62f51f19923b` | `ce8205c250d1f08daf6e0f9e50a983d1e944b8748dcdb82a575361a339e6aaec` | checked |
| `docs/cloud/INVARIANTS.md`                           | `760ad950dc68cdc9a859d33c9cdd73bd296bcdf5c17d76d1509bfea1ce872b14` | `9350514b04bbee1eaf03b9a15c11951c110bc879e8ff0e973d916f060f007296` | checked |
| `docs/sol/README.md`                                 | `815fb1a6f8231a58304ab933d9b2493c2225acbef2bdaacc1e22c85c2dae5c6b` | `d2816331a446ca513c15137ba83c9aa613264af93a4889b35738b1ce5efeefd7` | checked |
| `docs/sol/OPERATING_MODEL.md`                        | `f609584a8ca75097e21202921deb8069cbfa759972ab5f0413c5f6184d9b432a` | `98e639ca39d425d764f9817cd23ffe0c21421e1414bdf804d6820fc8bbcab8b1` | checked |
| `AGENTS.md`                                          | `0e6da670e75c52bbefef9da80edef8d9af9d63a7c06b21fdc7ed4b5eda8a74e1` | `3d4364065b74f9a174a578397a6724a92b7585f89084f4c8307fcf0fd89ce2d0` | checked |
| `INVARIANTS.md`                                      | `a693804e6182c5f943ba5b181d14d46d2c9510db1cd74f8de74d0abc8abdd445` | `fcd3bb9f9955ad0d9b9015e6a1d40e485dd8e0568fa7b81c61ac862f278c519b` | checked |
| `docs/sol/MASTER_ROADMAP.md`                         | `2b66e3bfb7d81898ca82615b351d8e35979e28f4af50b554f10ecdeec248bf67` | `7f4b8aebd6ff2f568c8113b358aa5a83aa0aa4f3a32d5c0bb2a3c623dfafccb8` | checked |
| `apps/openagents.com/AGENTS.md`                      | `17d2f0d45aeeef4a86f92174788adca843a7883265802c4c14268108c6dfca1e` | `c020d7d74771bc3afca988a3cdac8ab7b5916dc44b2a4d1e037b7a84827e3a49` | checked |
| `apps/openagents.com/INVARIANTS.md`                  | `77a9869330ca4d7a71ad5f7e40de6a58040df8fd99f2683f8359fc59692ed490` | `c0d6b302c043aa64af3db1e6b0f39e85a0bdd121bfd397485c7d0c185e833cf8` | checked |
| `apps/openagents.com/apps/start/public/AGENTS.md`    | `a654849046108e48cfb6abe50f4d313c4edc21fba76b4a431bea3e2a92051db7` | `a654849046108e48cfb6abe50f4d313c4edc21fba76b4a431bea3e2a92051db7` | checked |
| `apps/openagents.com/docs/autopilot-tasks/AGENTS.md` | `e99b28142347c7ede13cce9f8a25e9e9c69a6ce9ed43616f9d4738eca7357352` | `639c6bfd34a0f72ef3c17af37b87fc8f8f2eebe9ea4e76adb48f119e691694b3` | checked |
| `apps/openagents.com/docs/live/AGENTS.md`            | `a654849046108e48cfb6abe50f4d313c4edc21fba76b4a431bea3e2a92051db7` | `a654849046108e48cfb6abe50f4d313c4edc21fba76b4a431bea3e2a92051db7` | checked |

The cloud and Sol reviews accepted the `STE-2.4` possible defects.
The process model review also accepted the `STE-3.6` possible defects.
The reviewer confirmed that these forms keep the correct technical semantics.
The agent-control review also accepted `STE-5.1` and `STE-8.2` density reports.
These dispositions apply only to agent-only compact profiles.
They keep each dense condition and its control in one context for fast agent review.

The P1 work reconciled five concurrent document changes from `main`.
The ledger generator refreshed only the baseline for each specified migration file.
The semantic checker refreshed only the root invariant baseline.
This action did not reset the baseline for other files.

## Agent compact clarification

The RC.25 release has separate human and agent changelogs.
The policy now uses this dual-audience pattern.
Human text uses the base STE profile.
Agent text should also use that profile when possible.

The `openagents-agent-compact-v1` overlay permits controlled technical terms and labeled record fragments.
It applies only to an identified agent section or agent document.
It cannot relax authority, safety, evidence, or ambiguity controls.
The strict lexical check limits the expanded terms to that identified text.

## Author templates

- `docs/ste/templates/audit.md`
- `docs/ste/templates/receipt.md`
- `docs/ste/templates/runbook.md`
- `docs/ste/templates/specification.md`.

## Verification

- `pnpm run check:ste:all`
- `pnpm run check:ste-control-semantics`
- `pnpm run test:ste`.

## Root control punctuation pass

The first root-control pass removed 454 prose semicolons.
It changed `AGENTS.md`, `INVARIANTS.md`, and `docs/sol/MASTER_ROADMAP.md`.
The pass kept the protected semantic token sets without changes.
All eight final controls now have checked agent profiles.
The app-local punctuation pass removed prose semicolons and expanded contractions.
The controlled term review changed the old two-word form to `ProductSpec` in the Sol roadmap.
