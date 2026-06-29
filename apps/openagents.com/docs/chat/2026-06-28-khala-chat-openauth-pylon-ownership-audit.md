# Khala Chat OpenAuth Pylon Ownership Audit

Date: 2026-06-28
Status: product/auth audit
Owner: openagents.com `/chat`, Worker `/api/khala/chat`, Pylon account APIs

## Problem

The current `/chat` experience can now answer "what Pylons are connected" from
the real public Pylon registry. That fixed the first failure mode: Khala no
longer drifts into StarCraft/electrical-grid roleplay for basic Pylon questions.

The next visible failure is ownership. In a local browser session, the user
asked whether the listed Pylons were theirs. The answer was technically safe:
the public registry projection cannot prove ownership, and public chat should
not expose private owner linkage. But as product behavior it is not good enough.
If a signed-in user is using openagents.com, OpenAgents should know the user's
OpenAuth account and should show the Pylons linked to that account.

The right answer is not to make the public projection leak ownership. The right
answer is to make `/chat` login-aware:

- anonymous chat keeps using public Pylon context only;
- signed-in chat uses the OpenAuth browser session to load the account's linked
  Pylons;
- account-owned Pylon answers say "your connected Pylons" when the route has an
  authenticated owner context;
- the route still refuses to expose token, wallet, prompt, path, private trace,
  settlement, or credential material.

## Current Behavior

`POST /api/khala/chat` is documented and implemented as a public,
unauthenticated, stateless streaming route. It decodes the client-supplied
message list, rate-limits by IP, and either returns a fast/static answer or
opens the generic Khala inference stream.

For Pylons, the route currently has only a public context loader:

- `workers/api/src/khala-chat-routes.ts` accepts an optional
  `loadPylonContext(env)` dependency.
- Production wires that to `loadKhalaChatPylonContext(makeD1PylonApiStore(...))`
  in `workers/api/src/index.ts`.
- `workers/api/src/khala-chat-pylon-context.ts` calls
  `PylonApiStore.listRegistrations(100)`, derives public stats via
  `publicPylonStatsFromRegistrations`, and emits a bounded public-safe context
  block.
- `answerKhalaChatPylonQuestion()` intercepts list/status/capability/register
  Pylon questions before provider inference and returns deterministic text.

That path intentionally has no browser session, no OpenAuth user id, and no
owner-scoped query. Therefore a follow-up like "are those mine?" cannot be
answered honestly from the current route context. The model falls back to the
safe public answer: it cannot verify ownership from the public registry.

## Existing OpenAuth/Pylon Authority

The underlying identity bridge mostly exists already. This is important: the
gap is not "invent OpenAuth for Pylons from scratch." The gap is that `/chat`
does not use it.

Implemented pieces:

- OpenAuth browser sessions are verified in `workers/api/src/index.ts` via
  `verifySession()` and the shared `requireBrowserSession` boundary from
  `makeBrowserSessionBoundary`.
- Login entrypoints already exist for GitHub and email code sign-in:
  `/login/github`, `/login/email`, `/auth/callback`, and `/api/session`.
- Migration `workers/api/migrations/0234_pylon_openauth_links.sql` adds:
  - `agent_credentials.openauth_user_id`;
  - `openauth_agent_links`;
  - indexes for owner/status lookup;
  - backfill from approved agent-owner claims.
- `workers/api/src/agent-registration.ts` exposes:
  - `linkOpenAuthAgent(record)`;
  - `listLinkedAgentsForOpenAuthUser(openauthUserId, limit)`;
  - `AgentCredentialRecord.openauthUserId`.
- `workers/api/src/pylon-api.ts` exposes:
  - `PylonApiRegistrationRecord.ownerAgentUserId`;
  - `PylonApiStore.listRegistrationsForOwnerAgentUserIds(...)`.
- `workers/api/src/pylon-api-routes.ts` already has account-scoped routes:
  - `GET /api/account/pylons`;
  - `POST /api/account/pylon-agent-links`.
- `GET /api/account/pylons` requires the OpenAuth browser session, resolves the
  signed-in user's linked agent ids, reads only registrations for those agent
  ids, and returns linked agents, Pylons, recent assignments, recent events, and
  a summary.
- `POST /api/account/pylon-agent-links` requires the OpenAuth browser session,
  accepts an `oa_agent_` token, authenticates it, and links that agent
  credential to the signed-in OpenAuth user unless it is already linked to a
  different user.
