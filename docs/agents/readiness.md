# Agent readiness of openagents.com

What third-party agent-readiness checkers report for `https://openagents.com`,
every check that failed, and where each fix lives. The umbrella issue is
[#11083](https://github.com/OpenAgentsInc/openagents/issues/11083); the
2026-10-14 re-measure is
[#11629](https://github.com/OpenAgentsInc/openagents/issues/11629).

## 2026-10-14: re-measure

All scans were read-only, against production. "Before" is revision
`coder-web-577cc8118e-20261014191503` (the #11083 work deployed); "after"
is production after this change's deploys: `coder-web-f99baee65c-20261014195620`
(commit `bcedc28ef8`), then the order-aware negotiation fix (see the end of
this section).

| Checker | How it was run | Baseline 2026-10-09 | Before 2026-10-14 | After 2026-10-14 |
| --- | --- | --- | --- | --- |
| Is It Agent Ready (Cloudflare) | `POST https://isitagentready.com/api/scan` | Level 0, 2 pass / 14 fail / 6 neutral | **Level 5 "Agent-Native"**: 14 pass, 2 fail (dnsAid, authMd), 6 neutral (webBotAuth, 5 commerce) | **Level 5 "Agent-Native"**, unchanged: dnsAid (owner DNS) and authMd (owner decision) still fail |
| AgentReadyCheck | `GET https://www.agentreadycheck.com/api/check?url=` | 0 / 100 | **100 / 100 "agent-ready"**: llms, openapi pass; mcp skip | **100 / 100**, unchanged |
| AG AgentReady | MCP `tools/call run_website_scan {"website_url": …, "force_rescan": true}` at `https://agagentready.com/mcp` | Overall 33 (agent 42, revenue 26, trust 36) | **Overall 48** (agent/WebMCP 65, revenue 34, trust 52); WebMCP Verified, 5 tools | **Overall 57** (agent/WebMCP 72, revenue 45, trust 61): one `h1`, machine-readable email; all seven WebMCP prerequisites present ([report](https://agagentready.com/report/4158c3be627ba79b7ba896a23e08553eb48dc06b092c15ad)) |
| AgentGrade (new to this record) | MCP `tools/call scan_compact` / `scan_url` at `https://agentgrade.com/mcp` (no key) | Not run | **45 / 60 checks, 75 %, grade B** | **52 / 61, 85 %, grade A-** after the first deploy; after the second (`coder-web-769f0d876a-20261014202536`, commit `39eb8c473b`): **53 / 61, 87 %, grade A-**; every required check outside payments passes |
| Readiness.sh | Catalog `GET https://readiness.sh/api/v1/checks` (still criteria set v27, 124 checks), applied by hand; a scored scan still needs an account token (`POST /api/v1/scans`), and `GET /api/v1/scores/openagents.com` answers `scan_not_found` | Not scored | Not scored (hand pass below) | Not scored |

Searched for checkers and standards that appeared since 2026-10-09. New
with a public API: **AgentGrade** (`agentgrade.com/mcp`, run above). Found
without a usable public API: AgentReady by WP Creative (form only), the
Apify "AI Agent Readiness Audit" actor (needs an Apify token), Cloudflare's
URL Scanner "Agent Readiness" tab (needs a Cloudflare API token; it runs
the same checks as Is It Agent Ready), websiterating.com's checker (form
only). New standard text: the Agent Discovery Protocol individual draft
([draft-pro-adp-agent-discovery-02](https://datatracker.ietf.org/doc/draft-pro-adp-agent-discovery/),
2026-06-18), which defaults to `/.well-known/agent.json`.

### What failed before this change, and what happened to it

Is It Agent Ready:

- `dnsAid` (fail): no `_agents` SVCB/TXT records. openagents.com's DNS is
  on Cloudflare (`alina`/`karl.ns.cloudflare.com`), not in our Cloud DNS
  (the project's only zone is `fleet.internal.`), so the exact records are
  in the workspace's `NEEDS_OWNER.md` for the owner to add.
- `authMd` (fail): `/auth.md` and the `agent_auth` block are found, but the
  checker counts a registration method as complete only for the auth.md
  agent flows (ID-JAG identity assertion, verified email, or anonymous
  registration with a `claim_uri`). We offer OAuth dynamic client
  registration with a person approving in the browser, and API keys a
  person makes; there is no endpoint where an agent gets its own account.
  Advertising one we do not run would be false, so this stays failing
  until the owner decides whether agents may register themselves
  (an owner decision, not a code gap).
- Neutral: `webBotAuth` (we do not send signed bot requests; a directory
  would describe nothing), and x402, MPP, UCP, ACP, AP2 (the scanner does
  not classify the site as commerce; paying per request is #11085, and
  the rails are Bitcoin-only, so the EVM-based x402 and MPP defaults do
  not apply).

AgentReadyCheck: `mcp` is skipped because `llms.txt` names the docs MCP
server in a way its parser does not read as an MCP endpoint; it is
informational ("optional"), and the server card and `/.well-known/mcp.json`
already pass in the other checkers. No change.

AG AgentReady prerequisites that were missing:

- Semantic structure: **0 `h1`** on the homepage (the intro with the old
  heading was dropped in `aad36d0121`). Fixed: the homepage has one
  visually hidden `h1`, "OpenAgents".
- Machine-readable contact: no `mailto:` link. Fixed: the legal links under
  the homepage composer (signed-out visitors) add **Contact**
  (`mailto:chris@openagents.com`, the address the terms and privacy policy
  already give), and the `Organization` JSON-LD carries `email` and
  `contactPoint` (support, and security from `security.txt`).
- Its free findings ("contact options are hard to find", "no obvious next
  step", "contact details aren't complete") and the revenue score judge the
  homepage as a sales page. The homepage is a chat on purpose (the owner's
  design); no change.

AgentGrade checks that failed (required unless marked optional):

| Check | Before | Now |
| --- | --- | --- |
| llms.txt linked from HTML | No `<link rel="alternate" type="text/plain">` | Fixed: every page's `<head>` links `/llms.txt` and `/llms-full.txt` (optional check) that way |
| Skill file found, frontmatter valid | `/skill.md` 404 | Fixed: `/skill.md` serves the OpenAgents API skill (agentskills.io frontmatter) |
| Accept: text returns text | `/` with `Accept: text/plain` answered HTML | Fixed: plain text gets the Markdown twin as `text/plain` |
| Accept: JSON returns JSON | `/` with `Accept: application/json` answered HTML | Fixed: `/` answers a JSON site index (name, description, every discovery document) |
| Returns preferred Content-Type | Only Markdown was negotiated | Fixed: Markdown, plain text, and JSON against HTML by `q`, the first listed winning a tie (`agent_ready::preferred`; the first deploy let Markdown win a tie with HTML listed first, which this check catches, so a second deploy fixed it) |
| Structured errors (JSON 404) | HTML 404 for `Accept: application/json` | Fixed: a JSON `{"error": "not_found", …}` 404 |
| Payments: live 402, x402, MPP, SPT, L402, Bazaar | Not on openagents.com's own pages | Not changed here: paying per request is the API's (#11085); x402/MPP/SPT examples are EVM/Stripe and out of scope (Bitcoin-only rails). The scanner probes `POST /`, which is a page |
| Rate limit headers (optional) | None on pages | Not changed: the API sends `Retry-After` on `429`; pages are not rate-limited per client |
| agents.txt (optional) | 404 | Not added: no format exists; `robots.txt` carries the policy |
| RSS/Atom feed (optional) | None | Not added: the site has no posts to subscribe to |
| WebMCP manifest `/.well-known/webmcp.json`, form `tool-name` attributes (optional) | None | Not added: not in the WebMCP spec; the five tools register through `document.modelContext`, which both WebMCP checkers verify live |
| Web Bot Auth directory (optional) | None | Not applicable: we do not send signed bot traffic |
| WebFinger, DID, NIP-05, AT Protocol DID (optional) | None | Owner decision: which identity keys to publish |
| MCP auth discovery resolves, SSE transport (optional) | — | `/mcp/docs` needs no auth and is Streamable HTTP (a `GET` answers `405`, as the transport allows) |

### Readiness.sh catalog, applied by hand again

Of the checks that failed on 2026-10-09, these now pass on the evidence
above (the other checkers' requests): `json-ld`, `metadata-completeness`,
`org-schema-completeness`, `schema-type-breadth`, `json-ld-entity-linking`,
`sitemap`, `sitemap-lastmod`, `llms-txt-exists`, `llms-txt-formatting`,
`openapi-spec`, `api-catalog-rfc9727`, `link-headers-discovery`,
`robots-agent-user-policy`, `robots-ai-policy-quality`, `pricing-md`,
`markdown-link-alternate`, `markdown-negotiation`,
`markdown-negotiation-vary`, `markdown-url-fallback`, `agent-ua-markdown`,
`public-api-docs`, `content-no-js` (the cards and links render without
scripts, and `/index.md` exists), `agent-discovery-file`, `ard-catalog`,
`ai-catalog-published`, `oauth-protected-resource`, `oauth-support`,
`mcp-oauth-metadata`, `auth-md-exists`, `agent-auth-discovery-metadata`,
`mcp-server-card`, `mcp-well-known-discovery`, `mcp-streamable-http` (the
public `/mcp/docs`), `mcp-pkce-s256`, `webmcp`; and with this change
`json-error-responses` and `agent-friendly-404`.

Still failing: `auth-md-structure` and `auth-md-walkthrough-simulation`
(the same agent-registration decision as `authMd`), `rate-limit-headers`
on pages, `web-bot-auth-directory` (not applicable), the payment checks
(#11085), and the listings outside this repository: `mcp-registry-listed`,
`wikipedia-presence`, `skills-sh-listed` (each needs an owner account or
an independent article).

### Decided not to add

- `/.well-known/ai-plugin.json`: the retired ChatGPT plugin manifest. No
  checker run above asks for it; MCP and the API catalog replace it.
- `/.well-known/agent.json`: A2A moved its card to `agent-card.json`
  (served, passing). The only live text that uses `agent.json` is the ADP
  individual draft, which no checker probes and which needs an Ed25519
  identity key and fingerprint for the site: adding one is an owner
  decision about which key speaks for OpenAgents. Revisit if a checker
  starts asking.

## Baseline: 2026-10-09 (production, before this work)

Production at the time served `openagents-web` revision `coder-web-…` with the
private `coder-serve` sidecar behind it (`docs/deployment/openagents-web.md`).
All scans were read-only.

| Checker | How it was run | Result |
| --- | --- | --- |
| [Is It Agent Ready](https://isitagentready.com/) (Cloudflare, 22 checks) | `POST https://isitagentready.com/api/scan {"url":"https://openagents.com"}` | **Level 0, "Not Ready"**: 2 pass, 14 fail, 6 neutral |
| [AgentReadyCheck](https://www.agentreadycheck.com/) | `GET https://www.agentreadycheck.com/api/check?url=https://openagents.com` | **0 / 100, "not-ready"**: 2 fail, 1 skip |
| [AG AgentReady](https://agagentready.com/) (WebMCP) | Its public MCP server, `tools/call run_website_scan` at `https://agagentready.com/mcp` | **Overall 33**: agent/WebMCP 42, revenue 26, trust 36; WebMCP not detected ([report](https://agagentready.com/report/46fbfadbc123926cb553bbd8c97d2b6896a2512cc0cd887a)) |
| [Readiness.sh](https://readiness.sh/) (124 checks) | No public score exists for the domain; a scan needs an account token or a Turnstile form, so its published catalog (`GET https://readiness.sh/api/v1/checks`, criteria set v27) was applied by hand below | Not scored |
| Goodie, AuditMySite, AgentReady by Zeo, AgentDots | No fetchable report or API (AuditMySite's deployment answers "Payment required"; AgentDots has no reachable API) | Not run |

### Is It Agent Ready: failed checks

| Check | Finding | Spec |
| --- | --- | --- |
| robotsTxt | `/robots.txt` 404 | [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309) |
| sitemap | `/sitemap.xml`, `/sitemap-index.xml`, `/sitemap_index.xml` 404 | [sitemaps.org](https://www.sitemaps.org/protocol.html) |
| linkHeaders | No `Link` header on `/` | [RFC 8288](https://www.rfc-editor.org/rfc/rfc8288), [RFC 9727 §3](https://www.rfc-editor.org/rfc/rfc9727#section-3) |
| dnsAid | No `_index._agents` / `_a2a._agents` / `_mcp._agents` SVCB or TXT records | [DNS-AID draft](https://datatracker.ietf.org/doc/draft-mozleywilliams-dnsop-dnsaid/) |
| markdownNegotiation | `Accept: text/markdown` on `/` returns `text/html` | [Markdown for Agents](https://developers.cloudflare.com/fundamentals/reference/markdown-for-agents/) |
| robotsTxtAiRules | No robots.txt | [RFC 9309](https://www.rfc-editor.org/rfc/rfc9309) |
| contentSignals | No robots.txt | [Content Signals](https://contentsignals.org/) |
| apiCatalog | `/.well-known/api-catalog` 404 | [RFC 9727](https://www.rfc-editor.org/rfc/rfc9727) |
| oauthDiscovery | `/.well-known/oauth-authorization-server` and `/.well-known/openid-configuration` 404 | [RFC 8414](https://www.rfc-editor.org/rfc/rfc8414) |
| oauthProtectedResource | `/.well-known/oauth-protected-resource` answers, but its `resource` is `https://openagents.com/mcp`, not the scanned origin, and it names no `authorization_servers` | [RFC 9728](https://www.rfc-editor.org/rfc/rfc9728) |
| authMd | `/auth.md` 404 | [auth.md](https://isitagentready.com/.well-known/agent-skills/auth-md/SKILL.md) |
| mcpServerCard | `/.well-known/mcp/server-card.json`, `/.well-known/mcp/server-cards.json`, `/.well-known/mcp.json` 404 | [SEP-1649](https://github.com/modelcontextprotocol/modelcontextprotocol/pull/2127) |
| webMcp | No tools registered through `document.modelContext` / `navigator.modelContext` | [WebMCP](https://webmachinelearning.github.io/webmcp/) |
| ard | `/.well-known/ai-catalog.json` 404, no `<link rel="ai-catalog">`, no `_catalog._agents` TXT | [ARD](https://agenticresourcediscovery.org/), [ai-catalog](https://github.com/Agent-Card/ai-catalog) |

Passed: a2aAgentCard ("OpenAgents decision API" v0.1.0) and agentSkills.
Neutral: webBotAuth, and the commerce checks x402, MPP, UCP, ACP, AP2 (the
scanner did not classify the site as commerce).

### AgentReadyCheck: failed checks

| Check | Finding | Spec |
| --- | --- | --- |
| llms | `/llms.txt` 404 | [llmstxt.org](https://llmstxt.org/) |
| openapi | `/openapi.yaml`, `/openapi.json`, `/swagger.json` 404 | [OpenAPI 3.1](https://spec.openapis.org/oas/v3.1.0) |
| mcp (skip) | No MCP endpoint advertised in llms.txt | [MCP](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports) |

### AG AgentReady: findings

WebMCP not detected. Prerequisites missing: Organization/Service structured
data, machine-readable contact details (`mailto:`), crawler access
(robots.txt, sitemap.xml), llms.txt, a named action on the homepage form.
Present: semantic structure (5 landmarks, one `h1`), a labelled form. Its
free findings (contact options, next step, contact details) are about the
homepage's visible copy and need owner decisions.

### Readiness.sh catalog, applied by hand

The required and recommended checks that fail, by layer (the catalog's ids):

- Access: `content-no-js` (the homepage is a composer with no text an agent
  can read), `json-ld`, `metadata-completeness` (no description, Open Graph,
  or canonical), `org-schema-completeness`, `schema-type-breadth`,
  `json-ld-entity-linking`, `sitemap`, `sitemap-lastmod`, `llms-txt-exists`,
  `llms-txt-formatting`, `openapi-spec`, `api-catalog-rfc9727`,
  `link-headers-discovery`, `robots-agent-user-policy`, `pricing-md`,
  `markdown-link-alternate`, `markdown-negotiation`,
  `markdown-negotiation-vary`, `markdown-url-fallback`, `agent-ua-markdown`,
  `public-api-docs` (the API docs were not linked from the homepage).
- Discovery: `robots-ai-policy-quality`, `agent-discovery-file`,
  `ard-catalog`, `ai-catalog-published`, `mcp-registry-listed`,
  `wikipedia-presence`, `skills-sh-listed`.
- Usability: `oauth-protected-resource` (resource mismatch),
  `oauth-support`, `mcp-oauth-metadata`, `auth-md-exists`,
  `auth-md-structure`, `agent-auth-discovery-metadata`, `mcp-server-card`,
  `mcp-well-known-discovery`, `mcp-streamable-http` (the only MCP endpoint
  needs a key before `initialize`), `mcp-pkce-s256`, `webmcp`.
- Payments: `x402-support`, `mpp-support`, `acp-support`, `ucp-support`,
  `ap2-support`.
- Passed: `a2a-agent-card`, `agent-skills-index-v2`, `agent-auth-www-authenticate`
  (`/mcp` answers `401` with `WWW-Authenticate: Bearer resource_metadata=…`),
  `cli-tool`, `ax-document-structure`, `ax-form-labeling`, `bot-detection`.

## What existed before this work

- `/.well-known/agent-card.json` and `/.well-known/agent-skills/index.json`
  with one skill (`crates/openagents-web/src/wellknown.rs`, from
  `crates/discovery`): they describe the decision API.
- `/mcp` and `/.well-known/oauth-protected-resource`, answered by the
  private `coder-serve` sidecar; `/mcp` needs an API key or session.
- `/docs` (31 guides) and, on `main` but not yet deployed, `/docs/api` with
  `.md` twins and `/docs/api/llms.txt` (#11069).
- The gateway's own discovery set (`crates/gateway/src/discovery.rs`:
  llms.txt, api-catalog.json, openapi.yaml for the decision API, robots.txt,
  sitemap.xml) is served on the gateway's host, not on openagents.com.

## After this work (on `main`, waiting for a site deploy)

Implemented in `crates/openagents-web` (`src/agent_ready.rs`, `src/docs_mcp.rs`,
`static/webmcp.js`, `content/agents/auth.md`,
`content/skills/openagents-api/SKILL.md`), all compiled in and tested in
`src/agent_ready_tests.rs`:

| Path | What it is |
| --- | --- |
| `/robots.txt` | AI search, training, and user-agent groups, each with `Content-Signal: search=yes, ai-input=yes, ai-train=yes`, private paths disallowed, `Sitemap:` and `Agentmap:` |
| `/sitemap.xml` | Every public page, guide, and API guide, with `lastmod` |
| `/llms.txt`, `/llms-full.txt` | Every guide and API guide; the agent documents |
| `Accept: text/markdown` | Any page with a document answers its Markdown twin, `Vary: Accept`, `x-markdown-tokens`; a missing page answers a Markdown 404 |
| `/index.md`, `/docs.md`, `/docs/{slug}.md`, `/docs/api.md`, `/terms.md`, `/privacy.md`, `/download.md`, `/pricing.md` | Markdown twins with frontmatter |
| `/pricing` | Redirects to `/docs/pricing` |
| `Link` headers, `<head>` | `api-catalog`, `service-doc`, `service-desc`, `describedby`, Markdown `alternate`; meta description, Open Graph, canonical, `rel=ai-catalog`, JSON-LD (`Organization`, `WebSite`, page type) |
| `/auth.md` | Getting and using an `oak_` key |
| `/.well-known/api-catalog` | RFC 9727 linkset for `api.openagents.com/v1`, `openagents.com/api/v1`, and the docs MCP server |
| `/.well-known/ai-catalog.json` | ARD catalog |
| `/mcp/docs` | Public Streamable HTTP MCP server: `list_docs`, `search_docs`, `read_doc`, `list_models`, guides as resources |
| `/.well-known/mcp/server-card.json`, `/.well-known/mcp.json` | Its server card |
| `/.well-known/agent-skills/index.json` | Adds the `openagents-api` skill |
| `/openapi.json` | The inference gateway's `/openapi.json`, once the gateway serves it |
| `/static/webmcp.js` | WebMCP tools on pages that run scripts (the homepage): the four docs tools and `start_chat` |

Checked locally (development server on a free port) with the checkers'
own requests: every path above answers 200 in its type, and `/` with
`Accept: text/markdown` answers `text/markdown`.

OAuth for `/mcp` (#11084), in `crates/openagents-web/src/oauth.rs`: this
site is its own authorization server (the GitHub sign-in, invite list
applied). `/.well-known/oauth-protected-resource` (the origin) and
`/.well-known/oauth-protected-resource/mcp` (RFC 9728 path form) name it;
`/.well-known/oauth-authorization-server` (RFC 8414) has PKCE `S256` only,
`registration_endpoint` (`POST /oauth/register`, RFC 7591, public clients,
stateless MAC'd `client_id`), and the `agent_auth` block (`skill`,
`register_uri`, registration methods); `/oauth/authorize` (consent page,
Approve and Deny) and `/oauth/token` issue the same 30-day app session a
device sign-in does, listed and removable in Settings. A `401` from `/mcp`
answers `WWW-Authenticate: Bearer resource_metadata="…/oauth-protected-resource/mcp", scope="account"`.
`/mcp` itself accepts these `sess_` bearers: `coder-serve`'s admission
(private `coder` repo, `bins/coder-serve/src/mcp/admission.rs`, set
`CODER_ACCOUNTS_URL` to the account service) asks the account service's
`GET /v1/session`, which now answers the account's GitHub `{id, login}`
(`crates/gateway/src/accounts.rs`); the person is keyed by GitHub id as
every other credential is, the invite list (`admits_login`) still applies,
and an answer is reused for 60 seconds, so Remove in Settings stops `/mcp`
within a minute.

Still open (see the issue): DNS-AID records, registry listings, agentic
commerce (#11085), `security.txt`, visible homepage copy, the A2A card's
subject, and `/openapi.json` until the gateway serves it.

Expected after deploy: Is It Agent Ready from level 0 to level 3 or 4
(robots, sitemap, Link headers, Markdown, AI rules, Content Signals, API
catalog, auth.md, MCP card, ARD, WebMCP pass; DNS-AID and OAuth discovery
still fail); AgentReadyCheck 2 or 3 of 3 (llms.txt and MCP pass; OpenAPI
once the gateway serves it).
