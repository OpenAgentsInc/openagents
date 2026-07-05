# Khala Mobile Templates

Small local scaffolds inspired by Ignite templates. They are intentionally
plain `.ejs` files so the app does not depend on the Ignite CLI at runtime.

Use these as copy-and-adapt starting points when adding mobile screens,
components, navigators, or UX-contract oracle tests. Keep generated code in the
React Navigation structure under `src/navigators` and `src/screens`, and keep
ordinary UI on `KhalaScreen`, `KhalaText`, and `KhalaButton`.

Current templates:

- `screen/NAME-screen.tsx.ejs`
- `component/NAME.tsx.ejs`
- `navigator/NAMENavigator.tsx.ejs`
- `ux-contract-oracle/NAME.test.ts.ejs`
