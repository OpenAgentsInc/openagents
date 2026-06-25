# Agent Development Notes

This is a Foldkit app, a framework built on Effect-TS, architected like Elm.

<!-- effect-solutions:start -->

## Effect Best Practices

**IMPORTANT:** Always consult effect-solutions before writing Effect code.

1. Run `effect-solutions list` to see available guides
2. Run `effect-solutions show <topic>...` for relevant patterns (supports multiple topics)
3. Search `~/.local/share/effect-solutions/effect` for real implementations

Topics: quick-start, project-setup, tsconfig, basics, services-and-layers, data-modeling, error-handling, config, testing, cli.

Never guess at Effect patterns - check the guide first.

<!-- effect-solutions:end -->

## Local Effect Source

The Effect v4 repository is cloned to `~/.local/share/effect-solutions/effect` for reference.
Use this to explore APIs, find usage examples, and understand implementation
details when the documentation isn't enough.

## Getting the canonical references

The live Foldkit code is the canonical reference for everything: which APIs exist, what idiomatic apps look like, what the current conventions are. Written docs (including this one) can drift; the code can't.

Do not vendor Foldkit source into this repo. In particular, do not create or
commit `repos/foldkit`. Use external/local references instead:

- `../projects/repos/foldkit/examples/`: runnable example apps spanning every complexity tier. Usually your first stop when looking for a precedent for how to write something.
- `../projects/repos/foldkit/packages/foldkit/src/`: framework source. Ground truth for API signatures.
- `../projects/repos/foldkit/packages/typing-game/client/src/` and `../projects/repos/foldkit/packages/website/src/`: production apps built with Foldkit. Highest-fidelity reference for application architecture, Submodels, and OutMessage.

Imports must come from the `foldkit` npm package. If the external Foldkit
reference is missing, use the workspace-level `projects/sync.sh` from
`/Users/christopherdavid/work` or inspect the installed package under
`node_modules/foldkit`; do not add a subtree here.

subtree_prompted: disabled

If `foldkit-skills` is installed as a Claude Code plugin, the `generate-program` and `audit-program` skills carry snapshot architecture and conventions guides synced from the live code.

## Project Conventions

- Read `DESIGN.md` before changing UI. OpenAgents Autopilot uses the Vortex dark,
  mono, operational design direction; do not introduce light mode, decorative
  gradients, card-heavy marketing shells, or generic chat-first framing.
- When writing GitHub issue comments with `gh`, never pass Markdown containing
  escaped `\n` sequences in a quoted `--body` string. Use real newlines via
  `--body-file -`, a heredoc/stdin body, or a body variable populated from a
  heredoc. For important issue comments, verify the rendered body with
  `gh api repos/<owner>/<repo>/issues/comments/<comment-id> --jq .body` before
  moving on.
- Before editing a UI surface for a named URL, trace the actual rendered
  entrypoint from routing/startup through the top-level view and verify which
  component produces the live DOM. Do not assume that a same-named file is the
  route owner. For example, live `/login` is rendered by `apps/web/src/view.ts`
  via the GitHub OAuth login button, while
  `apps/web/src/page/loggedOut/page/login.ts` is a legacy/local auth view and
  is not the production `/login` surface. When changing deployed UI, verify
  against the served bundle or rendered DOM before calling the work complete.
- When the user asks to deploy this repo, the ONLY sanctioned path is
  `bun run --cwd workers/api deploy:safe` (see the Worker deploy safety gate in
  `docs/DEPLOYMENT.md`). It always, in order: checks local==origin/main, runs
  `check:deploy` (typecheck:web/api + the real web/worker test suites + guards,
  with NO dependency on the flaky `verse-launch-smoke`), applies remote D1
  migrations, runs `check:pending-migrations` (fails if ANY migration is still
  pending), builds web assets, then uploads the worker LAST. **Raw
  `bunx wrangler deploy` / `npx wrangler deploy` is FORBIDDEN as a deploy path:**
  it skips migrations and shipped the worker ahead of its schema in the
  2026-06-25 gateway-wide 500 outage
  (`docs/incidents/2026-06-25-khala-500-completions-outage-aar.md`). Migrations
  are always applied before the worker is uploaded. Never report deployment
  success until live smoke checks prove both the document and JS asset are
  reachable:
  `curl -fsSI https://openagents.com/` and the concrete
  `/assets/index-*.js` URL referenced by the served HTML must both return 200.
