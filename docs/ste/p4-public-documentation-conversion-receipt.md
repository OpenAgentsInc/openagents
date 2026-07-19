# P4 public documentation conversion receipt

- Date: 2026-07-19
- Issue: #9052
- Policy: `openagents-ste-policy-v2`
- Result: pass

## Inventory decision

P4 reviews 17 author-controlled public documents.
Eight files are the source for the public `/docs` site.
Nine files are public guides or agent API and control documents.

The documentation generator makes 22 artifacts from the eight `/docs` sources.
Thirteen artifacts are in the public docs directory.
Nine artifacts are compiled TypeScript modules for the site.

Ten generated Markdown or text artifacts are in the STE ledger.
The ledger now identifies these files as generated source data.
The generator source remains the language authority.
The check mode rejects a stale generated artifact.

The public font license is third-party source data.
P4 does not change it.

## Audience rule

The eight `/docs` sources, `INSTALL.md`, and `QA-RUNNER.md` use the base human profile.
They do not use a density exception.
The review accepts only `STE-2.4` screening results for approved technical nouns or modifiers.

The API and control documents use the agent compact profile.
This profile accepts reviewed sentence or paragraph density only when it improves fast and precise agent use.
It does not accept semicolons, contractions, ambiguity, or weaker safety and authority rules.

The Desktop rc.25 release note supplies the example for a dual-audience release record.
Its human changelog explains user-visible changes.
Its agent changelog gives compact issue, commit, contract, invariant, and evidence data.

## Generator control

The STE checker configuration identifies `apps/openagents.com/apps/start/public/docs/` as generated source data.
The profile source points to `apps/openagents.com/apps/start/content/docs`.
Authors must change the content source and then run the documentation generator.

The generated Markdown mirrors keep the exact source bytes.
The generated `llms` files use source descriptions and source bodies.
Thus, new explanatory text comes from an STE-governed source or a fixed STE generator string.

## Subject review

The conversion keeps commands, identifiers, URLs, issue refs, numeric values, and authority boundaries.
The structural rewrite changes prose semicolons into approved punctuation.
It does not change the normalized word sequence.

The language addition in `AGENTS.md`, `AGENTS-CORE.md`, and `agent-readable.md` records the human and agent profiles.
It links the rc.25 release note as the audience example.
The updated semantic baseline records the intentional URL and revision additions in `AGENTS.md`.

## Digest record

