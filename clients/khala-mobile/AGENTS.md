# Khala Mobile Agent Contract

## Scope

This package is the Expo React Native destination for the Khala mobile
companion. The SwiftUI app in `clients/khala-ios/Khala` remains the interim
shipping app and native-reference source until parity is proven.

## Invariants

- Build and submit locally only: `expo prebuild`, Xcode, Gradle, and Apple
  native upload tools. Do not add EAS build, submit, or update commands.
- OTA updates go through OpenAgents Updates at `updates.openagents.com`; do not
  use Expo's hosted update service.
- Store API keys and bearer material only through the secure-store/keychain
  adapter. Do not persist secrets in SQLite, AsyncStorage, source files, or
  bundled config.
- Keep chat bodies private to authenticated sync scopes. Issue comments,
  receipts, docs, tests, and visible diagnostics may include only public-safe
  refs, counts, and route names.
- Native modules under `modules/` port proven Swift pieces through
  `expo-modules-core`; keep platform-unavailable states explicit rather than
  silently pretending support exists.
