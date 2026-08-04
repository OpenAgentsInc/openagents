# X API and xAI X Search access (2026-08-04)

**Evidence labels:** repository observation for Omega crate placement; live
probe receipts for API access; external observation for any product claims
found on X.

## Short answer

**Yes — we can query X posts.** There are two separate, working paths with the
credentials recorded in the owner private note (not in this repo):

| Path | What it is | Credential | Host | Status on 2026-08-04 probe |
| --- | --- | --- | --- | --- |
| **X API v2** | Direct REST for posts, users, recent search, streams, etc. | App Bearer (`X_BEARER_TOKEN`) | `https://api.x.com` | Green: user lookup, recent search, official account timeline |
| **xAI Grok API + `x_search`** | Model tool that searches X (keyword, semantic, user, thread fetch) | xAI API key (`XAI_API_KEY`) | `https://api.x.ai` | Green: Responses API with `tools: [{ "type": "x_search" }]` ran 10 X tool calls and returned a synthesis |

These are **not** the same product surface. Buying X API credits can grant
promotional xAI credits ([X pricing tip](https://docs.x.com/overview)); the
APIs and auth remain independent.

## What the X docs say

Reviewed:

- <https://docs.x.com/overview>
- <https://docs.x.com/x-api/posts/lookup/introduction>
- <https://docs.x.com/x-api/posts/search/introduction>
- Docs index: <https://docs.x.com/llms.txt>

Capabilities relevant to us:

- **Post lookup** — single ID or up to 100 IDs; expansions for author/media.
- **Recent search** — last 7 days; query operators (`from:`, `-is:retweet`,
  `lang:`, phrases, hashtags).
- **Full-archive search** — back to 2006; pay-per-use / enterprise.
- **Filtered stream** — near real-time rules.
- **Users, Spaces, DMs, Lists, Trends** — documented; DMs need user context.

Pricing is pay-per-usage / credits via <https://console.x.com>.

## What the xAI docs say

- Grok inference: <https://docs.x.ai/docs>
- X Search tool: <https://docs.x.ai/developers/tools/x-search>
- Tool pricing (observed docs): X Search about **$5 / 1k calls**

`x_search` is a **server-side tool** on the Responses / chat APIs. The model
decides which keyword/semantic/thread tools to call; you do not get a pure
operator-string recent-search envelope unless the model chooses that shape.

Example:

```bash
curl https://api.x.ai/v1/responses \
  -H "Authorization: Bearer $XAI_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "grok-4.5",
    "input": "What are people saying about Coldcard on X?",
    "tools": [{ "type": "x_search" }]
  }'
```

Models available on the probed key included `grok-4.5`, `grok-4.3`,
`grok-4.20-*`, `grok-build-0.1`, and Imagine image/video models.

## Live probe notes (no secrets)

1. **Bearer form matters.** The owner note’s Bearer string worked **as stored**
   (including percent-encoding characters) against `api.x.com`. A manually
   URL-decoded form returned `401` in one path. Always use the exact console
   token; regenerate in console if unclear.
2. **OAuth2 client-credentials** against `api.twitter.com/oauth2/token` with
   consumer key/secret also issued a bearer (redacted). Prefer the console
   app Bearer for operators.
3. **Recent search** for Coldcard returned pages of English posts
   (`result_count: 25` per page) with `next_token` pagination.
4. **Official** `from:COLDCARDwallet` and Coinkite/nvk queries returned.
5. **xAI `x_search`** completed with server-side tool usage
   `x_search_calls: 10` and a multi-claim synthesis with X URL citations.

Never commit keys, consumer secrets, or bearer tokens. Store them outside the
repo (owner private note, shell env, or a future secret store).

## Omega fold-in

Rust tooling landed in the Omega workspace:

- crate: `~/work/omega/crates/x_api`
- CLI: `cargo run -p x_api -- …`
- design doc: `~/work/omega/docs/src/development/omega-x-api.md`

Env:

```sh
export X_BEARER_TOKEN='…'   # X API app-only
export XAI_API_KEY='…'      # Grok / x_search (separate)
```

Examples:

```sh
cargo run -p x_api -- user COLDCARDwallet
cargo run -p x_api -- search 'coldcard -is:retweet lang:en' --max-results 25
cargo run -p x_api -- post 2084632863756955661
```

Existing `crates/x_ai` remains the **model provider** surface (Grok chat
models in the agent panel). It does not replace direct X API post lookup.

## When to use which path

- **Exact IDs, metrics, operator queries, reproducible monitoring** → X API
  (`x_api` crate).
- **Agent synthesis / “what is the discourse”** → xAI `x_search` (or both:
  search with X API, summarize with Grok without tools).
- **Forensic or publication claims** → treat X text as **external
  observation** until independently verified (see `docs/coldcard/`).

## Related coldcard summary

Live recent-search snapshot from this session:

[`docs/coldcard/2026-08-04-x-posts-recent-summary.md`](../coldcard/2026-08-04-x-posts-recent-summary.md)