- Before changing Worker route/service boundaries, sync/runtime/config code,
  provider-account or GitHub-write error handling, logged-in update routing,
  login/root route behavior, UI family modules, or other zero-tech-debt
  cleanup surfaces, read
  `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`. Treat that
  document as a forward contract, not just a historical audit: do not increase
  compatibility budgets, add `Effect.runPromise` bridges, add Promise
  dependency adapters, reintroduce deleted aliases/files, or bypass typed
  service/layer boundaries unless the same change adds caller evidence,
  deletion conditions, and an architecture guardrail.
- User-facing product copy must not explain implementation mechanics,
  internal dispatch paths, grant plumbing, auth materialization, sync internals,
  SHC handoff details, or "how it works" process notes. Put that material in
  docs, logs, metadata dialogs, runbooks, or operator-only surfaces. Product UI
  should use short labels, values, statuses, and direct actions such as
  "Connect", "Reconnect", "Open Settings", or "Check status".
- Product UI icons must come from the generated Fireball Apps SDK icon catalog
  in `apps/web/src/icon.ts`. Type icon props as `IconName` and render through
  `iconView` or `IconService`; do not add ad hoc SVG paths, Unicode/text icon
  stand-ins, image icon URLs, icon fonts, lucide/react-icons, Iconify, or new
  icon dependencies. If a needed icon is missing, update the upstream Fireball
  catalog first, then run `bun run sync:icons` and keep `apps/web/src/icon*.test.ts`
  passing.
- Training and proof replay visual surfaces must use the existing
  `@openagentsinc/three-effect` renderer vocabulary and the visual taxonomy
  documented in `docs/launch/2026-06-17-tassadar-training-run-visual-language.md`
  plus the `/animations` studies. `@openagentsinc/proof-replay` owns replay
  bundle shape, clocks, source gates, and render plans only; it is not a visual
  renderer. Browser or desktop app code may adapt public replay bundles into
  `three-effect` options and may render Foldkit controls, inspectors, transcript
  mirrors, and accessibility fallbacks. Do not add new app-local DOM/CSS/canvas
  stages, actor/avatar renderers, payment-zap effects, camera grammar, particles,
  or other proof replay visuals. Add missing primitives to
  `/Users/christopherdavid/work/three-effect` first, then consume the package.
  The current `apps/web/src/scene/tassadarProofReplayElement.ts` is a temporary
  legacy bridge for the first replay route; do not extend its visual language
  except to replace it with `three-effect` mounts.
- Use Tailwind utility classes and the local Foldkit UI registry as the default
  styling path. For Worker-rendered HTML, put Tailwind classes in the template
  and make sure `apps/web/src/styles.css` sources the Worker templates so the
  classes compile into `/assets/openagents.css`.
- Vanilla CSS is a last resort. Use it only for global base/reset rules,
  platform quirks, third-party integration constraints, or selectors that
  cannot be expressed clearly with Tailwind. Do not add one-off vanilla CSS
  blocks for product surfaces when Tailwind utilities or UI registry primitives
  can express the same layout.
- Foldkit is tightly coupled to the Effect ecosystem. Do not suggest solutions outside of Effect-TS.
- Model fields must be Schema types (the model is a schema). Plain TypeScript types are fine elsewhere (function return types, local variables, etc.).
- Use full names like `Message` (not `Msg`), and `withReturnType` (not `as const` or type casting).
- Use `m()` for message schemas, `ts()` for tagged structs (model states, field validation), and `r()` for route schemas.
- Push back on any direction that violates Elm Architecture principles: unidirectional data flow, messages as facts (not commands), model as single source of truth, side effects confined to commands. If a prompt suggests mutating state, imperative event handlers, or two-way bindings, flag the issue and propose the idiomatic Foldkit approach.
- Never use `NoOp`. Every message must describe what happened. Fire-and-forget commands use `Completed*` messages mirroring the Command name verb-first: `LockScroll` → `CompletedLockScroll`.

