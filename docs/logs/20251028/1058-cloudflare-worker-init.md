```bash
➜  packages git:(main) bunx create-cloudflare


──────────────────────────────────────────────────────────────────────────────────────────────────────────
👋 Welcome to create-cloudflare v2.54.0!
🧡 Let's get started.
📊 Cloudflare collects telemetry about your usage of Create-Cloudflare.

Learn more at: https://github.com/cloudflare/workers-sdk/blob/main/packages/create-cloudflare/telemetry.md
──────────────────────────────────────────────────────────────────────────────────────────────────────────

╭ Create an application with Cloudflare Step 1 of 3
│
├ In which directory do you want to create your application?
│ dir ./tunnel-broker
│
├ What would you like to start with?
│ category Hello World example
│
├ Which template would you like to use?
│ type Worker only
│
├ Which language do you want to use?
│ lang TypeScript
│
├ Copying template files
│ files copied to project directory
│
├ Updating name in `package.json`
│ updated `package.json`
│
├ Installing dependencies
│ installed via `bun install`
│
├ Installing dependencies
│ installed via `bun install`
│
╰ Application created

╭ Configuring your application for Cloudflare Step 2 of 3
│
├ Installing wrangler A command line tool for building Cloudflare Workers
│ installed via `bun install wrangler --save-dev`
│
├ Retrieving current workerd compatibility date
│ compatibility date 2025-10-24
│
├ Generating types for your application
│ generated to `./worker-configuration.d.ts` via `bun run cf-typegen`
│
├ You're in an existing git repository. Do you want to use git for version control?
│ no git
│
╰ Application configured

╭ Deploy with Cloudflare Step 3 of 3
│
├ Do you want to deploy your application?
│ yes deploy via `bun run deploy`
│
├ bunx wrangler whoami
│
 ⛅️ wrangler 4.45.1
───────────────────
Getting User settings...

✘ [ERROR] Not logged in.


🪵  Logs were written to "/Users/christopherdavid/Library/Preferences/.wrangler/logs/wrangler-2025-10-28_15-56-39_178.log"

│
├ Logging into Cloudflare checking authentication status
│ not logged in
│
├ Logging into Cloudflare This will open a browser window
│ allowed via `wrangler login`
│
├ Selecting Cloudflare account retrieving accounts
│ account Arcadecd@gmail.com's Account
│
$ wrangler deploy

Cloudflare collects anonymous telemetry about your usage of Wrangler. Learn more at https://github.com/cloudflare/workers-sdk/tree/main/packages/wrangler/telemetry.md

 ⛅️ wrangler 4.45.1
───────────────────
Total Upload: 0.19 KiB / gzip: 0.16 KiB
Uploaded tunnel-broker (6.14 sec)
Deployed tunnel-broker triggers (5.05 sec)
  https://tunnel-broker.openagents.workers.dev
Current Version ID: 4c0253f7-ea60-49cc-8c8a-4397ac061211
├ Waiting for DNS to propagate. This might take a few minutes.
│ DNS propagation complete.
│
├ Waiting for deployment to become available
│ deployment is ready at: https://tunnel-broker.openagents.workers.dev
│
├ Opening browser
│
╰ Done

───────────────────────────────────────────────────────────────────────────────────
🎉  SUCCESS  Application deployed successfully!

🔍 View Project
Visit: https://tunnel-broker.openagents.workers.dev
Dash: https://dash.cloudflare.com/?to=/:account/workers/services/view/tunnel-broker

💻 Continue Developing
Change directories: cd tunnel-broker
Start dev server: bun run start
Deploy again: bun run deploy

📖 Explore Documentation
https://developers.cloudflare.com/workers

🐛 Report an Issue
https://github.com/cloudflare/workers-sdk/issues/new/choose

💬 Join our Community
https://discord.cloudflare.com
────────────────────────────────────────────
```
