#+ Internationalization (Languages & Locales)

Build multilingual experiences that match the user’s locale. Foundation Models supports the same locales as Apple Intelligence (check at runtime).

## Query Supported Locales

```swift
let supported = SystemLanguageModel.default.supportedLanguages // [Locale.Language]
```

Display human‑readable names using `Locale.current.localizedString(forLanguageCode:)` and `forRegionCode:`. Avoid hard‑coding lists; they change across releases.

## Conversation Language

- Use persistent sessions to maintain the chosen language across turns
- For auto‑detect flows: start in user’s UI locale; if content suggests another language, explicitly set response language in instructions

## Multilingual Scenarios

- Fresh session problem: language can reset after session creation → set/retain instruction snippets indicating desired output locale
- Persistent solution: keep one session per conversation; store selected language in your view model and rebuild instructions on restore

## OpenAgents Guidance

- Add a language picker that binds to `GenerationOptions`/instructions for output language
- Log detected/forced languages for diagnostics; keep a simple mapping strategy for variants (e.g., Spanish ES vs 419)
- Ensure summaries/titles generated on device follow the app’s primary locale unless the user opted otherwise

