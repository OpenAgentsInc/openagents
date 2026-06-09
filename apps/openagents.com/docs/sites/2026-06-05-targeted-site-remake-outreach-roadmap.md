# Targeted Site Remake And Outreach Roadmap

Date: 2026-06-05

Status: product and implementation planning note. This document does not run
Exa, crawl a website, copy a third-party site, send email, create GitHub
issues, or change runtime policy by itself.

## Sales Prompt

The sales idea is to automate an AI that can look at websites, especially
small professional-service sites such as law firms, decide whether they are
simplistic or old, build a better version using public images and copy from
the site, then email the firm a preview link and a meeting link.

This should become part of the Sites plan in two layers:

1. A short-term internal operator tool. Given a target website or target list,
   OpenAgents product surface captures public site material, audits the site, drafts an improved
   concept Site, and lets an operator review before any outreach email.
2. A longer-term agent toolkit. Users and their agents can run scoped prospect
   discovery, site audit, remake preview, outreach, and conversion workflows
   as approved campaigns. This is the beginning of a future "agent sales
   army" or freelancer-style revenue workflow, but only after safety,
   compliance, suppression, payment, and accepted-outcome controls exist.

## Recommendation

Build our own first-party capture and remake pipeline on Cloudflare for the
normal case, and keep third-party services as optional adapters, fallbacks,
and benchmarks.

The normal case should not need a heavy external scraper:

- Exa can identify and enrich prospect targets.
- Workers can do cheap static fetches, sitemap/robots reads, link extraction,
  and asset graph normalization.
- Browser Run Quick Actions can capture rendered content, screenshots,
  markdown, links, structured JSON, and scoped crawls.
- Queues or Workflows can orchestrate long-running capture/audit/remake work.
- D1 can store prospects, capture runs, source authority, audit scores,
  preview refs, outreach refs, and suppression state.
- R2 can store captured screenshots, public assets, normalized source packs,
  generated Site artifacts, and review snapshots.
- Workers for Platforms and the existing Sites runtime can serve preview
  concepts cheaply.

Third-party providers should be adapters, not authority:

- Firecrawl is useful for fast URL-to-markdown/html/images/screenshot/branding
  extraction and for comparing our extraction quality.
- Browserless is useful when we need managed Puppeteer, Playwright, Selenium,
  REST screenshot/scrape/PDF APIs, or a self-hostable browser pool.
- Browserbase is useful for AI-native browser sessions, page fetch, search,
  Stagehand, and high-concurrency agent browsing experiments.
- Apify is useful for off-the-shelf Actors, scheduled extraction, and
  marketplace scrapers where we do not want to maintain a vertical-specific
  collector.

The product should not become a bot-protection bypass service. If a target
blocks crawling, requires login, presents CAPTCHA/Turnstile, or disallows
capture through robots or crawler policy, the capture run should become
`blocked`, `needs_owner_permission`, or `manual_review`, not an evasion task.

## Why Not Just Use A Third-Party Scraper

A third-party service is the quickest way to validate extraction quality, but
it is the wrong long-term source of truth for OpenAgents product surface:

- We already need source authority, source cards, public-safe receipts,
  customer-safe projections, preview artifacts, suppression, and email
  ledgers. Those records belong in OpenAgents product surface.
- We need cost-tiered behavior. A static site should cost nearly nothing to
  inspect. A rendered page should pay Browser Run browser-time only when
  needed. A high-value, hard target can use a paid external provider only when
  an operator or customer accepts the cost.
- We need strict policy semantics. The crawler result must record why a page
  was allowed, skipped, blocked, stale, manually added, or third-party
  sourced.
- We need agent-facing APIs later. User agents should not be handed raw
  provider-specific payloads as the product contract.

The right approach is a provider-neutral capture boundary:

