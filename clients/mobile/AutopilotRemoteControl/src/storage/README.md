# Token Storage

`token-store.ts` defines the pure TypeScript `TokenStore` contract and an
in-memory implementation for tests and local development wiring.

Remaining operator step: add a `SecureStoreTokenStore` adapter backed by
`expo-secure-store`, installed with `npx expo install expo-secure-store`, and
use that adapter in release and dev-client builds.
