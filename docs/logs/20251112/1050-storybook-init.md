```bash
➜  tauri git:(main) npm create storybook@latest
Need to install the following packages:
create-storybook@10.0.7
Ok to proceed? (y) y


> openagents@0.1.0 npx
> create-storybook

info Installing dependencies...
info

removed 305 packages, changed 13 packages, and audited 304 packages in 3s

106 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
info Dependencies installed
╭───────────────────────────────────────────────────────╮
│                                                       │
│   Adding Storybook version 10.0.7 to your project..   │
│                                                       │
╰───────────────────────────────────────────────────────╯
✔ New to Storybook? › Yes: Help me with onboarding
Attention: Storybook now collects completely anonymous telemetry regarding usage. This information is used to shape Storybook's roadmap and prioritize features.
You can learn more, including how to opt-out if you'd not like to participate in this anonymous program, by visiting the following URL:
https://storybook.js.org/telemetry

 • Detecting project type. ✓
 • Adding Storybook support to your "React" app • Detected Vite project. Setting builder to Vite. ✓

  ✔ Getting the correct version of 5 packages

added 121 packages, and audited 425 packages in 7s

140 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
  ✔ Installing Storybook dependencies
. ✓
info Installing dependencies...
info

up to date, audited 425 packages in 598ms

140 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
info Dependencies installed
> npx storybook@10.0.7 add --yes @storybook/addon-a11y@10.0.7
npm warn deprecated inflight@1.0.6: This module is not supported, and leaks memory. Do not use it. Check out lru-cache if you want a good and tested way to coalesce async requests by a key value, which is much more comprehensive and powerful.
npm warn deprecated glob@7.2.3: Glob versions prior to v9 are no longer supported
npm warn deprecated rimraf@2.6.3: Rimraf versions prior to v4 are no longer supported
Verifying @storybook/addon-a11y
Installing @storybook/addon-a11y@^10.0.7

added 2 packages, and audited 427 packages in 2s

141 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
Adding '@storybook/addon-a11y@10.0.7' to the "addons" field in .storybook/main.ts
Running postinstall script for @storybook/addon-a11y
> npx storybook@10.0.7 add --yes @storybook/addon-vitest@10.0.7
Verifying @storybook/addon-vitest
Installing @storybook/addon-vitest@^10.0.7

added 4 packages, and audited 431 packages in 4s

142 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
Adding '@storybook/addon-vitest@10.0.7' to the "addons" field in .storybook/main.ts
Running postinstall script for @storybook/addon-vitest

╭ 👋 Howdy! ─────────────────────────────────────────────────────────────────╮
│                                                                            │
│   I'm the installation helper for @storybook/addon-vitest                  │
│                                                                            │
│   Hold on for a moment while I look at your project and get it set up...   │
│                                                                            │
╰────────────────────────────────────────────────────────────────────────────╯

╭ 🙈 Let me cover this for you ──────────────────────────────────────────────────────────────────────────────────────────────╮
│                                                                                                                            │
│   You don't seem to have a coverage reporter installed. Vitest needs either V8 or Istanbul to generate coverage reports.   │
│                                                                                                                            │
│   Adding "@vitest/coverage-v8" to enable coverage reporting.                                                               │
│   Read more about Vitest coverage providers at https://vitest.dev/guide/coverage.html#coverage-providers                   │
│                                                                                                                            │
╰────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

› Installing dependencies:
  vitest, playwright, @vitest/browser-playwright, @vitest/coverage-v8
info Installing dependencies...
info

added 63 packages, and audited 494 packages in 6s

154 packages are looking for funding
  run `npm fund` for details

found 0 vulnerabilities
info Dependencies installed

› Configuring Playwright with Chromium (this might take some time):
  npx playwright install chromium --with-deps

› Creating a Vitest setup file for Storybook:
  /Users/christopherdavid/code/openagents/tauri/.storybook/vitest.setup.ts

╭ 🚨 Oh no! ─────────────────────────────────────────────────────────────────────────────╮
│                                                                                        │
│   We were unable to update your existing Vite config file.                             │
│                                                                                        │
│   Please refer to the documentation to complete the setup manually:                    │
│   https://storybook.js.org/docs/writing-tests/integrations/vitest-addon#manual-setup   │
│                                                                                        │
╰────────────────────────────────────────────────────────────────────────────────────────╯
› Setting up @storybook/addon-a11y for @storybook/addon-vitest:
│
◆  Ran addon-a11y-addon-test migration
│
◆  Dependencies installed

╭ ⚠️ Done, but with errors! ───────────────────────────────────────────────────────────────────────────────────╮
│                                                                                                              │
│   @storybook/addon-vitest was installed successfully, but there were some errors during the setup process.   │
│                                                                                                              │
│   Please refer to the documentation to complete the setup manually and check the errors above:               │
│   https://storybook.js.org/docs/next/writing-tests/integrations/vitest-addon#manual-setup                    │
│                                                                                                              │
╰──────────────────────────────────────────────────────────────────────────────────────────────────────────────╯

╭──────────────────────────────────────────────────────────────────────────────╮
│                                                                              │
│   Storybook was successfully installed in your project! 🎉                   │
│   Additional features: docs, test, onboarding                                │
│                                                                              │
│   To run Storybook manually, run npm run storybook. CTRL+C to stop.          │
│                                                                              │
│   Wanna know more about Storybook? Check out https://storybook.js.org/       │
│   Having trouble or want to chat? Join us at https://discord.gg/storybook/   │
│                                                                              │
╰──────────────────────────────────────────────────────────────────────────────╯

Running Storybook

> openagents@0.1.0 storybook
> storybook dev -p 6006 --initial-path=/onboarding --quiet

storybook v10.0.7

info => Serving static files from /Users/christopherdavid/code/openagents/tauri/node_modules/@chromatic-com/storybook/assets at /addon-visual-tests-assets
info Using tsconfig paths for react-docgen
```
