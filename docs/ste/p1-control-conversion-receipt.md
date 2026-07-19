# P1 control conversion receipt

- Date: 2026-07-19
- Issue: #9049
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: active partial receipt

## Result

The first two P1 slices converted four control contracts and added four author templates.
The structural checker reports no defects in these eight files.
The protected semantic token check reports no change in the four contracts.

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
