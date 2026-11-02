# Aethyr next work after the issue review

The next milestone is chapter 11's complete playable Grid route, followed by
the remaining renderer profiles. Keep the book in dependency order. The
registry currently has 13 implemented chapters, five partly implemented
chapters and 68 design drafts. Chapter 11 is still partial; chapter 12 has
not been started in this continuation.

This review fetched `origin/main` through `167b6abe57` and read all 17 open
GitHub issues on 2026-11-02. The current private integration checkpoint is
`88a89f8ddf`, based on main through `681cf6ba2c`; subsequent tooling changes
are staged or in progress. Fetching main is not a completed integration or
publication claim.

## What changes the approach

- [#12628](https://github.com/OpenAgentsInc/openagents/issues/12628) remains
  the immediate acceptance contract: one playable consumer must use shared
  frame, binding and resource contracts, with native/browser evidence for
  the same declared scene and inputs. Preserve the already delivered frame
  API and reusable HUD. Additional shader ports alone do not close it.
- [#12625](https://github.com/OpenAgentsInc/openagents/issues/12625) is closed.
  Its focused R2/B2/B3 compiler and consumer gates passed. The earlier report
  of 130 unresolved findings in that issue is historical. Broader compiler,
  platform and product readiness remain separate.
- [#12639](https://github.com/OpenAgentsInc/openagents/issues/12639) is closed.
  Main now runs the Forge maker journey at desktop and phone sizes during
  release. Preserve that integration; do not create another journey driver.
- [#12638](https://github.com/OpenAgentsInc/openagents/issues/12638) remains
  open and externally owned. However, owner commit `a02afa2228` now builds
  website releases with `COAX_MUTATION=0` in a separate cache. It reports
  compiler survivors instead of waiting for mutation during release. Thus
  this issue no longer has the same website-release blocking role described
  in its older comment. The publisher/full mutation qualification remains
  unresolved; a fast release build does not establish zero survivors.
- Main also raises source admission to 4,096 units and 64 MiB in
  `7027896698`, with a refreshed loader seed in `eda4077928`. Integrate that
  source change before choosing the final compiler. Retained runs with the
  explicitly identified older compiler remain evidence for their original
  inputs, not current-toolchain qualification.

## What should happen next

1. Finish the current Coax tooling ports and their refusal/parity checks.
   Preserve independent references and narrow browser/Vulkan SDK effects.
   Retain immutable evidence with its actual source, compiler and observer
   identities. Run the actual tracked-tools gate before pushing.
2. Commit the coherent checkpoint, rebase on current main, and resolve both
   sides of overlaps. In particular preserve the release journeys, compiler
   changes and the independent DeepSeek wrapper update.
3. Qualify the final integrated Grid source and chosen matching compiler.
   Reuse passing evidence only when its inputs are identical. Existing
   walking/jumping, camera/feet, replay, resource refusal/recovery and moving
   pixel records are substantial bounded evidence; they do not cover every
   subsequent host/compiler change or an interactive native window.
4. Close Everglade's full-size runtime gate and investigate Dead Gears'
   strict numerical failure. Asset-container checks do not establish runtime
   completion. Keep the exploratory precision mask unapplied and retain the
   failed numerical result. Track Anime Lab's adoption explicitly.
5. Promote chapter 11 only when its written exits pass, then implement
   chapter 12 and subsequent chapters in order with code, teaching text and
   runnable examples. Do not relabel partial work as implemented.

## Later consumers to preserve

[#12629](https://github.com/OpenAgentsInc/openagents/issues/12629) names the
concrete Forge gap: a publication must pin its document, engine and assets
together, then prove reload and isolated forks. Use that contract in chapters
51–55 and 82–86. The current release journey does not itself solve artifact
pinning.

[#12637](https://github.com/OpenAgentsInc/openagents/issues/12637) asks for a
headless agent seat with typed observations/actions and explicit seat
admission. Main now includes the separately delivered CLI/MCP command-table
work (`167b6abe57`); inspect and reuse it when those consumers are reached.
The speech issues under [#12614](https://github.com/OpenAgentsInc/openagents/issues/12614)
guide later live voice integration. None requires expanding chapter 11 into
Forge, an agent seat or a speech model now.