```text
target prospect
-> discovery refs
-> capture policy decision
-> static capture attempt
-> rendered Browser Run attempt when needed
-> optional third-party fallback when approved
-> source authority pack
-> audit score
-> remake brief
-> generated preview
-> operator review
-> outreach or skip
```

## First-Party Capture Stack

### Discovery

Use Exa for target identification and enrichment:

- find law firms or other verticals by geography, size, practice area,
  technology signal, ranking, or weak website signal;
- enrich known targets with site URLs, snippets, likely pages, and source
  cards;
- dedupe against CRM contacts, suppression records, previous outreach,
  active customers, and blocked domains.

Exa should select prospects and provide discovery evidence. It should not be
the only capture source for assets, screenshots, or full-page source packs.

### Static Capture

Start cheap:

- fetch `/robots.txt`, sitemaps, homepage, and selected internal pages;
- use strict URL normalization and same-origin asset graph tracking;
- use `HTMLRewriter` or structured HTML parsing to extract titles, metadata,
  headings, CTAs, links, images, logos, OpenGraph images, service pages,
  attorney/team pages, contact data, schema.org data, and basic technical
  markers;
- store normalized HTML, extracted text, link graph, asset manifest, source
  hashes, response headers, and failure reasons;
- avoid downloading large media by default; store refs and fetch thumbnails or
  review-safe assets only when needed.

This is where custom Rust can be useful later: fast URL normalization,
MIME/type sniffing, sitemap parsing, HTML extraction, dedupe, image hashing,
archive packing, and batch analysis. Rust is less compelling as the primary
browser runner; keep browser execution in Browser Run or a managed browser
pool unless a very specific cost or control case appears.

### Rendered Capture

Escalate to Cloudflare Browser Run when static capture is insufficient:

- `/snapshot` for rendered HTML plus screenshot;
- `/content` for rendered HTML;
- `/markdown` for clean page text;
- `/links` for rendered link discovery;
- `/json` for structured extraction when a schema is known;
- `/crawl` for bounded multi-page capture with depth, page limits, and
  include/exclude paths;
- Puppeteer, Playwright, CDP, or Stagehand for multi-step pages, responsive
  checks, consent banner handling, or screenshot comparison.

Use Browser Run costs as a metered line item. Browser Run Quick Actions are
priced by browser hours, while full browser sessions add concurrency cost.
The pipeline should record `browserMsUsed` or equivalent usage data whenever
the provider returns it.

### Containers And Heavy Runners

Containers should not be the default for prospect capture or preview. They make
sense only when the job requires:

- custom Chromium or Playwright dependencies that Browser Run cannot provide;
- long-running crawl state or heavyweight post-processing;
- sidecar models, OCR, image analysis, or PDF extraction that does not fit a
  Worker;
- high-value targets where the user approved paid capture;
- customer-owned sites where the customer has explicitly permitted deeper
  capture;
- repeatable benchmark runs where environment control matters.

For internal sales experiments, pass this cost to the campaign budget. For
user-owned agent campaigns, surface it as quote approval, credits, or a
recoverable `402` path before the heavy runner starts.

## Audit And Remake Model

The site audit should classify:

- outdated visual design;
- mobile/responsive problems;
- poor information architecture;
- weak local SEO and metadata;
- missing or unclear CTAs;
- missing trust signals such as reviews, attorney bios, credentials, case
  results where appropriate, office location, and consultation flow;
- low-quality hero imagery or generic stock feel;
- page speed and basic Core Web Vitals risk signals;
- accessibility issues detectable from static/rendered HTML;
- stale copyright dates, broken links, mixed content, and CMS markers;
- legal-sensitive claims that require human review.

The remake brief should include:

- source refs for every reused image or copied text span;
- a public-safe source authority pack;
- captured screenshots of the original site;
- a concise diagnosis of what the preview improves;
- a generation plan for a concept Site;
- constraints that prevent deceptive, defamatory, or unauthorized claims.

