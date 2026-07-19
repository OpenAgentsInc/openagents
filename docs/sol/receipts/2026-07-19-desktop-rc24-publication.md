# OpenAgents Desktop rc.24 publication receipt

- version: `0.1.0-rc.24`
- tag: `openagents-desktop-v0.1.0-rc.24`
- release: https://github.com/OpenAgentsInc/openagents/releases/tag/openagents-desktop-v0.1.0-rc.24
- published: 2026-07-19
- source revision: `915932bf4b6eb9f9fa33f131b7d3f9adc1306d6e`
- publication class: `desktop_experimental_prerelease`
- authority: `AUTHORITY.md` revision 2; `program.full_auto_release`; `grant.autonomous_rc_release_and_communication`
- trigger: owner direction to publish a tested basic-IDE RC from `main`

## Published artifact

| Target | Artifact | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| darwin-arm64 | `OpenAgents-0.1.0-rc.24-rc-darwin-arm64-unsigned.zip` | 206178717 | `b615434ae7f75be47d5c26300e9251d12d5b346ee732a441250a903c40d1a6e5` |

GitHub reported the same byte length and `sha256:` digest after upload. The
release is public, marked prerelease, immutable at the tag above, and targets
the exact source revision recorded in this receipt.

## Verification evidence

- IDE-01 through IDE-05 focused gates: 55 test files and 558 assertions passed.
- Complete Desktop gate: TypeScript passed; 270 test files passed; 2,645 tests passed and 39 were intentionally skipped; production build passed.
- Compatibility Electron smoke passed through renderer reload and lifecycle teardown.
- Default React Electron smoke passed Files open and close through Command-E, Pierre expansion, Monaco document open, a streamed Codex fixture turn, immediate Full Auto start and stop, navigation, and renderer reload.
- Packaged IDE-02 journey passed against a disposable corpus with more than 5,000 entries, including pointer activation, Home and End keyboard traversal, keyboard context menu, screen-reader labels, and workspace-root withholding.
- Packaged IDE-03 and IDE-04 journey passed against the exact app bundle, including Finder-style document open, Monaco readiness and edit, Tokyo Night resources, Vim toggle, split views, quick open, preview-to-pin, draft recovery, private-scheme loading, root withholding, and resource teardown.
- The GitHub publication adapter re-read the remote release and verified its asset digest and byte length before making the prerelease public.

## Honest limitations

The production macOS maker was invoked first and refused before creating maker
artifacts because the Developer ID identity and notarization credentials were
not available to this machine. The published fallback is therefore explicitly
unsigned and not notarized, Apple Silicon only, and experimental. It is not a
stable release, is not promoted to the signed Desktop update feed, and makes no
Windows, Linux, or Intel Mac support claim.

The basic IDE does not yet include LSP and Problems integration, an integrated
terminal screen, agent proposal and apply workflows, or inline AI editing.