- Pylon route tests cover same-OpenAuth rotated credentials reclaiming and
  heartbeating an existing Pylon, and reject unrelated credentials from
  reclaiming/heartbeating another account's Pylon.

The key product fact: OpenAuth-linked Pylon ownership is now a real server-side
concept. `/chat` is simply still using the older public-only Pylon context.

## Security Boundary

Do not solve this by broadening the public registry. The boundary should be:

- Public Pylon context:
  - visible to anonymous users;
  - aggregate or public-safe rows only;
  - no ownership confirmation;
  - no linked-agent token prefixes;
  - no private assignment bodies, raw logs, traces, paths, prompts, wallet
    material, invoices, preimages, balances, or settlement facts.
- Account Pylon context:
  - visible only when `verifySession()` succeeds for the browser request;
  - loaded through the same OpenAuth account link model as
    `GET /api/account/pylons`;
  - may say "your Pylons" and list account-linked Pylons;
  - may include public-safe assignment/event projections and linked-agent
    display labels;
  - should avoid token prefixes in chat prose unless there is a very explicit
    settings/debug affordance outside the chat transcript;
  - must still not expose bearer tokens, raw credential hashes, wallet secrets,
    payment preimages, private traces, local file paths, raw prompts, or private
    diffs.

The account context is an authorization boundary, not a public projection. It
should be `no-store` and should never be embedded in a public static page,
public cache, public sync channel, public stats endpoint, or model-visible
context for an anonymous request.

## Product Requirement

The expected user experience:

1. A user visits `/chat` signed out.
2. They can still ask general questions and public Pylon questions.
3. If they ask "are those mine?", "show my Pylons", "connect to my Pylon", or
   "use my Pylon", the route should say sign-in is required and point to the
   OpenAuth login path.
4. A user signs in with OpenAuth and returns to `/chat`.
5. `/chat` can now answer:
   - "yes, these are linked to your OpenAuth account";
   - "you have N linked agents and M linked Pylons";
   - "these Pylons are online/assignment-ready now";
   - "this Pylon advertises Codex/Claude capacity";
   - "this Pylon has recent assignments/events";
   - "I can route eligible caller-owned coding workflows only through your own
     linked capacity when the authenticated execution path is used."
6. If no linked Pylons exist, `/chat` should offer the concrete linking path:
   sign in, create/use an agent token, run or paste it through the
   `POST /api/account/pylon-agent-links` backed flow, then heartbeat/register
   the Pylon.

The public `/chat` UI can stay minimal. The important requirement is not a large
dashboard; it is that the chat route can tell whether the browser request has an
OpenAuth account and can load that account's Pylons.

## Implementation Plan

### 1. Add an account Pylon context type

Create a sibling to the public `KhalaChatPylonContext`, for example:

- `KhalaChatAccountPylonContext`
- `KhalaChatLinkedAgentSummary`
- `KhalaChatOwnedPylonSummary`
- `KhalaChatPylonContextMode = 'anonymous_public' | 'authenticated_account'`

The account context should be built from the same data source as
`GET /api/account/pylons`: OpenAuth session user id -> linked agent user ids ->
Pylon registrations/assignments/events. It should be bounded and public-safe
before it ever reaches a prompt or deterministic answer renderer.

Do not reuse `GET /api/account/pylons` by HTTP-fetching the Worker from itself.
Extract the account-context loader behind a shared typed function and have both
the API route and Khala chat call that function.

### 2. Thread optional browser session into `/api/khala/chat`

Keep `/api/khala/chat` publicly callable, but let it attempt optional session
verification:

- no session: load public Pylon context only;
- valid session: load account Pylon context and, if useful, also include public
  aggregate counts as background;
- invalid/expired session cookie: clear/refresh behavior should follow the
  existing session boundary patterns, but the chat route should fail soft to
  anonymous public context rather than 500.

This requires extending `KhalaChatRouteDependencies` beyond `env`-only
`loadPylonContext`. A likely shape:

```ts
loadPylonContext?: (input: {
  env: unknown
  request: Request
  ctx: ExecutionContext
}) => Effect.Effect<KhalaChatPylonContextEnvelope, unknown>
```

The envelope can carry `mode`, `publicContext`, `accountContext`, and a
`sessionRef` or redacted owner id for internal logs only. Do not place raw
session tokens in any context object.

### 3. Teach deterministic Pylon answers about ownership

Extend `answerKhalaChatPylonQuestion()` or split it into public/account answer
renderers. It should recognize bounded account forms after the route has already
selected Pylon context:

