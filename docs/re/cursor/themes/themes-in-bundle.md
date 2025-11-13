# Cursor Themes in App Bundle

Source: /Applications/Cursor.app/Contents/Resources/app/extensions/theme-cursor/
- Extension: cursor-themes (version 0.0.2)
- Author: Ryo Lu <ryo@anysphere.co>
- Repo: https://github.com/ryokun6/cursor-themes
- Contributes 4 themes via package.json:
  - Cursor Dark Midnight (uiTheme: vs-dark) → themes/Cursor Dark Midnight-color-theme.json
  - Cursor Dark High Contrast (uiTheme: hc-black) → themes/Cursor Dark High Contrast-color-theme.json
  - Cursor Dark (uiTheme: vs-dark) → themes/cursor-dark-color-theme.json
  - Cursor Light (uiTheme: vs) → themes/cursor-light-color-theme.json

Below are observed highlights for each JSON theme (name, palette, counts, and notable UI colors).

## Cursor Dark Midnight High Contrast
- File: Cursor Dark High Contrast-color-theme.json
- Name: "Cursor Dark Midnight High Contrast v0.0.1"
- Type: High contrast dark (uiTheme in extension: hc-black)
- Colors count: 179; Token colors count: 250
- Palette characteristics:
  - Backgrounds: deep black/graphite (#0A0A0A, #1A1A1A)
  - Foregrounds: light text (#D8DEE9, #FFFFFF)
  - Accents (Nord-like): blue #88C0D0, green #A3BE8C, red #BF616A, yellow #EBCB8B
- Notable UI colors:
  - editor.background: #0A0A0A; editor.foreground: #D8DEE9
  - activityBar/sideBar/statusBar/titleBar background: #0A0A0A
  - tab.activeBackground: #0A0A0A; tab.inactiveBackground: #0A0A0A
  - list.activeSelectionBackground: #434C5E; list.hoverBackground: #2A2A2A99
  - button.background: #434C5E; button.foreground: #ECEFF4
  - terminal.background: #0A0A0A; terminal.foreground: #FFFFFF

## Cursor Dark Midnight
- File: Cursor Dark Midnight-color-theme.json
- Name: "Cursor Dark Midnight v0.0.1"
- Type: Dark (uiTheme: vs-dark)
- Colors count: 238; Token colors count: 132
- Palette characteristics:
  - Backgrounds: midnight slate (#191c22, #1e2127)
  - Foregrounds: soft gray-blue text (#7b88a1)
  - Accents: Arctic blue (#88c0d0), with nord‑style green/yellow/red present in diff and gutters
- Notable UI colors:
  - editor.background: #1e2127; editor.foreground: #7b88a1
  - activityBar/sideBar/statusBar/titleBar background: #191c22
  - tab.activeBackground: #1e2127; tab.inactiveBackground: #191c22
  - list.activeSelectionBackground: #21242b; list.hoverBackground: #272c3699
  - button.background: #88c0d0; button.foreground: #191c22
  - terminal.background: #191c22; terminal.foreground: #d8dee9

## Cursor Dark (Anysphere)
- File: cursor-dark-color-theme.json
- Name: "Cursor Dark Anysphere v0.0.3"
- Type: Dark (uiTheme: vs-dark)
- Colors count: 250; Token colors count: 225
- Palette characteristics:
  - Backgrounds: charcoal (#141414, editor #181818)
  - Foregrounds: near‑white with slight alpha (#E4E4E4EB)
  - Accents: soft blue (#81A1C1)
  - Selections are subtle/translucent (e.g., list.hoverBackground #E4E4E411)
- Notable UI colors:
  - editor.background: #181818; editor.foreground: #E4E4E4EB
  - activityBar/sideBar/statusBar/titleBar background: #141414
  - tab.activeBackground: #181818; tab.inactiveBackground: #141414
  - list.activeSelectionBackground: #E4E4E41E; list.hoverBackground: #E4E4E411
  - button.background: #81A1C1; button.foreground: #191c22
  - terminal.background: #141414; terminal.foreground: #E4E4E4EB
- Sample token colors (from rules):
  - Strings: #e394dc
  - Rust Standard Function: #aaa0fa
  - Haskell Generic Type Variable: #82D2CE

## Cursor Light
- File: cursor-light-color-theme.json
- Name: "Cursor Light v0.0.2"
- Type: Light (uiTheme: vs)
- Colors count: 289; Token colors count: 129
- Palette characteristics:
  - Backgrounds: near‑white surfaces (#FCFCFC, panels #F3F3F3)
  - Foregrounds: near‑black with slight alpha (#141414EB)
  - Accents: blue (#3C7CAB) for buttons; complementary blues for tokens
- Notable UI colors:
  - editor.background: #FCFCFC; editor.foreground: #141414EB
  - activityBar/sideBar/statusBar/titleBar background: #F3F3F3
  - tab.activeBackground: #FCFCFC; tab.inactiveBackground: #F3F3F3
  - list.activeSelectionBackground: #14141411; list.hoverBackground: #14141411
  - button.background: #3C7CAB; button.foreground: #FCFCFC
  - terminal.background: #F3F3F3; terminal.foreground: #141414EB
- Sample token colors:
  - Comments: #141414AD
  - Strings: #9E94D5
  - Variables in Strings: #206595

## Notes
- The Midnight variants (including High Contrast) use a Nord‑like accent palette: blue (#88C0D0), green (#A3BE8C), yellow (#EBCB8B), red (#BF616A).
- High Contrast variant drives most surfaces to true black with stronger whites and thicker contrasts, while preserving the same accent palette.
- The extension declares the theme type (vs, vs-dark, hc-black) in its package.json; the theme JSON files themselves do not include a `type` field.
- Token color sets vary by theme; counts above summarize their size without listing every rule.

