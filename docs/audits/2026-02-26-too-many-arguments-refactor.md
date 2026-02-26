# Too-Many-Arguments Refactor (Issue #2312)

Date: 2026-02-26  
Scope: `crates/wgpui`

## Inventory Result

Initial suppressions found in source lanes:

- `22` total `too_many_arguments` suppressions (`expect`/`allow`)

After this refactor:

- `11` suppressions remain (all in `platform/ios.rs` plus one in `components/hud/frame.rs`)

## Refactors Landed

## 1) Markdown renderer context refactor

Replaced argument-heavy render helpers with a typed render context:

- Added `RenderSurface { text_system, scene, opacity }`
- Updated markdown render helpers to take `&mut RenderSurface` instead of repeated `(text_system, scene, opacity)` parameter triplets.
- Removed `too_many_arguments` expectations from markdown render helpers.

## 2) Live editor formatted line helper

- Removed unused `_line_height` argument from `render_formatted_line`.
- Removed corresponding `too_many_arguments` expectation.

## 3) Grid lines dashed drawing helper

- Replaced positional argument list with `DashedSegment` config struct.
- Removed corresponding `too_many_arguments` expectation.

## 4) Puffs generation helper

- Replaced long constructor argument list with `PuffsSetConfig`.
- Removed corresponding `too_many_arguments` expectation.

## Remaining Debt

- `platform/ios.rs`: retained `too_many_arguments` allowances for Objective-C/FFI callback boundaries.
- `components/hud/frame.rs`: one render helper still carries a temporary expectation and can be split in a follow-up.