For law firms, never generate legal advice, unverifiable success claims, fake
reviews, fake attorney credentials, fake office locations, or misleading
guarantees. The preview should be a concept, not a representation that the
firm endorsed or already operates the Site.

## Preview And Outreach

The preview URL should make the concept nature clear. Use an OpenAgents-owned
preview domain and avoid domain impersonation:

```text
https://sites.openagents.com/concepts/<campaign>/<target-slug>
```

Outreach must run through the typed `EmailService` boundary:

- no direct ad hoc SMTP sends;
- every send has idempotency, campaign ref, target ref, preview ref, template
  version, suppression check, unsubscribe link, and redacted delivery attempt;
- outreach only starts after operator approval for v0;
- the email should include the preview link, short value proposition, meeting
  link, sender identity, postal/contact information where required, and
  unsubscribe/preference controls;
- bounces, spam complaints, unsubscribes, and replies must update campaign
  state before another send.

This lane depends directly on the open email issues:

- `OPENAGENTS-O-005` through `OPENAGENTS-O-010` for templates, drip/campaign tables,
  suppression, scheduler, and webhook ingestion.
- `OPENAGENTS-SITES-REV-004` for safer stable Site revision activation.

## Agent Toolkit Direction

After the internal operator lane works, expose scoped tools for users and
agents:

- create a prospect campaign;
- add target URLs or request Exa target discovery;
- set geography, vertical, budget, daily send cap, and do-not-contact rules;
- run dry-run capture and audit;
- approve remake brief;
- generate preview;
- approve outreach;
- track opens, clicks, meetings, replies, conversions, and accepted outcomes;
- delegate tasks to owned agents with spend caps, scopes, and receipts.

The future "freelancer army" path should be modeled as accepted outcomes, not
as unbounded autonomous selling. Agents can propose leads, generate previews,
draft outreach, follow up, and request rewards, but humans or organizations
remain accountable owners and acceptance authorities.

## Issue Plan

Short-term internal operator lane:

| ID | Title | Outcome |
| --- | --- | --- |
| OPENAGENTS-SITES-OUTREACH-001 | Add targeted Site campaign and prospect schema | Implemented in #181. Campaigns and prospects are durable in D1 with deduped domains, public-safe contact refs, suppression/capture/review states, source refs, confidence, budget refs, owner/operator refs, and repository tests. |
| OPENAGENTS-SITES-OUTREACH-002 | Add Exa-backed prospect discovery planner | Implemented in #182. Operators can plan bounded Exa company searches by vertical/geography/signal, review public-safe source cards, run dry-run discovery, and persist deduped prospect candidates through the #181 repository. |
| OPENAGENTS-SITES-OUTREACH-003 | Add respectful capture policy and robots/suppression gates | Implemented in #183. Capture policy events classify allowed, disallowed, blocked, manual-review, customer-owned, suppressed, and paid-escalation states; only allowed and paid-escalation records are fetchable, and projections redact suppression/contact/provider/payment/bypass material. |
| OPENAGENTS-SITES-OUTREACH-004 | Add static site capture and asset graph service | Implemented in #184. Static capture runs require a fetchable #183 policy event, normalize same-origin homepage/page/asset refs, bound source-pack metadata and response summaries, and project only public/operator-safe capture state. |
| OPENAGENTS-SITES-OUTREACH-005 | Add Browser Run rendered capture service | Implemented in #185. Rendered capture runs require a fetchable #183 policy event, optionally link to static capture, store Browser Run-style output refs and bounded usage summaries, and block bot-protection/login-wall output capture. |
| OPENAGENTS-SITES-OUTREACH-006 | Add capture provider adapter boundary | Implemented in #186. Provider adapter runs record first-party Worker, Browser Run, Firecrawl, Browserless, Browserbase, Apify, or Container fallback/benchmark refs, require fetchable capture policy, require paid-escalation evidence for paid fallback, and redact raw payloads. |
| OPENAGENTS-SITES-OUTREACH-007 | Add website quality audit scorer | Implemented in #187. Quality audits record bounded scores for design age, mobile risk, IA, SEO, CTA, trust, image quality, accessibility, performance, content, stale/broken/mixed-content, and legal-sensitive claims, with evidence refs and manual-review routing. |
| OPENAGENTS-SITES-OUTREACH-008 | Add remake brief and source authority pack | Implemented in #188. `targeted_site_remake_briefs` records reviewable source authority packs, original screenshot refs, copied text/image refs, audit findings, and concept-only generation constraints before preview generation. |
| OPENAGENTS-SITES-OUTREACH-009 | Add targeted remake preview generation | Implemented in #189. `targeted_site_remake_preview_generations` records concept preview URLs, generated artifact/source refs, candidate Site/version refs, source authority pack refs, generation receipts, and concept-domain guardrails for approved briefs. |
| OPENAGENTS-SITES-OUTREACH-010 | Add internal operator review UI for targeted remakes | Implemented in #190. `targeted_site_operator_review_events` records review decisions and `targeted-site-operator-review.ts` builds a UI-ready operator model with capture refs, audit score, source authority count, preview state, outreach/meeting readiness, suppression state, disabled-action reasons, and redacted projections. |
| OPENAGENTS-SITES-OUTREACH-011 | Add typed targeted-remake outreach email | Implemented in #191. `TargetedRemakeOutreachEmailInput` renders concept-preview outreach with preview, meeting, sender, postal/contact, unsubscribe, and preferences links, and `targeted_site_remake_outreach_email_dispatches` records operator-approved EmailService dispatches with suppression and idempotency gates. |

