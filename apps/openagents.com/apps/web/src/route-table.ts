// ---------------------------------------------------------------------------
// THE single route table — one file, the source of truth for every route's
// path, public/authed visibility, and serving surface (#6222).
// ---------------------------------------------------------------------------
//
// Historically a route had to be declared in FOUR disconnected places that
// silently desynced:
//   1. the client Foldkit parser/registry (`route.ts`),
//   2. the route-coverage parse guard (`route-coverage.test.ts`),
//   3. the per-route auth/nav policy (`product-policy.ts`),
//   4. the SERVER document allowlist `knownDocumentPathPatterns`
//      (`workers/api/src/worker-routes.ts`).
//
// (4) was the worst desync: a client route missing from the server allowlist
// 302-redirects to "/" in production even though the page exists — the real
// `/trace/{uuid}` prod bug on 2026-06-24 (server didn't list `/trace`, so every
// shared trace link bounced home). `#6218` (gym gate) was the same mechanism
// patched by hand for one path.
//
// This file is now the ONE declarative table. Each entry declares, for one
// `AppRoute` tag:
//   - `surface`            : how the route is served (see `RouteSurface`),
//   - `serverDocument`     : the canonical server-side document path matcher
//                            (null when the route is NOT a server-served SPA
//                            document — e.g. a redirect-only or API surface),
//   - `examplePaths`       : canonical example URLs that MUST round-trip
//                            (parse client-side to this tag AND, for documents,
//                            be admitted server-side; for non-documents, NOT be
//                            admitted),
//   - the visibility / policy fields the client policy + startup guards consume
//     (`requiresAuthBootstrap`, `loggedInGate`, `inLoggedOutUnion`,
//     `inLoggedInUnion`, `render`).
//
// Downstream artifacts are DERIVED from (or exhaustively checked against) this
// table:
//   - `route.ts` builds the typed `routeRegistry` from it (and cross-checks the
//     Foldkit parser ordering),
//   - `worker-routes.ts` derives `knownDocumentPathPatterns` from the
//     `serverDocument` patterns (no hand-maintained regex list),
//   - `product-policy.ts` reads the visibility fields,
//   - `route-coverage.test.ts` + `client-server-route-agreement.test.ts` lock
//     the parse coverage and prove client⇄server agreement so the `/trace` 302
//     bug is structurally impossible going forward.
//
// IMPORTANT: this module is intentionally dependency-free (no foldkit, no
// effect). The Foldkit-typed parsers stay in `route.ts`; the Cloudflare Worker
// (which does NOT depend on foldkit) imports ONLY this plain-data table to
// derive its document allowlist. Adding a route = one edit here.

// Auth gate applied to a logged-in user for routes that are part of
// `LoggedInRoute`. Drives `routeAllowedForLoggedInAuth` in product-policy.
//   - 'open'     : allowed for any logged-in (post-gate) user
//   - 'workroom' : requires Core Team membership + completed onboarding
//   - 'admin'    : requires admin flag + completed onboarding
//   - 'mullet'   : requires admin flag + completed onboarding + owner email
export type RouteLoggedInGate = 'open' | 'workroom' | 'admin' | 'mullet'

// Render disposition for view.ts exhaustiveness (rendering itself stays in
// view.ts; this only guarantees every route has a KNOWN render path so a route
// can never silently fall through to the maintenance body again).
//   - 'submodel'       : rendered through a LoggedOut/LoggedIn/Demo submodel
//   - 'statelessShell' : rendered through a stateless public-header page view
//   - 'loggedInOnly'   : rendered only inside the logged-in submodel
//   - 'demo'           : rendered through the demo submodel
//   - 'special'        : has a bespoke render branch (e.g. Onboarding)
//   - 'maintenance'    : NOT wired in view.ts; currently falls to the shared
//                        maintenance body. This is an HONEST classification of
//                        an existing route whose page has not (yet) been wired
//                        into `view.ts`. It is recorded explicitly rather than
//                        hidden so the latent "renders maintenance body" state
//                        is visible and intentional, not silent.
export type RouteRenderDisposition =
  | 'submodel'
  | 'statelessShell'
  | 'loggedInOnly'
  | 'demo'
  | 'special'
  | 'maintenance'