## Pylon Presence Auth Contract (token-only)

Pylon presence and lifecycle writes (`POST /api/pylons/:ref/heartbeat`,
`/wallet-readiness`, `/payout-target-admission`, `/assignments/*`, and
`/register`) are authenticated with an OpenAgents agent **bearer token**
(`Authorization: Bearer <agent token>`) in `workers/api/src/pylon-api-routes.ts`
(`requireAgent`). This is intentional and is the documented contract.

A node's self-held Nostr key (NIP-98) is **not** accepted as presence
authority. Registrations are bound to `ownerAgentUserId` derived from the
bearer-token session; the registry does not bind a verified Nostr pubkey to
that owner (`providerNostrPubkey` is optional discovery metadata, only set for
NIP-90 provider Pylons, and is never verified server-side). Accepting a
self-signed heartbeat would require server-side NIP-98 schnorr verification
plus a mandatory pubkey→owner binding — a broader auth change that must not be
made implicitly.

When a presence request arrives with a `Nostr`/NIP-98 `Authorization` scheme
and no usable bearer token, the route returns a typed, explanatory `401`
(`error: pylon_api_presence_requires_agent_token`, `WWW-Authenticate: Bearer`)
naming the token-only contract and pointing the node at the bearer path,
rather than a bare `unauthorized` (#5058). Do not silently fall back to a bare
401 for the Nostr-signed presence path. If self-signed presence is ever to be
supported, design the NIP-98 verification + pubkey binding deliberately with a
new dated review and tests; do not broaden any other authority (spend,
settlement, moderation) in the process. Regression coverage lives in
`workers/api/src/pylon-api-routes.test.ts`.

## Foldkit Patterns

### Update

`init` and `update` both return `[Model, ReadonlyArray<Command<Message>>]`:

```ts
type UpdateReturn = readonly [Model, ReadonlyArray<Command<Message>>]
const withUpdateReturn = M.withReturnType<UpdateReturn>()

const update = (model: Model, message: Message): UpdateReturn =>
  M.value(message).pipe(
    withUpdateReturn,
    M.tagsExhaustive({
      ClickedIncrement: () => [evo(model, { count: count => count + 1 }), []],
    }),
  )
```

Use `evo()` from `foldkit/struct` for immutable model updates. Never spread or `Object.assign`.

### View

Bind the `html` factory inside each view function with `const h = html<Message>()` (never at module level), then reach for `h.div`, `h.OnClick`, etc. off the returned record. Use `empty` (not `null`) for conditional rendering, `M.value().pipe(M.tagsExhaustive({...}))` for discriminated unions, and `Array.match` for lists that may be empty.

Use `keyed` wrappers whenever the view branches into structurally different layouts based on route or model state. Without keying, the virtual DOM tries to diff one layout into another, causing stale DOM and event handler mismatches.

### Commands

Define a Command with `Command.define`, which is curried: the first call binds the name (and optionally args + result Message schemas), and the second call binds the Effect. Assign definitions to PascalCase constants. Never inline in pipe chains. Commands catch all errors via `Effect.catch(() => Effect.succeed(FailedX(...)))` so side effects never crash the app. Definitions live colocated with the update function that returns them.

For the with-args shape, see `../projects/repos/foldkit/examples/weather/src/main.ts` or `../projects/repos/foldkit/examples/kanban/src/command.ts`. For an argless DOM-side-effect Command, the argless form in `kanban/src/command.ts` (`FocusAddCardInput`) is the canonical reference.

For DOM operations (focus, scroll, modals, scroll lock), Foldkit ships a `Dom` module. For time, randomness, UUIDs, and delays, use Effect's built-ins directly (`Clock`, `Random`, `Effect.uuid`, `Effect.sleep`). Don't reach for raw `document.querySelector`, `setTimeout`, `Date.now()`, or `Math.random()`.

### File Organization

This repo is a Bun workspace:

- `apps/web/` is the Foldkit/Vite browser app.
- `workers/api/` is the Cloudflare Worker API and Durable Object surface.
- `packages/sync-schema/` holds Effect Schema protocol models shared by browser and Worker code.
- `packages/sync-client/` holds browser-side sync helpers.
- `packages/sync-worker/` holds Worker-side sync helpers.

Keep Cloudflare runtime code out of `apps/web/`. Keep browser/Foldkit code out
of `workers/api/`. Shared code belongs in `packages/*` and must stay Effect
Schema-first at every external boundary.

A Foldkit app lives in two files. `src/main.ts` holds the pure definitions (Model, Messages, init, update, view). `src/entry.ts` imports them and boots the runtime with `Runtime.makeProgram` and `Runtime.run`. `index.html` references `entry.ts`. The split keeps `main.ts` importable from tests without booting a runtime as a side effect. Never call `Runtime.run` from `main.ts`.

Use uppercase section headers (`// MODEL`, `// MESSAGE`, `// INIT`, `// UPDATE`, `// COMMAND`, `// VIEW`) for wayfinding.

### Testing

Test update functions with `foldkit/test`. Since update is pure, tests run without a runtime, DOM, or side effects. Use `Story.story` for update-level tests (send Messages, assert on Model and Commands) and `Scene.scene` for feature-level testing through the view with accessible locators. Study the `.story.test.ts` and `.scene.test.ts` files in `../projects/repos/foldkit/examples/` when the external Foldkit reference exists.

## Code Style

- Encode state in discriminated unions, not booleans or nullable fields. `Idle | Loading | Error | Ok`, not `isLoading: boolean`. Make impossible states unrepresentable.
- Use `Option` instead of `null` or `undefined`. Prefix Option-typed values with `maybe*`. Match with `Option.match`; don't unwrap with `Option.map(...)` + `Option.getOrElse(...)` when you can just match.
- Use Effect modules over native methods in `pipe` chains (`Array.map`, `String.startsWith`, `Array.findFirst`). Native methods are fine when calling directly on a named variable.
- Never cast Schema values with `as Type`. Use the callable constructor: `SucceededLogin({ sessionId })`, not `{ _tag: 'SucceededLogin', sessionId } as Message`.
- Always `Array.isEmptyArray` / `Array.isNonEmptyArray` (not `.length === 0`). Use `Array.match` when handling both empty and non-empty cases.
- Never use `for` loops or `let` for iteration. Reach for `Array.map`, `Array.filterMap`, `Array.makeBy`, `Array.reduce`.
- Never use `T[]`. Always `Array<T>` or `ReadonlyArray<T>`.
- Always use `Effect.Match`, never `switch`.
- Always use braces for control flow: `if (foo) { return true }`.
- Don't add inline comments to explain code. Use better names instead. Reserve `// NOTE:` for behavior that would mislead a careful reader.

## Message Layout

Group all `m()` declarations together with no blank lines between them, then put `S.Union([...])` and `type Message = typeof Message.Type` on adjacent lines:

```ts
const ClickedSubmit = m('ClickedSubmit')
const UpdatedEmail = m('UpdatedEmail', { value: S.String })

const Message = S.Union([ClickedSubmit, UpdatedEmail])
type Message = typeof Message.Type
```

Messages are verb-first past-tense. Common prefixes: `Clicked*`, `Updated*` (input changes and external state updates), `Submitted*`, `Pressed*`, `Selected*`, `Succeeded*` / `Failed*` (paired async results), `Completed*` (fire-and-forget), `Got*` (child OutMessage in the Submodel pattern).

## Debugging

This project ships with `@foldkit/devtools-mcp` pre-wired. When the dev server is running and the app is open in a browser, `foldkit_*` MCP tools let you inspect Model, Message history, and time-travel. Reach for them before adding `console.log` whenever the question is about state or Message flow.

## Going Deeper

For Submodels and OutMessage, Subscriptions, Mount / ManagedResource / CustomElement, field validation, routing, accessibility, and the full convention set, read the live Foldkit code in `../projects/repos/foldkit/`. The `examples/` directory and the production apps (`packages/typing-game/`, `packages/website/`) are the highest-fidelity references for any specific pattern. The `foldkit-skills` plugin's `generate-program` and `audit-program` skills carry written snapshot guides if you want a structured walkthrough.
