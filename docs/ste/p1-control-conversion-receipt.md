# P1 control conversion receipt

- Date: 2026-07-19
- Issue: #9049
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: active partial receipt

## Result

The P1 slices converted six control contracts and added four author templates.
The structural checker reports no defects in these ten files.
The protected semantic token check reports no change in the six contracts.

This receipt does not close P1.
The other control documents remain in the migration state.
A tool result does not prove full STE conformance.

## Converted contracts

| Path                                            | Source SHA-256                                                     | Converted SHA-256                                                  | State   |
| ----------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ | ------- |
| `AUTHORITY.md`                                  | `286201c9a2b67b7798e119a95e5cf2e102b618b1f41333d69a611d524e416f42` | `c27c798bfe197a9c9fb496d0947593756a5014fd33c517c8ee25746754af5e86` | checked |
| `docs/sol/CLAIM_PROTOCOL.md`                    | `c0aab92aa7ebf871d887df5a34b3324e8b794440da2195451901f14546bc2e82` | `153a608c4566fe25a3a599c883db31e52b77e95045d5eae9a8d94cb2bcbc35e3` | checked |
| `packages/assurance-spec/starter-kit/AGENTS.md` | `c87d1623c41aadb7d46c23d4afbd2362b303622f25d3e0cf0c0bfeb7b7ec09ca` | `2a6cb07bee22d34f7d324a9ee6c9c2d8d524335e08d5feb1da5b80d6140ecaa5` | checked |
| `specs/CONVENTIONS.md`                          | `c5a7da4b74c04bb98a30dc7152360ff252452c3a37f681274cbc62f51f19923b` | `ce8205c250d1f08daf6e0f9e50a983d1e944b8748dcdb82a575361a339e6aaec` | checked |
| `docs/cloud/INVARIANTS.md`                      | `760ad950dc68cdc9a859d33c9cdd73bd296bcdf5c17d76d1509bfea1ce872b14` | `9350514b04bbee1eaf03b9a15c11951c110bc879e8ff0e973d916f060f007296` | checked |
| `docs/sol/README.md`                            | `815fb1a6f8231a58304ab933d9b2493c2225acbef2bdaacc1e22c85c2dae5c6b` | `d2816331a446ca513c15137ba83c9aa613264af93a4889b35738b1ce5efeefd7` | checked |

The cloud and Sol index reviews accepted the `STE-2.4` possible defects.
The reviewer confirmed that these forms are technical terms or permitted modifiers.

The P1 work reconciled five concurrent document changes from `main`.
The ledger generator refreshed only the baseline for each specified migration file.
The semantic checker refreshed only the root invariant baseline.
This action did not reset the baseline for other files.

## Author templates

- `docs/ste/templates/audit.md`
- `docs/ste/templates/receipt.md`
- `docs/ste/templates/runbook.md`
- `docs/ste/templates/specification.md`.

## Verification

- `pnpm run check:ste:all`
- `pnpm run check:ste-control-semantics`
- `pnpm run test:ste`.

## Residual work

Convert all other P1 control documents.
Complete the technical review and the STE inspection for each important contract.
