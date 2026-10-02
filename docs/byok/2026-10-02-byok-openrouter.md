# BYOK: run everything through the user's own OpenRouter key

Status: design, 2026-10-02. Nothing here is implemented yet. Tracking issue:
[#10176](https://github.com/OpenAgentsInc/openagents/issues/10176).

Owner request (2026-10-02, verbatim): "I want to be able to have an open router
API key mode, a B.Y.O.K. mode. We need to generally flesh out B.Y.O.K. stuff. I
want users to be able to easily add their own OpenRouter API key, and there
should be a flag you can pass at CLI time, or also have an in-app settings
screen for it. I want to be able to add an OpenRouter API key. If they do, they
can opt to have everything run through that key. If they provide that key,
everything runs through OpenRouter and not ours. The decision models and
language models should use their stuff instead of ours."

## 1. What BYOK means

A person adds their own OpenRouter API key. With **"Use my key for
everything"** on, every model call OpenAgents makes on that person's behalf goes
through OpenRouter on their key, and none goes on ours. That covers chat
replies, Jev (System One) decisions, Microcoder, embeddings, and judges.

Coding agents that already run under the person's own logins are unchanged:
Claude Code, Codex, Grok Build, OpenCode, and Devin. Their providers bill the
person directly today, and BYOK does not touch them.

BYOK is always an explicit choice. An `OPENROUTER_API_KEY` that happens to be in
a shell environment never switches anyone to their own key. The Khala Code
incident ([#7955](https://github.com/OpenAgentsInc/openagents/issues/7955))
showed why. An ambient developer key was silently forwarded as BYOK, the
account had no credits, and hosted chat broke with a 402 that pointed somewhere
else.

## 2. Where we pay today

Every place below spends our keys or our accounts. Paths are relative to the
repository root.

| Call site | What it does | Model today | Door and key today |
| --- | --- | --- | --- |
| Chat worker, primary (`crates/coder/src/generate.rs` `Lane::SpaceBunny`, `FallbackDoor::openrouter`; `crates/coder/src/bin/coder-worker.rs`) | Chat replies for the phone, the desktop, `openagents chat`, the terminal | `stealth/space-bunny-alpha`, reasoning low (free on OpenRouter; retired 2026-10-05) | OpenRouter, our `OPENROUTER_API_KEY` on oa-coder-worker-1 |
| Chat worker, fallback (`generate.rs` `Lane::Gemini`) | Any turn the primary misses | `google/gemini-3.8-flash` | Vercel AI Gateway, our `CODER_DOOR_KEY` |
| Website Ask box (`crates/openagents-web/src/ask.rs`) | Homepage questions | Same chat worker, through the relay | Ours (the site signs each visitor's jobs with a derived key) |
| Chat router judge (`coder-worker.rs`, `crates/coder/src/router/judge.rs`) | Route, prepared answer, opener, lane, risk, command group, tool, capability | Jev | Gateway `typesafe-ai/jev`, then OpenRouter `typesafe/jev-1.13`, then TypeSafe (`crates/jev/src/doors.rs`, `Failover::primary_last`) |
| Personalization (`crates/coder/src/router/personalize.rs`) | Finishes "We'll dispatch Coder to ..." | `google/gemini-2.5-flash-lite` | OpenRouter, our key |
| Product and codebase knowledge (`crates/coder/src/product_kb.rs`, `codebase.rs`, `crates/knowledge/src/search.rs`) | Embeds the query for retrieval | `openai/text-embedding-3-small` | Gateway, our `CODER_DOOR_KEY` |
| Hosted decision service (`crates/jev-hosted`, `deploy/systemd/decision-worker.service`, `docs/deployment/decision-worker.md`) | Jev for every computer with no TypeSafe key: the delegate door, issue flow and issue gate checks (`crates/coder-delegate/src/issue.rs`), Coder classify/select/cli_route, Microcoder's repository judge, Gym, Voyager | Jev | Same three doors, our keys, on the decision worker |
| Microcoder's cloud provider (`crates/coder/src/delegate_door.rs` `targets`, `Provider::Vertex`; `crates/microcoder` `--provider door`) | Microcoder steps when the person has no Codex login | OpenAgents cloud | Ours |
| Plugin eval graders (`crates/ext-eval/src/door.rs`, `crates/eval-runner/src/config.rs`) | `decision` and `judge` graders | Jev; `google/gemini-3.8-flash` | Gateway, our key when run on our hosts |
| Knowledge harvest proposer (`crates/knowledge/src/harvest.rs`) | Proposes entries | `openai/gpt-6-luna` | Codex login or `OPENROUTER_API_KEY` |
| Simulated QA (`scripts/qa/simulated_users.py`) | Persona and rubric judge | `stealth/space-bunny-alpha` | Our OpenRouter key; an internal tool, out of BYOK scope |

Already the person's own and unchanged by BYOK: Claude Code, Codex, Grok Build,
OpenCode, and Devin runs (`crates/microcoder`, `crates/acp-client`), and
Microcoder on the person's Codex login (`crates/microcoder-loop`, `gpt-6.1-sol`
through `~/.codex/auth.json`). Background rules call no model
(`crates/background/src/lib.rs`). Lev, Kev, and Laya run locally and cost
nothing per call. Fable appears only as a recorded baseline.

## 3. Setting it up

### What the person sees

One setting with three plain parts:

- **OpenRouter key:** added, or none.
- **Use my key for everything:** on or off.
- **Status line**, one of:
  - "Running on OpenAgents."
  - "Running on your OpenRouter key."
  - "Your OpenRouter key was refused; nothing is running on ours."

There are three modes. The setting key is `models.payer`.

| Mode | Meaning |
| --- | --- |
| `ours` (default) | Today's behaviour. A stored key is kept but not used. |
| `mine` | Every model call in section 2 goes through OpenRouter on the person's key. Nothing falls back to ours. A failure says so in one line (section 6). |
| `mine_then_ours` | The person's key first. A call that fails for the key's own reasons (401, 402, 429, no connection) falls back to ours, and the status line says it did. The person turns this on explicitly. It is never the default for someone who added a key. |

Adding a key does not switch the mode on its own. The add flow then asks one
question: "Use this key for everything?" Answering yes sets `mine`.

### Command line

The new commands follow the `openagents settings` tree in
`crates/openagents-cli/src/settings.rs`.

```text
openagents settings openrouter-key set      Read the key from the prompt or stdin (never argv), test it, store it.
openagents settings openrouter-key show     Say whether a key is stored, its last four characters, its label, and whether it works.
openagents settings openrouter-key test     Test the stored key now.
openagents settings openrouter-key clear    Remove the key; the mode returns to ours.
openagents settings set models.payer mine|mine_then_ours|ours
```

There is also a per-invocation form:

- `--openrouter-key KEY` is a global flag, parsed like `--json` in
  `crates/openagents-cli/src/main.rs`. It lasts for that one command, means
  `mine`, and is never stored.
- `OPENAGENTS_OPENROUTER_KEY` is the environment-variable form of the same
  thing. It is distinct from the ambient `OPENROUTER_API_KEY` on purpose
  (section 1).

The help text warns that a key in argv shows up in shell history and `ps`, and
recommends `set` or the environment variable instead. `openagents terminal`,
`openagents chat`, and bare `openagents` all take the flag.

The `openrouter-key` commands are declared `Effect::Secret`, like `wallet
export`. The chat router never proposes a secret command (INVARIANTS.md), so no
chat turn can ask for or carry a key.

### Screens

- **Terminal `/settings`** (`crates/openagents-terminal/src/lib.rs`
  `Settings`/`Choice`). Its choices are on/off only today. It needs one new
  input kind: a masked secret field with paste. The mode is a three-way choice.
- **Desktop** (`crates/openagents-desktop/src/settings.rs`). A new "Models"
  pane next to Coder holds the key field (masked; paste; Test; Remove), the
  mode, the status line, and a "Get a key at OpenRouter" link.
- **Phone.** There is no Settings screen today. The same section goes on the
  Account tab (`crates/openagents-mobile/src/account.rs`).
- **Easiest add.** A "Connect OpenRouter" button using OpenRouter's OAuth PKCE
  flow, which returns a key the person controls. They then never copy or paste
  a key. The desktop opens the browser to a loopback callback, and the phone
  uses its URL scheme. Pasting stays available everywhere.

### Validation

A key is tested when it is added and when the person taps Test. The test is
`GET https://openrouter.ai/api/v1/key`, which costs nothing and returns the
key's label and credit state. A key that answers 401 is not stored, and the
person sees "OpenRouter didn't accept that key." A key with no credits is
stored with the warning "This key has no credits; calls on it will fail."

### Storage

The key is the person's money, so it gets the same care as the keys we already
hold:

- **Desktop host and CLI.** The OS keychain, through the existing `KeySource`
  in `crates/openagents-connect` / `crates/coder-host/src/serve/keys.rs`
  (service `com.openagents.desktop`, new account `openrouter-key`). That is
  Keychain on macOS, Secret Service on Linux, and Credential Manager on
  Windows. Where no keychain answers (headless Linux, a CLI-only install), the
  key goes in `~/.openagents/openrouter.json`, mode 0600 in a 0700 directory,
  written through `coder::private`. That file is the one
  `openrouter::Config::from_env` already reads.
- **iPhone.** A this-device-only Keychain item, as the wallet seed is.
- **Android.** Encrypted under its own Android Keystore key, in app-private
  storage with no backup.
- **The mode.** `models.payer` lives in `~/.openagents/settings.json` under a
  new `models` section. It is never the key.
- **Never shown.** The key never appears in a log line, an error, a usage
  record, a trace, a crash report, `--json` output, or a Debug string.
  `openrouter::ApiKey` already redacts itself. Records carry only a
  fingerprint: the first 8 hex characters of the key's SHA-256 digest.

## 4. One place decides who pays

A new small crate, `crates/model-access`, answers one question for every call
site: **who pays for this call, and through which door?** Call sites stop
reading `OPENROUTER_API_KEY`, `CODER_DOOR_KEY`, and `AI_GATEWAY_API_KEY`
themselves and ask it instead.

```rust
pub enum Payer { Ours, Theirs { key: openrouter::ApiKey, fingerprint: String } }
pub enum Mode { Ours, Mine, MineThenOurs }

pub struct Access { mode: Mode, theirs: Option<openrouter::ApiKey> }

impl Access {
    /// The door for a chat model, a decision, or an embedding: ours, or the
    /// person's OpenRouter key, per mode. Mine never returns one of ours.
    pub fn chat(&self, want: Use) -> Result<Doors, NoDoor>;
    pub fn decisions(&self) -> Result<jev::doors::Failover, NoDoor>;
    pub fn embeddings(&self) -> Result<knowledge::search::Embedder, NoDoor>;
}
```

The rules:

- **`mine`** builds doors that hold only the person's key. The chat door is
  OpenRouter Responses with the OpenRouter model ID. The Jev door is a
  `Failover` with the OpenRouter door alone, so there is no gateway or
  TypeSafe fallback on our keys. The embedder is `Embedder::openrouter`.
- **`mine_then_ours`** builds the same doors with ours behind them, using the
  existing `FallbackDoor` and `Failover` machinery.
- **The hosted decision service** (`jev_hosted::resolve`) is skipped under
  `mine` and `mine_then_ours`. The computer asks OpenRouter's Decisions API
  directly with the person's key. This is the cheapest and simplest piece,
  because the computer already runs Jev locally whenever it holds a key.
- **Which surfaces use it.** Each surface (the desktop host, `openagents chat`,
  the terminal, Microcoder, the delegate door, plugin evals run locally) builds
  one `Access` at start from the settings and the flag, then passes it down.
  Nothing chooses a door from message text. The mode is a typed setting, and
  routing stays the router's typed judgment.

### Model IDs on OpenRouter

Checked against OpenRouter's public catalog (`GET /api/v1/models`,
`/api/v1/embeddings/models`) on 2026-10-02.

| Use | Ours today | On the person's OpenRouter key | If it's not on OpenRouter |
| --- | --- | --- | --- |
| Chat primary | `stealth/space-bunny-alpha` (OpenRouter) | Same ID until 2026-10-05, then the chat's next primary | After retirement, `google/gemini-3.8-flash` |
| Chat fallback | `google/gemini-3.8-flash` (Gateway) | `google/gemini-3.8-flash` (listed) | — |
| Personalization | `google/gemini-2.5-flash-lite` (OpenRouter) | Same (listed) | — |
| Jev | `typesafe-ai/jev` (Gateway) / `typesafe/jev-1.13` (OpenRouter) / `jev-1.13.0` (TypeSafe) | `typesafe/jev-1.13` at `POST /api/alpha/decisions` | Under `mine`, the decision fails with its one-line message; nothing falls back to ours |
| Embeddings | `text-embedding-3-small` (Gateway, OpenAI) | `openai/text-embedding-3-small` (listed); the same vectors, so the cache and indexes stay valid | — |
| Microcoder cloud | OpenAgents cloud (Vertex) | `openai/gpt-6.1-sol` (listed; `gpt-6-sol` for hard tasks) | — |
| Knowledge harvest | `openai/gpt-6-luna` | Same (listed) | — |
| Eval `judge` graders | `google/gemini-3.8-flash` (Gateway) | Same, on OpenRouter | — |

Notes on the table:

- The Jev route is OpenRouter's `/alpha/` Decisions API. It is not in the chat
  model catalog, which lists only `typesafe/jev-router`. Phase 1 confirms that
  a fresh, non-owner key can call `typesafe/jev-1.13` (open question 1).
- A model the person's key cannot call is never quietly swapped for a
  different model on our key. Under `mine` the call fails and says which model
  is missing. Under `mine_then_ours` that one call goes to ours and is recorded
  as ours.
- Gym and benchmark runs pin door identity (`docs/gym/regression.md`). A run
  made on the person's key records `payer: theirs` and the OpenRouter door, so
  it is never compared as if it had used our gateway door.

## 5. The hosted chat

The chat runs on our worker (oa-coder-worker-1), not on the person's computer:

- the phone has no computer,
- the router, bank, knowledge index, and question sets live on the worker, and
- the website's Ask box goes there too.

For the chat to run on the person's key, either the key goes to the worker for
their jobs, or the model calls move off the worker.

| Option | How | For | Against |
| --- | --- | --- | --- |
| **A. Key with each job** (recommended) | The client puts the key in a payer envelope inside the NIP-44-encrypted job. The worker uses it for that job's model, personalization, Jev, and query embedding, then drops it. | Works on the phone, desktop, terminal, and CLI alike. One worker. The key is never at rest on our side. Revoking is "clear it in the app". | The key exists in the worker's memory during the job. People have to trust our open-source worker not to keep it. NIP-CJ's "Payloads contain no bearer credential" rule needs an amendment. |
| B. Key registered with the worker | The person registers the key once. The worker stores it encrypted, keyed by their signing key, and uses it for every job they sign. | Fits NIP-CJ as written: the signer maps to policy outside the payload. Smaller jobs. | Many people's keys at rest on our server, which is a standing breach target. Needs a register/rotate/delete protocol. A stored key outlives the person's intent. |
| C. Model calls on the person's device | The worker returns the routed plan (instructions, retrieved context, judgment) and the client calls OpenRouter itself. | The key never leaves the device. | Jev's judgment and the retrieval embedding still need a key on the worker, or the router moves to the client. That splits the chat into two implementations and doubles phone traffic. |

**Recommendation: A.** The key goes with each job, under four rules.

1. **A declared feature.** The job's `requires` gains `payer.openrouter`.
   NIP-CJ already says a worker refuses a body with features it doesn't know.
   An older worker therefore refuses the turn rather than silently answering
   it on our key. The client says "OpenAgents chat can't use your key yet",
   with no fallback under `mine`.
2. **A separate envelope.** The key travels as `payer: {kind: "openrouter",
   key: <NIP-44 ciphertext to the worker>}`, encrypted a second time apart
   from the rest of the body. The decrypted job `Value` never holds the
   plaintext, so a stray debug dump of the request cannot leak it. The worker
   decrypts it into an `ApiKey` held only for that job and zeroed after.
3. **It pays; it grants nothing.** The payer credential pays for the job's
   model calls. It never widens admission, delegation, or execution: an open
   caller still gets conversation jobs only. The usage log gets
   `payer: "theirs"` and the fingerprint, never the key.
4. **Policy.** The NIP-CJ amendment states this exception: a caller-paid
   provider credential that is not a grant. The same change adds a matching
   row in INVARIANTS.md with tests:
   - no key in any log, usage line, or result,
   - the old worker refuses,
   - `mine` never touches our doors,
   - the key is not retained after the job.

People who want their key never to leave their own machine can run their own
worker. Self-hosting is already in the [API design](../api/2026-10-02-openagents-api.md).
Option C can come later for the desktop and terminal, where the computer can
run the whole router locally.

The website's Ask box stays on ours. Visitors have no settings, and a key
pasted into a public page is a phishing pattern we should not teach.

## 6. When it fails

Each failure gets one plain line, drawn from the error's code and never from
the provider's words (the INVARIANTS rule on refusals):

| What happened | What the person sees |
| --- | --- |
| 401 (bad or revoked key) | "Your OpenRouter key was refused. Update it in Settings." |
| 402 (no credits) | "Your OpenRouter account is out of credits. Add credits at OpenRouter, or switch to OpenAgents in Settings." |
| 429 (rate limit) | "OpenRouter is rate-limiting your key; try again shortly." |
| Model not available on the key | "Your OpenRouter key can't use {model}." |
| No connection | "Couldn't reach OpenRouter; try again." |

- **Never a silent fallback.** Under `mine`, nothing ever falls back to our
  key. Under `mine_then_ours`, a fallback happens and the status line says so
  for that turn ("Answered on OpenAgents; your key was refused").
- **Benching.** A refused key is benched for five minutes, as
  `jev::doors::BENCH` does for our doors, so one bad key does not fail every
  call twice.
- **No limits shown for our path** (owner rule, #10120). The 402 and 429
  messages above are about the person's own OpenRouter account, a limit they
  set and pay for. Nothing on our path gains a limit, and the `ours` mode reads
  exactly as it does today.

## 7. Records: who paid

- **Coder run records.** `ResultRecord` (`crates/coder/src/task/owner.rs`)
  gains `payer: ours | theirs | mixed` and `payer_fingerprint`. The per-part
  costs (`engine_microusd`, `jev_microusd`) keep their meaning. A part paid
  through the person's OpenRouter key takes OpenRouter's reported cost
  (`usage.cost`) as the price.
- **Chat worker usage log.** The log (`crates/coder/src/relay/usage.rs`
  `Record`) gains `payer` and `payer_fingerprint`. `coder-worker usage --by
  payer` reads them. Our spend then excludes jobs the person paid for.
- **Decision records.** `openagents.decision-call.v1` records keep
  `service.upstream` (the door) and add `payer`.
- **The person sees their spend; we don't show ours.** Under `mine`, the
  settings section shows "This key has spent $X" from OpenRouter's key
  endpoint, and a Coder run's detail view may show what that run cost on their
  key. It is their money, and they asked to see where it goes. Our own costs
  stay in the records and stay unshown, as decided in f9a8cce433.

### What the chat says about BYOK

The knowledge corpus currently says the chat has "no API key to bring"
(`knowledge/openagents/openagents.pricing.md`). That is true until this ships.
The same change that ships BYOK updates these entries to describe it exactly:

- `openagents.pricing.md`
- `openagents.chat-privacy.md`: who receives messages under `mine` (OpenRouter
  and the model's provider, under the person's own OpenRouter account and its
  data settings)
- `openagents.jev.md`
- `openagents.microcoder.md`

The bank's `meta.privacy` and `meta.model` answers then name the person's key
as the door when it is in use. Until then, the chat keeps saying there is no
key to bring.

## 8. Phased plan

The phases are ordered by value per effort. Each phase lists the touched crates'
tests and `cargo fmt` (lean verification).

| Phase | What | Crates | Size | Tests |
| --- | --- | --- | --- | --- |
| 1. Key and mode | `models.payer` in settings; `openrouter-key set/show/test/clear` (`Effect::Secret`); keychain-or-0600 storage; `--openrouter-key` and `OPENAGENTS_OPENROUTER_KEY`; validation call; confirm a non-owner key can call `typesafe/jev-1.13` | `coder` (task/settings), `openagents-cli`, `openagents-connect`, `openrouter` | M | The key is never in output, logs, or argv echo; an ambient `OPENROUTER_API_KEY` never sets `mine`; a 401 is not stored; `clear` resets to `ours` |
| 2. Model access layer, local calls | `crates/model-access`. Jev on the person's key (skip the hosted decision service). Embeddings, Microcoder's cloud provider (`openai/gpt-6.1-sol`), and local plugin eval judges on their key | `model-access` (new), `jev-hosted`, `coder-delegate`, `coder`, `microcoder`, `knowledge`, `ext-eval` | M–L | `mine` builds no door with our keys (a fake door per provider asserts it was never called); `mine_then_ours` falls back and records it; each failure line |
| 3. Hosted chat on their key | Payer envelope and `requires: payer.openrouter`; worker uses it for model, personalization, Jev, embeddings; NIP-CJ amendment and INVARIANTS row; usage log `payer` | `openagents-chat` (basic_coder), `coder` (coder-worker, relay/usage, generate, router/personalize), `nostr` (cj_conversation), `nips/openagents/NIP-CJ.md`, `INVARIANTS.md` | L | An old worker refuses; the key is absent from every log, usage line, and result; it is not retained after the job; open callers stay conversation-only; a 402 gives its one line |
| 4. Screens | Terminal `/settings` secret field and mode; desktop Models pane; phone Account section; Connect OpenRouter (OAuth PKCE); status line everywhere | `openagents-terminal`, `openagents-cli` (screen), `openagents-desktop`, `openagents-mobile`, `openagents-chat-app` | M | Snapshot and deck captures of each screen; the masked field never renders the key |
| 5. Records and the chat's own words | `payer` on run and decision records; the person's spend shown; knowledge entries and bank answers updated | `coder` (task/owner, relay/usage), `jev`, `knowledge/openagents`, `crates/coder/answers` | S | The bank lint passes; the QA rubric checks that BYOK is described accurately and never invented |

## 9. Open questions

1. **Jev on any key.** Does OpenRouter's `/api/alpha/decisions` serve
   `typesafe/jev-1.13` to any key with credits, or only to approved accounts?
   Phase 1 tests this with a fresh key. If Jev is gated, `mine` needs either
   TypeSafe's own BYOK (a TypeSafe key field beside the OpenRouter one) or an
   explicit exception that Jev stays on ours.
2. **The chat primary after 2026-10-05.** Should the chat under `mine` follow
   whatever primary we pick next, or let the person choose a chat model from
   OpenRouter's catalog? This design keeps our choice, so behaviour stays the
   same on either key.
3. **Website and API callers.** Should the OpenAgents API (`oak_` keys) accept
   a per-request OpenRouter key, as the Khala gateway design did
   ([#6380](https://github.com/OpenAgentsInc/openagents/issues/6380))? This
   design leaves the website's Ask box on ours.
4. **`mine_then_ours`.** Is it wanted at all, or should BYOK be strictly `mine`
   or `ours`? It adds a mode to explain, but it keeps a person with an empty
   OpenRouter account working.
5. **Option B later?** Would people rather register a key once on the worker
   (stored encrypted) than send it with each job? Option A first; revisit only
   if per-job sending proves a problem.
6. **TypeSafe and the Vercel AI Gateway.** Does "bring your own key" mean
   OpenRouter only, or also accept those keys? The owner named OpenRouter; the
   access layer's `Payer` enum leaves room for more.
