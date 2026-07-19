# P2 specification conversion receipt

- Date: 2026-07-19
- Issue: #9050
- Policy: `openagents-ste-policy-v2`
- Result: pass

## Scope

This conversion includes all 16 active specifications in `specs/`.
It also includes 12 active authoring documents and templates.
The invalid examples and the test fixtures remain source data.

The work used this risk order:

1. Authority, safety, privacy, managed sandbox, full auto, and trust text
2. Other active product and assurance specifications
3. Authoring instructions, starter documents, and examples

## Language result

The conversion removed prose semicolons and contractions.
It uses the approved ProductSpec and AssuranceSpec terms.
It did not change code, commands, paths, URLs, identifiers, or protocol values.

The 28 converted documents have a checked profile.
They use the agent compact profile because they are agent authoring or control records.
The review accepts only the sentence-density, paragraph-density, and `-ing` screening rules.
It does not accept a weaker requirement or an ambiguous instruction.

## Specification identity

The conversion did not change ProductSpec intent.
Thus, it did not change a ProductSpec revision number.
The converted bytes changed the exact ProductSpec document digests.
The bound AssuranceSpec documents now contain the new digests.

The assurance revisions changed as follows:

- Desktop trust-complete workbench: 1 to 2
- Full auto: 5 to 6
- Cursor capability parity: 1 to 2
- Managed agent sandboxes: 1 to 2
- Sarah owner orchestrator: 3 to 4

## Digest record

The source digest identifies the document before conversion.
The converted digest identifies the checked P2 document.

| Document                                                                         | Source SHA-256                                                     | Converted SHA-256                                                  |
| -------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------ |
| `specs/desktop/desktop-trust-complete-workbench.assurance-spec.md`               | `16312574c53487b2a0a60e4c69c5fdafe31debd25f041f767d4d5a70d50d3606` | `768893e823be6c953a122041103d90e04dcec65260e1f10c127d88fbdac413fd` |
| `specs/desktop/desktop-trust-complete-workbench.product-spec.md`                 | `a1520828d12d230d357306fe664a9153515034e1676df65252b20b5f60a9b643` | `019c4b3b55c69f94764bb44c65c6f5bbe135b2dfb71e40145907fbea5ddbe9d0` |
| `specs/desktop/full-auto.assurance-spec.md`                                      | `dce84fc8d7cf936ba72f65b5bb4ca7e04a52e05957f51643c98c4464c4facd8e` | `989a64c5e608394f749e5756ba42bdec7675bcf72fb26ed209e5fc73726a58a9` |
| `specs/desktop/full-auto.product-spec.md`                                        | `1ec816bd58dce62b71060381188e2a82307d4e50baa3ba86ee2d0f8a827857ef` | `5da9eba0601be1b2fe849dce96260e8a785d16590a60298d8180faf9442dba47` |
| `specs/mobile/mobile-any-host-fleet-controller.product-spec.md`                  | `2304fc1a3591ea446d98116a49465a2f3a83d96dac682ac0064c38997887a471` | `00e7aba617b55bcaad51cbdea6a9a6e56c1d450ccbb08e379e2171a2003682f1` |
| `specs/openagents/authority-delegation.product-spec.md`                          | `221240f3adf09e713d0aef4b42ea5f91dd17740ce422faa935a12c09daa07ed1` | `f811200d7d673f2704288d7db48174c4321e75d7858f038453339fc172aaee96` |
| `specs/openagents/cursor-capability-parity.assurance-spec.md`                    | `7f2999e168d64b34837dedb10ad9e16b2121e2b5996eb9a301da3fe1624892e2` | `9e1a68a742bba7ab7aca2b5cfd840949e431b77d1a8f271f4febe4d3373363b8` |
| `specs/openagents/cursor-capability-parity.product-spec.md`                      | `f26c6335d258de9c62594bbba643abd56bf2a963e6db196d492ca58309a49fcb` | `a50322b69d6b85e4d65c38f349e65d6ab656a4b147fdf0e17061d1f0dbc27498` |
| `specs/openagents/fast-follow.product-spec.md`                                   | `1da9037a1754fe0cd3b018800eaee4f815302ebd181d7ec9e9bc8af89ad5f4f3` | `8bfd79d0e779887adde3048f16e109fa5e56d7a119148a26b4e18406841e6b78` |
| `specs/openagents/managed-agent-sandboxes.assurance-spec.md`                     | `a27c0f4deb90feb2bbf5edb9b3718ab4c407a4b732e4dd5a4303fec53853294a` | `673670006fd1e192ed3b16d71784e11158769f10a48dfe0ac3872cc0a9d1a465` |
| `specs/openagents/managed-agent-sandboxes.product-spec.md`                       | `a3c17e61c25807e16913f59a15d48f499a0b6502a68bde957cc46e38168300c1` | `0bef38178b696e0a3866f5206862af36fa80d31537d61bc15b90f2be3379665f` |
| `specs/openagents/portable-coding-sessions.product-spec.md`                      | `e072eec0c38d03f50637605e3611f8b758097e5a9dbd210adbd02f278a6f882b` | `e2899f19f72e8def2e48a564e11d635cb921e0c7c98bb00bdb3cb702a627df3b` |
| `specs/openagents/sarah-owner-orchestrator.assurance-spec.md`                    | `d21ed52d7b0be950131181cd1a5bd9c5c22313fc949d5c87e3f30617386647c8` | `b6048674a828a82c4331382db879de75085cf6142d2ce8d3852eb8bd3857ce93` |
| `specs/openagents/sarah-owner-orchestrator.product-spec.md`                      | `040b813b20d24870c24e67490c6de35cd278a3389e2aea6ea5a89d5c8467a89b` | `9de58d7e23e5488783f42fe4312b029d6d507b76818283c9ce4cab9d09c93bea` |
| `specs/web/openagents-com-sales-landing.product-spec.md`                         | `54ffaa824c4b0f7ca4e25105b1f179b8e1a4fb2ebb79dd811758baddcc542b1e` | `2b26b6051097ee5ffcab945d8913d76301c2f6a930e44fa722b5d5fdf75c9d36` |
| `specs/web/openagents-com-trust-surface.product-spec.md`                         | `c0aceca0613c53dc2c1656f5e5740b7907f90aee285b8d9b015bd9c0c4514380` | `775ed839e1e7379edfe456216b34190c54b9de927b8e3fdd2f28b9c65899d0d2` |
| `packages/assurance-spec/starter-kit/assurance/example.assurance-spec.md`        | `3d5593a9d568d249f5b4c5cc9ff0ebc6b26c6bcd3e2f20aedc7a16abf6320def` | `af38ba9004473f0a4e9a0f4816f60f9426af0014b97146f67c45d1b3bab55bda` |
| `packages/assurance-spec/starter-kit/docs/product-specs/example.product-spec.md` | `61ba3a36686dcfc0562105ed999e04c1db3b525846780c4a5418f35471dbab43` | `61ba3a36686dcfc0562105ed999e04c1db3b525846780c4a5418f35471dbab43` |

## Evidence

These checks passed for the converted scope:

- `pnpm run check:ste:all`
- `pnpm run check:ste-control-semantics`
- `pnpm exec vp test --run packages/product-spec/test/product-spec.test.ts packages/assurance-spec/test`

The semantic baseline protects 33 control and specification files.
It checks normative keywords, code literals, URLs, issue references, and numeric values.
The package tests confirm stable parsing, binding, compilation, distribution, and skill contracts.

## Review decision

The P2 documents keep their technical requirements.
No sentence gives new authority or removes a safety condition.
The exact digest changes and assurance revision changes are recorded above.