// How a route is served at the edge.
//   - 'spaDocument'    : the Worker serves the SPA app-shell document for this
//                        path (it is admitted by `knownDocumentPathPatterns`).
//   - 'redirectOnly'   : the path is intentionally NOT a served document; a
//                        hard navigation to it 302-redirects to "/" (e.g. a
//                        retired or internal-only alias).
//   - 'clientOnly'     : resolvable in the client parser but never directly
//                        navigated as a top-level document (e.g. the
//                        NotFound catch-all). Not admitted server-side.
export type RouteSurface = 'spaDocument' | 'redirectOnly' | 'clientOnly'

export type RouteTableEntry = Readonly<{
  // How this route is served at the edge.
  surface: RouteSurface
  // The canonical server-side document path matcher for this route, or `null`
  // when the route is not a server-served SPA document. When non-null AND
  // `surface === 'spaDocument'`, this regex is contributed to the Worker's
  // `knownDocumentPathPatterns` allowlist. Multiple tags may legitimately share
  // the SAME server pattern (e.g. Trace + TraceCompare both → /trace/...); the
  // Worker dedupes by `source`.
  serverDocument: RegExp | null
  // Canonical example URLs that exercise this route at the SERVER level. For
  // `spaDocument` routes, each must be admitted server-side; for
  // `redirectOnly` / `clientOnly` routes, each must be REJECTED server-side
  // (302 → "/"). Each path also parses through the client router; the resulting
  // tag is this entry's key UNLESS `clientParseTag` overrides it (see below).
  examplePaths: ReadonlyArray<string>
  // The tag the CLIENT parser yields for this entry's example paths, when it
  // differs from the entry key. Needed for the few routes whose server document
  // shape and client parser disagree by design:
  //   - Home: `/` renders the Landing scene, so `/` parses to `Landing`.
  //   - Dashboard: `/dashboard` is server-admitted but has no client parser yet,
  //     so it parses to `NotFound`.
  // Omit when the parse tag equals the entry key (the common case).
  clientParseTag?: string
  // Whether this route requires the auth bootstrap to be fetched before
  // resolving. Drives `routeRequiresAuthBootstrap` in product-policy.
  requiresAuthBootstrap: boolean
  // Logged-in auth gate (only consulted for routes in `LoggedInRoute`; the
  // value is harmless/ignored for routes that are not logged-in-resolvable).
  loggedInGate: RouteLoggedInGate
  // Whether this tag is a member of the `LoggedOutRoute` / `LoggedInRoute`
  // schema unions. Used by the startup exhaustiveness guards.
  inLoggedOutUnion: boolean
  inLoggedInUnion: boolean
  // Known render disposition (view.ts exhaustiveness guard).
  render: RouteRenderDisposition
}>