- "are those mine"
- "which Pylons are mine"
- "show my connected Pylons"
- "connect to my Pylon"
- "use my Pylon"
- "what is running on my Pylons"

The answer matrix:

- anonymous + ownership question: sign in required; no ownership inference;
- authenticated + linked Pylons: answer from account context;
- authenticated + no linked Pylons: say none linked and show linking next step;
- authenticated + context load failure: say account Pylon state is unavailable
  for this turn, not that no Pylons exist;
- account context present + public registry also has other Pylons: clearly
  distinguish "your linked Pylons" from "public network Pylons."

### 4. Add a tiny `/chat` sign-in affordance

The page does not need a dashboard, but it needs a way out of the dead end. A
minimal top/right nav or info action is enough:

- signed out: "Sign in" linking to `/login?returnTo=/chat`;
- signed in: "Pylons" or "Account" linking to an account Pylon surface, or a
  compact status if that surface already exists;
- the transcript should not explain auth plumbing unless asked.

This should use the existing app session bootstrap rather than inventing a
second client auth path.

### 5. Use the existing account routes for programmatic/API access

The user-facing web chat is not the only consumer. Programmatic clients need the
same answer:

- Browser/API: `GET /api/account/pylons` under OpenAuth session for owner view.
- Linking: `POST /api/account/pylon-agent-links` under OpenAuth session with an
  agent token in the request body.
- Registered agent writes: keep `POST /api/pylons/register`,
  `/heartbeat`, `/wallet-readiness`, and assignment progress on bearer token +
  `Idempotency-Key`.
- Public network view: keep `GET /api/pylons` and
  `GET /api/public/pylon-stats` public-safe.

Do not make `/api/public/pylon-stats` answer "mine." Do not add ownership fields
to public Pylon rows.

## Prompt/Model Guidance

The system prompt should include account context only when the request has a
verified session. It should phrase the context as facts, not capabilities:

- "The signed-in OpenAuth account has these linked Pylons..."
- "Use these rows to answer ownership/status questions."
- "Do not reveal token prefixes or private data."
- "Do not say another public Pylon belongs to the user."
- "If asked to dispatch work, explain the caller-owned execution boundary and
  use only explicit authenticated execution routes."

The deterministic answer path should own common ownership/status questions so
the model does not have to infer the auth boundary from prose.

## Acceptance Tests

Add focused tests before broad UI work:

- `khala-chat-routes.test.ts`: anonymous "are those mine?" returns sign-in
  required and does not infer ownership.
- `khala-chat-routes.test.ts`: authenticated account with linked Pylons returns
  "your linked Pylons" and includes only those linked rows.
- `khala-chat-routes.test.ts`: authenticated account with no links returns
  "no linked Pylons" plus link/register next step.
- `khala-chat-routes.test.ts`: account context load failure says unavailable,
  not empty.
- `khala-chat-routes.test.ts`: generic inference request receives account
  context only when the request has a verified session.
- `pylon-api-routes.test.ts`: `GET /api/account/pylons` never returns another
  OpenAuth user's linked Pylons.
- `pylon-api-routes.test.ts`: `POST /api/account/pylon-agent-links` rejects an
  agent token already linked to another OpenAuth user.
- Browser scene test: signed-out `/chat` has a sign-in path; signed-in `/chat`
  can ask "which Pylons are mine?" and render an account-scoped answer.

## Open Questions

- Should `/chat` itself show a compact account/Pylon status, or should it only
  answer in the transcript and link to a separate account Pylon page?
- Should linking from the browser require pasting an `oa_agent_` token, or
  should the Pylon CLI use a device-code style OpenAuth flow so the user never
  pastes a long secret into the browser?
- Should account Pylon context include recent assignment/event summaries by
  default, or only when the user asks "what is running"?
- Should account Pylon context include linked-agent display names but omit token
  prefixes in all chat output?

## Bottom Line

The user's complaint is correct. Public chat should not claim ownership from
public registry rows, but a signed-in OpenAgents user should not be trapped in a
public-only answer. The central auth server is already the right authority:
OpenAuth session -> linked agents -> owned Pylons.

The next implementation should make `/api/khala/chat` optional-session-aware,
feed an account-scoped Pylon context to deterministic Pylon answers, and add a
minimal `/chat` sign-in path. That preserves the public/private boundary while
letting a real user ask the obvious question: "which connected Pylons are mine?"
