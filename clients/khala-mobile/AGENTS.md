# Khala Mobile Agent Contract

> **Deprecated/frozen source.** The destination is the from-scratch
> `apps/openagents-mobile` app under #8597, not this package. Make no new product
> features, UI, branding, or releases here. Changes are limited to critical
> security fixes, parity evidence, contract/native-module extraction, and typed
> migration support. Never make the greenfield app import this package.

## Scope

This package is the deprecated Expo React Native Khala mobile implementation.
The SwiftUI app in `clients/khala-ios/Khala` is deprecated too. Both are
reference sources until the new OpenAgents app proves parity and cutover.

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