| Document | Source SHA-256 | Converted SHA-256 |
| --- | --- | --- |
| `apps/openagents.com/apps/start/content/docs/agent-readable.md` | `12cd3fdc409650ad03b7296b04a9ec9a3eacff47b5712d10409931c450daeca6` | `5c696864d898bc63a51c5bdb4b47dbeb3cd66f42bcda0a53f6d230d7f8c360fc` |
| `apps/openagents.com/apps/start/content/docs/full-auto.md` | `ec111d93d586e6666179769a272b10ecd8d457548453cccc374f4986fb23c5f1` | `c47356ca33d8dd03aaecbc9c993412b1834c0c05f1307cdd0e23b292efc759e7` |
| `apps/openagents.com/apps/start/content/docs/getting-started.md` | `e0fd631f65bf83f23f3de91ff1321d070d390f7d78e5e175a96e8ed105cc4aef` | `2b1a9f9876612baf64438e7ef0fe5690533255293a6552863423a18b430426fa` |
| `apps/openagents.com/apps/start/content/docs/index.md` | `07df9bfc6303efc5226da41c65eed0a619683d034aec1db4c75164ae2a29a06d` | `6c84c2b57bc738b0dc3bbf548e4c548a29e4bd7dd8c0b9af0dac04f00b05ec08` |
| `apps/openagents.com/apps/start/content/docs/review-and-recovery.md` | `ffde339940b7438792e2e023b16691449f45670755a793308b709c50d9910446` | `4385b108d6f2f97b98152655f1f734107903bd85fcd0644fded298a8ed542a3b` |
| `apps/openagents.com/apps/start/content/docs/security-and-privacy.md` | `f32f4182910f7772fc3c510194fb8e4c39e0ceb83bd6bb55f03bebccd16030ce` | `7fad6d574a31ec07ded303518c7914ea971b90655cf37459985a41762bd95a42` |
| `apps/openagents.com/apps/start/content/docs/troubleshooting.md` | `6ac889c04eedc96ad860abbca4033bb9017b1530c62c158344a69761c72a68bc` | `c6eaeab21d8e34c1822c9e409fd6d57d50bcf8bc82798f8a857fa7eaf8cd1b29` |
| `apps/openagents.com/apps/start/content/docs/workroom.md` | `5c0b1e8eb197d662f6ba05346f37d2b2c40c0205c5f2b3521e7320b16697c895` | `041f2313ddbf535a729a2779dd4d427182dc0e0e0f3e75915072d5160d6c0a4b` |
| `apps/openagents.com/apps/start/public/AGENTS-CORE.md` | `2661a56cd643b384f1723ad7539bd8480e9b2bb8f4fabac4df20ec4e9f07de8f` | `78b2c273ead6c83962f54e9811cf63d9289d3215b4038e97427dbc883a252940` |
| `apps/openagents.com/apps/start/public/AGENTS.md` | `a654849046108e48cfb6abe50f4d313c4edc21fba76b4a431bea3e2a92051db7` | `a3deb2e356f2cd979d0f24f98ab27e41b51e9f782ced088deb75d0652387aa29` |
| `apps/openagents.com/apps/start/public/HEARTBEAT.md` | `156b5d4fbaf3026e377d9c4151d7e28ba9b5bbe9af387589a17d9968c05761f8` | `156b5d4fbaf3026e377d9c4151d7e28ba9b5bbe9af387589a17d9968c05761f8` |
| `apps/openagents.com/apps/start/public/INSTALL.md` | `fa580afc1b9b8b467103bc4d68224a58e8194440dfbbc30142a22a3a0b4598cb` | `eb6d983718c9463202fce7b75ae4cb80e1fdacb27f274182e7accbb83410c4e6` |
| `apps/openagents.com/apps/start/public/PYLON.md` | `4ff3cbeb1a9f89fd3d24214c411842e9657ec736d332c0dcd551f61f66338be2` | `e31cf57f1cca0c4c579aeb4e984fd18f8da2eb5aab43330f8c5bfba930c043b4` |
| `apps/openagents.com/apps/start/public/QA-RUNNER.md` | `66058119462bce25f5d2e5ff1f35fe600bd39a345381be38e8dbd0b7db48c978` | `9ed16e7704af5a04f9bb8b5284619c3aedf1ec38701fe24918fd1264338c89a8` |
| `apps/openagents.com/apps/start/public/RULES.md` | `335eed468b6cb667a9941760c813d1ad00ee63b0cc733641798aa820270b8d65` | `335eed468b6cb667a9941760c813d1ad00ee63b0cc733641798aa820270b8d65` |
| `apps/openagents.com/apps/start/public/SITES.md` | `5ee9be7ab31db8d970a151ffc990a6fbe33523b5fa1ef2a9d5dd271458772945` | `cf9c6005e4265a6f383681d279a8f4d254adefa708a3a4f3df17059a91fe5bd9` |
| `apps/openagents.com/apps/start/public/SURFACES.md` | `e63de3510d2e3e2d012555ccfb6d329a09d5e75c67e5761f4aa91c1d903b5456` | `4143bf4ef4d2ac4a0eb045732cc5d99882055181ea6f1616e20cd7e8b0b12752` |

## Evidence

These checks pass:

- The STE checker for all 17 author-controlled public documents
- The public documentation generator in write and check modes
- The Start documentation content tests
- The STE checker tests
- The semantic check for 49 protected control documents
- The root fast check

The migration ledger records the author-controlled profiles and generated source-data profiles.
