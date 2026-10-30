# Coax introduction and readable NIP-01 — 2026-10-30

Owner request: explain Coax on its project page, link the book, enlarge
NIP-01's small text, push and deploy.

The actual Coax project now has a visible paragraph covering the language,
why one checked codebase matters, the working C11 bootstrap and the ongoing
language/library work. A 44-pixel-tall book link opens *The Coax Programming
Language*. The copy lives in `project-intro.coax`; the snapshot tool reuses
it for the HTML fallback and Markdown, so refreshing relay data preserves it.
Other projects and the demo keep their existing headers.

NIP-01 replaces its 3.6–5.5-pixel explanatory text with wrapped system-sans
body text at approximately 15 CSS pixels. Titles, stage names, controls and
IDs are larger too. Phone IDs wrap at 32 digits instead of shrinking to fit
64 on one line. Long pipeline and PARTS content scroll by wheel, touch drag
or arrow/page/home/end keys, with a visible scroll indicator and fixed
navigation. Its dedicated shader generation combines the existing splash
HUD and system-sans atlas ABI; it distinguishes glyphs from bars by their
zero versus positive half-height. The original event verifier is unchanged.

## Focused checks

- NIP-01: 47 Coax checks pass. Actual Wasm passes 82 frames, all five real
  verification outcomes, 18 control clicks, keyboard/index navigation and
  phone scrolling with unchanged guard bytes.
- The new Coax intro check passes identity scoping, non-overlapping layout
  and book navigation at 320×568, 390×844 and 1440×900.
- The actual project Wasm renders three desktop/phone layouts with finite
  geometry and intact guard bytes; peak 46,764 / 240,000 vertices.
- NIP inventory/generation checks: two tests pass. Golden routing/policy:
  ten tests, 34 assertions pass. Visual rules, shell syntax and whitespace
  checks pass.
- Muted headless WebGPU Chrome reviewed the Coax intro on phone and desktop,
  and NIP-01's desktop pipeline, phone pipeline, scrolled verdict/IDs and
  PARTS. NIP-01 desktop and the new phone lane have reviewed golden images.

The larger project interpreter suite is not all green: 43 of 61 checks pass.
An unchanged baseline from `37a100a4c8`, using the same generated inputs,
passes 42 of 60 and has the same 18 failures: 17 aggregate-storage limits
and one existing history-count assertion with the current long commit log.
No compiler limits or inherited checks were weakened. The new small check
and actual emitted-Wasm geometry checks above pass. Full unchanged crypto
mutation gates were not rerun for this presentation change.

Deployment uses the normal clean-commit staging, browser/golden and performance
gates before production promotion. Its result is recorded below after the
release completes.
