# Khala Mobile Templates

Small local scaffolds inspired by Ignite templates. They are intentionally
plain `.ejs` files so the app does not depend on the Ignite CLI at runtime.

Use `bun run generate <screen|component|api-core> <Name>` when adding mobile
surfaces. Generated screens are expected to keep their whole bundle: screen,
four-state mount test, stories, pending contract stub, Maestro flow stub, and
visual-baseline registration. The QAM-3 conformance test fails the release gate
when a generated screen is missing bundle members.

Current templates:

- `screen/NAME-screen.tsx.ejs`
- `screen-mount-test/NAME-screen.test.tsx.ejs`
- `screen-stories/NAME-screen.stories.tsx.ejs`
- `screen-contract/NAME-contract.test.ts.ejs`
- `screen-maestro/NAME-screen.yaml.ejs`
- `screen-visual/NAME-screen.ts.ejs`
- `component/NAME.tsx.ejs`
- `component-test/NAME.test.tsx.ejs`
- `component-stories/NAME.stories.tsx.ejs`
- `api-core/NAME-core.ts.ejs`
- `api-core-test/NAME-api-core.test.ts.ejs`
- `navigator/NAMENavigator.tsx.ejs`
- `ux-contract-oracle/NAME.test.ts.ejs`
