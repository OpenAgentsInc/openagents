# P3 high-risk procedure conversion receipt

- Date: 2026-07-19
- Issue: #9051
- Policy: `openagents-ste-policy-v2`
- Result: pass

## Inventory decision

The pre-conversion generated ledger identified 143 high-risk documents.
The risk rule intentionally finds more than active operator procedures.
It also finds plans, audits, receipts, public guidance, templates, and historical records.

P3 converts 15 current primary or supporting operator procedures.
It also converts and supersedes one obsolete Worker deployment runbook.
The superseded runbook points to the current Google Cloud deployment authority.

The ledger has two other checked high-risk documents.
One is the checked runbook template.
One is a checked incident after-action report.
The other 125 files stay in the migration state.

P4 handles public guidance.
P5 handles active plans and audits.
P6 handles remaining mutable, historical, evidence, and duplicate documents.

Thus, a high-risk filename does not automatically make a file an active P3 procedure.
The migration state is an explicit nonterminal disposition.

## Audience rule

These procedures are human-facing text.
They use the base mixed profile.
They do not use the agent compact density exception.

The review accepts only `STE-2.4` and `STE-3.6` screening results.
These rules require inspection of an `-ing` form or a possible passive form.
The review does not accept long sentences, dense paragraphs, semicolons, or contractions.

## Subject review

The conversion keeps commands, identifiers, URLs, issue refs, numeric values, and normative keywords.
An exact token comparison passes for all 15 current procedures.

The obsolete Worker runbook has one intentional semantic addition.
Its new warning prohibits the old deployment path.
The warning identifies the current Google Cloud script and app control document.
It does not change the retained historical command values.

## Digest record

| Document                                                                       | Source SHA-256                                                     | Converted SHA-256                                                  |
| ------------------------------------------------------------------------------ | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| `apps/acceptance-runner/docs/DEPLOY.md`                                        | `ab8016cde7d22d25fe58164e604f7175a0ef0cc10c5478b71a0504284b8704d6` | `d6ccc46f3a13f12de39ecb1f3dc4defdb0e6946e6a7ec3a768b93927a788ce37` |
| `apps/oa-updates/docs/release-set-v2-feed-runbook.md`                          | `8c0967a79e7eefac9731fd5d073db008f1a07231335355a16511f327352b43bd` | `038331dcd900087b48f2cb1f43a370ffea038379b51f5b7cc25781f0d76ec8b4` |
| `apps/oa-updates/docs/release-signing-runbook.md`                              | `d72f89b3e63c75efddfc1f4c66e75a48656a3edb8b8dadda3e0ff5f096393642` | `e64300a61d36654034bd0c68d3bb5bab57c914a893a2114bcbcf8f18d2520409` |
| `apps/openagents.com/docs/2026-06-05-chatgpt-device-login-operator-runbook.md` | `47ec822b77f8e58ccd2c8c47096dfafc9539322bb13f324176d144e11a38872a` | `e70c0bb549a198f0f4cd523d7b9daaad0e4da3887fa915abca54ef1d1abdc2d2` |
| `apps/openagents.com/docs/2026-06-15-openagents-web-deploy-runbook.md`         | `97cb6cfd6cfe09ce4c600bad7678e8375f3bb123f48a2122019856c90a7655c9` | `267f38f81a3aa28d964b82b13dd90c7f5ca58acf05c08c79bea9719717759cb9` |
| `apps/openagents.com/docs/2026-06-16-private-workspace-setup-runbook.md`       | `5fc325f4ada1fa7ca939edccbd4b16ae518e5447961fe7ff362a0d59d9f87bc1` | `8b8784e0af05ce3f7f1518c356510de71b7bd40c8a95eaa064cac011eccd39f2` |
| `apps/pylon/docs/cloud-node-deployment.md`                                     | `6767c51efb1027b18ac036edc241bccece8415cbfaa02b26231000e82f1383c1` | `a21fc40d3ecb75bb97de15e3a68f80671a1c67660b5cd1b34a977e97ea6fcf65` |
| `apps/pylon/docs/npm-publishing-runbook.md`                                    | `0f46c86cb860c9c78d400b5244c4432df5dcbfd46a56a646dcafd56df6a20dc8` | `18933f9bb54a39e396bc8354d844023aca070689dd6c6408c9e1edcd5d8aa0c2` |
| `docs/deploy/README.md`                                                        | `1ada46e6cf743bdf59a50569c59976ac60ecc33ea8225a31e7c2a42693a4a581` | `349d718559d66cdaf5a0de915435557ecf8af254da213ae821964eba8101a4fd` |
| `docs/deploy/agent-computer-production.md`                                     | `11635f08fa6d915a1b787cd9d5b28820af9f9b86d425b3dd741d1df9719f1de1` | `777642587b6648795914867e98d614f08bb88b239b36865a5ca0bad058e496ff` |
| `docs/deploy/openagents-audio-retention.md`                                    | `a0912894e2a0042ef9e81e369bb4736d4c85ec114eb333e7a0799e0931be12b0` | `1d94133296c3b11f08b71d6957a276afed002f56ef48b396f689c6f33a5e31ff` |
| `docs/deploy/openagents-desktop-cross-platform-release.md`                     | `102adfa8ce6d14eb7daad10c0c6a9e51533d56995c36458d60cfea46c468abf9` | `295cfe721e5823bbf957452b7003f5f2b5ada30fadc4d82d6a20f0e4651a53d4` |
| `docs/deploy/openagents-desktop-production-release.md`                         | `30077aa49251af41d6fe619cb1b94231e7e42ee20298352a31694368e08e32d7` | `09b3df641cb9512647ac14d93c50ba85d1a0cdf98e323d03dac1eb9c34dd246d` |
| `docs/deploy/openagents-desktop-release-coordinator.md`                        | `f6a8b3fee85c52116a6c7c0d948c0f3848928271cd3d05613b9ea9a0bbf66ed2` | `0e5bd4d0feee1b6ae9fa586cf3a219e2f7bbdbfbc7ca0a4545be4b8e879a55c9` |
| `docs/deploy/openagents-mobile-production-release.md`                          | `cebb6084b16f4ce1420aec0fd44e85104a7d0d5d4fc470c4a53ad974f8b86ff8` | `a6c843928b0f68d07293beffa6eb396b2607a576925e252b7d0bc28394b74b52` |
| `docs/ops/2026-07-14-vp1-treasury-wallet-recovery-runbook.md`                  | `ca02fb1aa5ac9aab8c663f84d021a2b6f9bfd7bd99c42973512812f300a76874` | `0feeba90cedaef37c7651899b8896a124597fccd874b9a620b66b6e31e0e4b9d` |

## Evidence

These checks pass:

- The STE checker for all 16 P3 documents
- The protected-token comparison for the 15 current procedures
- `pnpm run check:ste-control-semantics` for 49 protected documents
- `pnpm run test:ste`
- `pnpm run check:fast`

The migration ledger records every governed document and its current disposition.
The semantic baseline protects the converted procedures against later control-token drift.