// Shared server document patterns. A single server pattern can admit several
// client route tags (e.g. `/autopilot`, `/autopilot/legal`, `/autopilot/work`
// are three client tags but one server regex). Declaring them once here and
// referencing them from the relevant entries keeps the table honest about
// which tags collapse to one server-admitted shape — and the Worker dedupes by
// `RegExp.source` so a shared pattern is contributed exactly once.
const ROOT = /^\/$/
const AUTOPILOT = /^\/autopilot(?:\/(?:legal|work))?$/
const BLOG = /^\/blog(?:\/[^/]+)?$/
const COMPONENTS = /^\/components(?:\/[^/]+)?$/
const DEMO = /^\/demo(?:\/.*)?$/
const DOCS = /^\/docs(?:\/[^/]+)?$/
const FILES = /^\/files(?:\/[^/]+)?$/
const FORUM = /^\/forum(?:\/.*)?$/
const ARTANIS_ACCOUNTS = /^\/artanis\/accounts$/
const SETTINGS = /^\/settings(?:\/[^/]+)?$/
const SITES_DEMO_CHECKOUT = /^\/sites\/demo-checkout(?:\/[^/]+)?$/
// Public shareable agent traces (#6209/#6211): /trace/{uuid} + /trace/compare/{ids}.
const TRACE = /^\/trace(?:\/.*)?$/
const QA_SWARM = /^\/qa\/[^/]+$/
const PYLON_CODEX_ASSIGNMENTS = /^\/pylon\/codex\/assignments\/[^/]+$/
const TEAMS =
  /^\/teams\/[^/]+(?:\/chat|\/files(?:\/[^/]+)?|\/projects\/[^/]+\/chat)$/
const THREAD = /^\/t\/[^/]+$/
const TRAINING_RUNS = /^\/training\/runs(?:\/[^/]+)?$/
const AGENTS = /^\/agents\/[^/]+$/
const ARTANIS_TRACES = /^\/artanis\/traces$/
const ARTANIS_GYM = /^\/artanis\/gym$/

