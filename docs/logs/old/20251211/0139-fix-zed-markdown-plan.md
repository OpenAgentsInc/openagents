# Plan: Get Zed Markdown Rendering Working in MechaCoder

## Problem Summary

MechaCoder crashes when using Zed's markdown crate because:
1. `SettingsStore::new()` auto-registers ALL settings via inventory
2. `AllLanguageSettings` (via `#[derive(RegisterSetting)]`) gets loaded
3. `AllLanguageSettings::from_settings()` calls `.unwrap()` on 40+ fields
4. Our `assets/settings/default.json` is incomplete - missing most required fields

## Key Finding

The markdown crate ONLY needs `cx.theme()` (5 call sites for colors). It does NOT need language settings. But the settings system loads everything at once.

## Solution: Complete the default.json

The simplest fix is to add ALL required fields to `assets/settings/default.json`. This is better than:
- Using test-support feature (which also needs complete JSON)
- Patching inventory (breaks other things)
- Avoiding markdown crate (loses features)

## Required Fields in default.json

Based on the exploration, these fields are REQUIRED (will crash if missing):

### Theme Settings (already have most)
- `theme`, `icon_theme`, `unnecessary_code_fade` ✓
- `ui_font_*`, `buffer_font_*` ✓
- `base_keymap` ✓

### Language Settings (MISSING - add to root level)
```json
{
  "features": {
    "edit_prediction_provider": "none"
  },
  "edit_predictions": {
    "disabled_globs": [],
    "mode": "eager",
    "copilot": {},
    "codestral": {},
    "enabled_in_text_threads": false
  },
  "tab_size": 4,
  "hard_tabs": false,
  "soft_wrap": "none",
  "preferred_line_length": 80,
  "show_wrap_guides": true,
  "wrap_guides": [],
  "indent_guides": {
    "enabled": true,
    "line_width": 1,
    "active_line_width": 1,
    "coloring": "fixed",
    "background_coloring": "disabled"
  },
  "format_on_save": "on",
  "remove_trailing_whitespace_on_save": true,
  "ensure_final_newline_on_save": true,
  "formatter": "auto",
  "prettier": { "allowed": false },
  "jsx_tag_auto_close": { "enabled": true },
  "enable_language_server": true,
  "language_servers": ["..."],
  "allow_rewrap": "in_comments",
  "show_edit_predictions": true,
  "edit_predictions_disabled_in": [],
  "show_whitespaces": "none",
  "whitespace_map": { "space": "•", "tab": "→" },
  "extend_comment_on_newline": true,
  "inlay_hints": {
    "enabled": false,
    "show_value_hints": true,
    "show_type_hints": true,
    "show_parameter_hints": true,
    "show_other_hints": true,
    "show_background": false,
    "edit_debounce_ms": 700,
    "scroll_debounce_ms": 50
  },
  "use_autoclose": true,
  "use_auto_surround": true,
  "always_treat_brackets_as_autoclosed": false,
  "use_on_type_format": true,
  "code_actions_on_format": {},
  "linked_edits": true,
  "auto_indent": true,
  "auto_indent_on_paste": true,
  "tasks": { "variables": {}, "enabled": true, "prefer_lsp": false },
  "show_completions_on_input": true,
  "show_completion_documentation": true,
  "completions": {
    "words": "fallback",
    "words_min_length": 3,
    "lsp": true,
    "lsp_fetch_timeout_ms": 0,
    "lsp_insert_mode": "replace_suffix"
  },
  "debuggers": [],
  "word_diff_enabled": true,
  "colorize_brackets": false,
  "languages": {}
}
```

## Implementation Steps

1. **Undo the simplification** - Restore message_view.rs, thread_view.rs, lib.rs, main.rs, Cargo.toml to use markdown

2. **Update assets/settings/default.json** - Add all required language settings fields listed above

3. **Update init_theme()** in lib.rs:
   ```rust
   pub fn init_theme(cx: &mut App) {
       // Initialize settings from complete default.json
       settings::init(cx);

       // Register ThemeSettings
       theme::ThemeSettings::register(cx);

       // Set GlobalTheme
       let theme_family = theme::zed_default_themes();
       let theme = theme_family.themes.into_iter().next().unwrap();
       let icon_theme = theme::default_icon_theme();
       cx.set_global(theme::GlobalTheme::new(Arc::new(theme), icon_theme));
   }
   ```

4. **Test** - Run MechaCoder and verify markdown renders without crashes

## Files to Modify

1. `assets/settings/default.json` - Add complete language settings
2. `crates/mechacoder/src/lib.rs` - Restore init_theme with settings::init()
3. `crates/mechacoder/src/main.rs` - Restore init_theme() call
4. `crates/mechacoder/Cargo.toml` - Restore markdown, language, theme, ui, settings deps
5. `crates/mechacoder/src/ui/message_view.rs` - Restore markdown-based MessageView
6. `crates/mechacoder/src/ui/thread_view.rs` - Restore Entity<MessageView> caching

## Risk Mitigation

- If new required fields are added to Zed's settings, our default.json will need updating
- We should add a comment in default.json noting it must stay in sync with Zed's requirements