Later user/agent campaign lane:

| ID | Title | Outcome |
| --- | --- | --- |
| OPENAGENTS-SITES-OUTREACH-012 | Add campaign metrics and conversion ledger | Implemented in #206. `targeted_site_campaign_metric_events` records idempotent public-safe campaign metric events and derives aggregate projections for capture cost, previews, sends, bounces, replies, meetings, conversions, accepted outcomes, refunds, complaints, suppression, and blocked states. |
| OPENAGENTS-SITES-OUTREACH-013 | Expose scoped agent toolkit for user-owned campaigns | Implemented in #207. `targeted_site_agent_toolkit_grants` and `targeted_site_agent_toolkit_actions` record private scoped agent grants and idempotent actions with dry-run defaults, scopes, spend caps, daily send caps, suppression, approval gates, receipt refs, and public-safe action projections. |
| OPENAGENTS-SITES-OUTREACH-014 | Add accepted-outcome reward policy for sales agents | Implemented in #208. `targeted_site_sales_reward_policy_events` records proposed leads, accepted meetings/customers, reward eligibility, payout intent, holds, disputes, complaints, refunds, reversals, settlement caveats, public receipt refs, and public-safe projections while keeping buyer payment, referral attribution, accepted work, payout intent, and settlement separate. |

## Source Links

- Cloudflare Browser Run Quick Actions:
  `https://developers.cloudflare.com/browser-run/quick-actions/`
- Cloudflare Browser Run `/crawl` endpoint:
  `https://developers.cloudflare.com/browser-run/quick-actions/crawl-endpoint/`
- Cloudflare Browser Run pricing:
  `https://developers.cloudflare.com/browser-run/pricing/`
- Cloudflare web crawler with Queues and Browser Run:
  `https://developers.cloudflare.com/queues/tutorials/web-crawler-with-browser-run/`
- Firecrawl scrape endpoint:
  `https://docs.firecrawl.dev/api-reference/v2-endpoint/scrape`
- Browserless introduction:
  `https://docs.browserless.io/overview/intro`
- Browserbase introduction:
  `https://docs.browserbase.com/introduction/getting-started`
- Apify Actors:
  `https://docs.apify.com/platform/actors`