// The full table, keyed by `AppRoute['_tag']`. `route.ts` enforces, at compile
// time, that these keys are EXACTLY the `AppRoute` tag union (no missing, no
// extra), so a route added to `AppRoute` without a table entry — or vice versa —
// fails the build.
export const routeTable = {
  Home: {
    surface: 'spaDocument',
    serverDocument: ROOT,
    examplePaths: ['/'],
    // `/` renders the Landing scene (homeRouter/landingRouter both map root to
    // LandingRoute), so the parser yields `Landing` for the root path.
    clientParseTag: 'Landing',
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Invite: {
    // `/invite` resolves in the client parser and is a startup membership
    // surface, but it is not in the server document allowlist (a hard nav to
    // `/invite` redirects home server-side); preserve that exact behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/invite'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Onboarding: {
    surface: 'spaDocument',
    serverDocument: /^\/onboarding$/,
    examplePaths: ['/onboarding'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'special',
  },
  Order: {
    surface: 'spaDocument',
    serverDocument: /^\/order$/,
    examplePaths: ['/order'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  OrderDetail: {
    surface: 'spaDocument',
    serverDocument: /^\/orders\/[^/]+$/,
    examplePaths: ['/orders/software_order_1'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Autopilot: {
    surface: 'spaDocument',
    serverDocument: AUTOPILOT,
    examplePaths: ['/autopilot'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  AutopilotVertical: {
    surface: 'spaDocument',
    serverDocument: AUTOPILOT,
    examplePaths: ['/autopilot/legal'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  AutopilotWork: {
    surface: 'spaDocument',
    serverDocument: AUTOPILOT,
    examplePaths: ['/autopilot/work'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  AutopilotWorkDetail: {
    // `/autopilot/work/{ref}` resolves client-side but is NOT admitted by the
    // server `/autopilot(?:/(legal|work))?$` document pattern (a deeper path), so
    // a hard nav redirects home — preserve that exact existing behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/autopilot/work/autopilot_work_order.visible_1'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Forge: {
    surface: 'spaDocument',
    serverDocument: /^\/forge$/,
    examplePaths: ['/forge'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Decisions: {
    // `/decisions` resolves client-side but is NOT in the server document
    // allowlist today (a hard nav redirects home); preserve that behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/decisions'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Workspace: {
    // `/workspaces/{id}` resolves client-side but is NOT in the server document
    // allowlist today (a hard nav redirects home); preserve that behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/workspaces/workspace_seed'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'submodel',
  },
  Workroom: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/workrooms/workroom_1'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  WorkroomTab: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/workrooms/workroom_1/overview'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Chat: {
    // `/autopilot` parses to AutopilotRoute, not ChatRoute (chatRouter is a
    // deprecated/duplicate router kept out of the parser); ChatRoute has no
    // directly-navigable canonical path. No example paths.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: [],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  TeamChat: {
    surface: 'spaDocument',
    serverDocument: TEAMS,
    examplePaths: ['/teams/team_1/chat'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  TeamProjectChat: {
    surface: 'spaDocument',
    serverDocument: TEAMS,
    examplePaths: ['/teams/team_1/projects/project_1/chat'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  TeamFiles: {
    surface: 'spaDocument',
    serverDocument: TEAMS,
    examplePaths: ['/teams/team_1/files'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  TeamFile: {
    surface: 'spaDocument',
    serverDocument: TEAMS,
    examplePaths: ['/teams/team_1/files/file_1'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  PersonalFile: {
    surface: 'spaDocument',
    serverDocument: FILES,
    examplePaths: ['/files/file_1'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Thread: {
    surface: 'spaDocument',
    serverDocument: THREAD,
    examplePaths: ['/t/55555555-5555-4555-8555-555555555555'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Docs: {
    surface: 'spaDocument',
    serverDocument: DOCS,
    examplePaths: ['/docs'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  DocsPage: {
    surface: 'spaDocument',
    serverDocument: DOCS,
    examplePaths: ['/docs/api'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ProductPromises: {
    surface: 'spaDocument',
    serverDocument: /^\/promises$/,
    examplePaths: ['/promises'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  PublicTrainingRuns: {
    surface: 'spaDocument',
    serverDocument: TRAINING_RUNS,
    examplePaths: ['/training/runs'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  PublicTrainingRun: {
    surface: 'spaDocument',
    serverDocument: TRAINING_RUNS,
    examplePaths: ['/training/runs/run.cs336.a1.demo'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Forum: {
    surface: 'spaDocument',
    serverDocument: FORUM,
    examplePaths: ['/forum'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ForumForum: {
    surface: 'spaDocument',
    serverDocument: FORUM,
    examplePaths: ['/forum/f/product-promises'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ForumTopic: {
    surface: 'spaDocument',
    serverDocument: FORUM,
    examplePaths: ['/forum/t/55555555-5555-4555-8555-555555555555'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ForumReceipt: {
    surface: 'spaDocument',
    serverDocument: FORUM,
    examplePaths: ['/forum/receipts/receipt_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  SiteCheckoutDemo: {
    surface: 'spaDocument',
    serverDocument: SITES_DEMO_CHECKOUT,
    examplePaths: ['/sites/demo-checkout'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  SiteCheckoutDemoReturn: {
    surface: 'spaDocument',
    serverDocument: SITES_DEMO_CHECKOUT,
    examplePaths: ['/sites/demo-checkout/success'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ClientsPreview: {
    // `/clients-preview` resolves client-side but is NOT in the server document
    // allowlist today (a hard nav redirects home); preserve that behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/clients-preview'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Components: {
    surface: 'spaDocument',
    serverDocument: COMPONENTS,
    examplePaths: ['/components'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ComponentsFamily: {
    surface: 'spaDocument',
    serverDocument: COMPONENTS,
    examplePaths: ['/components/buttons'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Business: {
    surface: 'spaDocument',
    serverDocument: /^\/business$/,
    examplePaths: ['/business'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  LandingPreview: {
    surface: 'spaDocument',
    serverDocument: /^\/preview\/landing$/,
    examplePaths: ['/preview/landing'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Animations: {
    surface: 'spaDocument',
    serverDocument: /^\/animations$/,
    examplePaths: ['/animations'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Activity: {
    // `/activity` resolves client-side but is NOT in the server document
    // allowlist today (a hard nav redirects home); preserve that behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/activity'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ArtanisAccounts: {
    surface: 'spaDocument',
    serverDocument: ARTANIS_ACCOUNTS,
    examplePaths: ['/artanis/accounts'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  ArtanisGym: {
    surface: 'spaDocument',
    serverDocument: ARTANIS_GYM,
    examplePaths: ['/artanis/gym'],
    requiresAuthBootstrap: true,
    loggedInGate: 'admin',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Run: {
    surface: 'spaDocument',
    serverDocument: /^\/run$/,
    examplePaths: ['/run'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Gym: {
    surface: 'spaDocument',
    serverDocument: /^\/gym$/,
    examplePaths: ['/gym'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  // Owner/internal Gym document: `/gym/oss` serves the admin-gated GPT-OSS
  // playground while bare `/gym` is the public Terminal-Bench web visualizer.
  GymOss: {
    surface: 'spaDocument',
    serverDocument: /^\/gym\/oss$/,
    examplePaths: ['/gym/oss'],
    requiresAuthBootstrap: true,
    loggedInGate: 'admin',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  // `/mirrorcode` — public "MirrorCode, powered by Khala" page (#6378). Same
  // public, logged-out, served-document posture as bare `/gym`.
  MirrorCode: {
    surface: 'spaDocument',
    serverDocument: /^\/mirrorcode$/,
    examplePaths: ['/mirrorcode'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Tassadar: {
    surface: 'spaDocument',
    serverDocument: /^\/tassadar$/,
    examplePaths: ['/tassadar'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'submodel',
  },
  TassadarReplay: {
    surface: 'spaDocument',
    serverDocument: /^\/tassadar\/replay\/[^/]+$/,
    examplePaths: ['/tassadar/replay/first-real-settlement'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Login: {
    surface: 'spaDocument',
    serverDocument: /^\/login$/,
    examplePaths: ['/login'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'submodel',
  },
  Blog: {
    surface: 'spaDocument',
    serverDocument: BLOG,
    examplePaths: ['/blog'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  BlogPost: {
    surface: 'spaDocument',
    serverDocument: BLOG,
    examplePaths: ['/blog/some-post'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  PublicAgent: {
    surface: 'spaDocument',
    serverDocument: AGENTS,
    // The short `/artanis` + `/adjutant` aliases are admitted by their own
    // explicit server patterns (declared as extra allowlist entries derived in
    // the Worker) and resolved client-side by `urlToAppRoute`'s special-case.
    examplePaths: ['/agents/artanis'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'submodel',
  },
  ArtanisTraceTree: {
    surface: 'spaDocument',
    serverDocument: ARTANIS_TRACES,
    examplePaths: ['/artanis/traces'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Share: {
    surface: 'spaDocument',
    serverDocument: /^\/share\/[^/]+$/,
    examplePaths: ['/share/share_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  // Public, shareable ATIF trace render at `/trace/{uuid}` (issue #6209). No
  // auth bootstrap and `loggedInGate: 'open'` — anyone with the link can view a
  // shared trace. Member of BOTH unions so a logged-in visitor resolves it as a
  // public route too. THIS is the route whose missing server entry caused the
  // 2026-06-24 prod 302 bug; it is now structurally guaranteed admitted.
  Trace: {
    surface: 'spaDocument',
    serverDocument: TRACE,
    examplePaths: ['/trace/0e08d2db-2026-4624-9a39-f1efe8000001'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  // Public, shareable comparison of N traces at `/trace/compare/{ids}` (#6211).
  TraceCompare: {
    surface: 'spaDocument',
    serverDocument: TRACE,
    examplePaths: ['/trace/compare/a,b,c'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  QaSwarm: {
    surface: 'spaDocument',
    serverDocument: QA_SWARM,
    examplePaths: ['/qa/qa-run.khala-code-nightly.2026-07-02'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  PylonCodexAssignmentStatus: {
    surface: 'spaDocument',
    serverDocument: PYLON_CODEX_ASSIGNMENTS,
    examplePaths: [
      '/pylon/codex/assignments/assignment.public.khala_coding.chatcmpl_example',
    ],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  Moksha: {
    surface: 'spaDocument',
    serverDocument: /^\/moksha$/,
    examplePaths: ['/moksha'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Moksha2: {
    surface: 'spaDocument',
    serverDocument: /^\/moksha2$/,
    examplePaths: ['/moksha2'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  // Landing IS the homepage at `/` (covered by ROOT). The `/landing` path is an
  // inbound-only alias that is also a served document.
  Landing: {
    surface: 'spaDocument',
    serverDocument: /^\/landing$/,
    examplePaths: ['/landing'],
    // `/` parses client-side to `Landing`, so this drives whether the SPA
    // fetches `/api/auth/session` on the homepage. Keep `true` so a signed-in
    // user is reflected in the public header on the landing scene (and any
    // public route that parses to Landing).
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Terms: {
    surface: 'spaDocument',
    serverDocument: /^\/terms$/,
    examplePaths: ['/terms'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'statelessShell',
  },
  Privacy: {
    surface: 'spaDocument',
    serverDocument: /^\/privacy$/,
    examplePaths: ['/privacy'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'statelessShell',
  },
  Code: {
    surface: 'spaDocument',
    serverDocument: /^\/code$/,
    examplePaths: ['/code'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'statelessShell',
  },
  Khala: {
    surface: 'spaDocument',
    serverDocument: /^\/khala$/,
    examplePaths: ['/khala'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  KhalaChat: {
    surface: 'spaDocument',
    serverDocument: /^\/chat$/,
    examplePaths: ['/chat'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  // The Pylon scene lives at `/pylons` (not the root).
  Pylon: {
    surface: 'spaDocument',
    serverDocument: /^\/pylons$/,
    examplePaths: ['/pylons'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Download: {
    // `/download` resolves client-side but is NOT in the server document
    // allowlist today (a hard nav redirects home); preserve that behavior.
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/download'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  // `/dashboard` is admitted as a server document (it was in the original
  // allowlist), but there is no `dashboardRouter` in the client parser, so a
  // direct visit parses to `NotFound`. Preserve that exact existing behavior.
  Dashboard: {
    surface: 'spaDocument',
    serverDocument: /^\/dashboard$/,
    examplePaths: ['/dashboard'],
    clientParseTag: 'NotFound',
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  // The /pro operator/power-user console (#6179). Open to ANY signed-in user.
  // `/pro` resolves client-side but is NOT in the server document allowlist
  // today (a hard nav redirects home); preserve that behavior.
  Pro: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/pro'],
    requiresAuthBootstrap: true,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  OperatorDashboard: {
    surface: 'spaDocument',
    serverDocument: /^\/operator\/dashboard$/,
    examplePaths: ['/operator/dashboard'],
    requiresAuthBootstrap: true,
    loggedInGate: 'admin',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Billing: {
    surface: 'spaDocument',
    serverDocument: /^\/billing$/,
    examplePaths: ['/billing'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Usage: {
    surface: 'spaDocument',
    serverDocument: /^\/usage$/,
    examplePaths: ['/usage'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Stats: {
    surface: 'spaDocument',
    serverDocument: /^\/stats$/,
    examplePaths: ['/stats'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'submodel',
  },
  PublicStatsArchive: {
    surface: 'spaDocument',
    serverDocument: /^\/stats-old$/,
    examplePaths: ['/stats-old'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: false,
    render: 'submodel',
  },
  Admin: {
    surface: 'spaDocument',
    serverDocument: /^\/admin$/,
    examplePaths: ['/admin'],
    requiresAuthBootstrap: true,
    loggedInGate: 'admin',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Mullet: {
    surface: 'spaDocument',
    serverDocument: /^\/mullet$/,
    examplePaths: ['/mullet'],
    requiresAuthBootstrap: true,
    loggedInGate: 'mullet',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Images: {
    surface: 'spaDocument',
    serverDocument: /^\/images$/,
    examplePaths: ['/images'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Settings: {
    surface: 'spaDocument',
    serverDocument: SETTINGS,
    examplePaths: ['/settings'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  SettingsSection: {
    surface: 'spaDocument',
    serverDocument: SETTINGS,
    examplePaths: ['/settings/profile'],
    requiresAuthBootstrap: true,
    loggedInGate: 'workroom',
    inLoggedOutUnion: false,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
  Demo: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  DemoLegal: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/legal'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'statelessShell',
  },
  DemoOrder: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/order'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  DemoThread: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/t/thread_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  DemoTeamProjectChat: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/teams/team_1/projects/project_1/chat'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  DemoTeamFiles: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/teams/team_1/files'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  DemoTeamFile: {
    surface: 'spaDocument',
    serverDocument: DEMO,
    examplePaths: ['/demo/teams/team_1/files/file_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  // The Demo2 family resolves in the client parser but is NOT a server document:
  // the original server allowlist only admitted `/demo(?:/.*)?` (the `/demo`
  // family), never `/demo2*`. So a hard navigation to any `/demo2*` path 302s
  // home, while the client-side parser still resolves it. Preserve that exact
  // existing behavior — these are `clientOnly`.
  Demo2: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  Demo2Order: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2/order'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  Demo2Thread: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2/t/thread_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  Demo2TeamProjectChat: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2/teams/team_1/projects/project_1/chat'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  Demo2TeamFiles: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2/teams/team_1/files'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  Demo2TeamFile: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: ['/demo2/teams/team_1/files/file_1'],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: false,
    inLoggedInUnion: false,
    render: 'demo',
  },
  // The catch-all NotFound route is parser-resolvable for any unmatched path,
  // but is never a directly-navigated server document (an unknown document path
  // 302-redirects to "/"). No example paths; not server-admitted.
  NotFound: {
    surface: 'clientOnly',
    serverDocument: null,
    examplePaths: [],
    requiresAuthBootstrap: false,
    loggedInGate: 'open',
    inLoggedOutUnion: true,
    inLoggedInUnion: true,
    render: 'loggedInOnly',
  },
} as const satisfies Record<string, RouteTableEntry>

export type RouteTableTag = keyof typeof routeTable

// Short top-level public-agent aliases (`/artanis`, `/adjutant`) are resolved
// client-side by `urlToAppRoute` and must ALSO be admitted as server documents
// (they predate the `/agents/{ref}` canonical path). They are part of the
// PublicAgent surface but are not their own `AppRoute` tags, so they are
// declared here alongside the table and contributed to the Worker allowlist.
export const publicAgentAliasDocumentPatterns: ReadonlyArray<RegExp> = [
  /^\/artanis$/,
  /^\/adjutant$/,
]

// The derived server document allowlist: every `spaDocument` route's
// `serverDocument` pattern, deduped by source, plus the short public-agent
// aliases. THIS replaces the hand-maintained `knownDocumentPathPatterns` list
// in `worker-routes.ts`. Adding a `spaDocument` route to the table above
// automatically admits it server-side — the `/trace` 302 bug cannot recur.
export const knownDocumentPathPatterns: ReadonlyArray<RegExp> = (() => {
  const seen = new Set<string>()
  const patterns: RegExp[] = []
  for (const entry of Object.values(routeTable)) {
    if (entry.surface !== 'spaDocument' || entry.serverDocument === null) {
      continue
    }
    if (seen.has(entry.serverDocument.source)) {
      continue
    }
    seen.add(entry.serverDocument.source)
    patterns.push(entry.serverDocument)
  }
  for (const alias of publicAgentAliasDocumentPatterns) {
    if (!seen.has(alias.source)) {
      seen.add(alias.source)
      patterns.push(alias)
    }
  }
  return patterns
})()
