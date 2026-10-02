# BYOK: run everything through the user's own keys

Status: implemented on computers, 2026-10-02. Tracking issue:
[#10176](https://github.com/OpenAgentsInc/openagents/issues/10176).

What shipped: `crates/model-access` (who pays, the fixed order, key checks,
keychain-or-0600 storage); `openagents settings provider-key
set|show|test|clear` and `models.payer`; the `--openrouter-key`,
`--vercel-key`, and `--typesafe-key` flags and `OPENAGENTS_*_KEY` variables;
Jev, embeddings, Microcoder's cloud fallback, and local plugin eval judges on
the person's keys; the hosted chat's `payer.keys` envelope (NIP-CJ,
"Caller-paid model calls"); `payer` fields in the chat worker's usage log;
the terminal's `/settings` key fields and switch; the desktop's Model
providers page; and `OpenAgents-Provider-Key` on model-cost routes of the
x402 pay front. Not yet: the phone's Account section, Connect OpenRouter
(OAuth PKCE), and grounded product and codebase answers for a chat on the
person's keys (those seams are off for such a chat).

The design started with OpenRouter. The owner's answers the same day (section 9)
added Vercel AI Gateway and TypeSafe keys from the start, and narrowed the
modes to two.

Owner request (2026-10-02, verbatim): "I want to be able to have an open router
API key mode, a B.Y.O.K. mode. We need to generally flesh out B.Y.O.K. stuff. I
want users to be able to easily add their own OpenRouter API key, and there
should be a flag you can pass at CLI time, or also have an in-app settings
screen for it. I want to be able to add an OpenRouter API key. If they do, they
can opt to have everything run through that key. If they provide that key,
everything runs through OpenRouter and not ours. The decision models and
language models should use their stuff instead of ours."

## 1. What BYOK means

A person adds one or more of their own provider keys:

- **OpenRouter**: chat models, embeddings, and Jev through OpenRouter's
  Decisions API.
- **Vercel AI Gateway**: chat models, embeddings, and Jev as
  `typesafe-ai/jev`.
- **TypeSafe**: Jev (System One) decisions only.

With **"Use my keys for everything"** on, every model call OpenAgents makes on
that person's behalf goes to a provider on one of their keys, and none goes on
ours. That covers chat replies, Jev decisions, Microcoder, embeddings, and
judges. When their keys cannot make a call, it fails plainly. Nothing falls
back to ours.

Coding agents that already run under the person's own logins are unchanged:
Claude Code, Codex, Grok Build, OpenCode, and Devin. Their providers bill the
person directly today, and BYOK does not touch them.

BYOK is always an explicit choice. Ambient variables never switch anyone to
their own keys: an `OPENROUTER_API_KEY`, `AI_GATEWAY_API_KEY`, or
`TYPESAFE_API_KEY` that happens to be in a shell environment. The Khala Code
incident ([#7955](https://github.com/OpenAgentsInc/openagents/issues/7955))
showed why. An ambient developer key was silently forwarded as BYOK, the
account had no credits, and hosted chat broke with a 402 that pointed somewhere
else.

One exception already exists. A TypeSafe key the person saved in
`~/.openagents/jev.json` already makes their own computer's Jev calls on their
key (`jev_hosted::resolve`). BYOK keeps that behaviour and moves the key into
the same storage as the others.

## 2. Where we pay today

Every place below spends our keys or our accounts. Paths are relative to the
repository root.

| Call site | What it does | Model today | Door and key today |
| --- | --- | --- | --- |
| Chat worker, primary (`crates/coder/src/generate.rs` `Lane::SpaceBunny`, `FallbackDoor::openrouter`; `crates/coder/src/bin/coder-worker.rs`) | Chat replies for the phone, the desktop, `openagents chat`, the terminal | `stealth/space-bunny-alpha`, reasoning low (free on OpenRouter; retired 2026-10-05) | OpenRouter, our `OPENROUTER_API_KEY` on oa-coder-worker-1 |
| Chat worker, fallback (`generate.rs` `Lane::Gemini`) | Any turn the primary misses | `google/gemini-3.8-flash` | Vercel AI Gateway, our `CODER_DOOR_KEY` |
| Website Ask box (`crates/openagents-web/src/ask.rs`) | Homepage questions | Same chat worker, through the relay | Ours (the site signs each visitor's jobs with a derived key) |
| Chat router judge (`coder-worker.rs`, `crates/coder/src/router/judge.rs`) | Route, prepared answer, opener, lane, risk, command group, tool, capability | Jev | Gateway `typesafe-ai/jev`, then OpenRouter `typesafe/jev-1.13`, then TypeSafe (`crates/jev/src/doors.rs`, `Failover::primary_last`) |
| Personalization (`crates/coder/src/router/personalize.rs`) | Finishes "Working on ...", "Looking through ...", "Picking up ..." | `google/gemini-2.5-flash-lite` | OpenRouter, our key |
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

One section, **Your keys**, with:

- **One row per provider** (OpenRouter, Vercel AI Gateway, TypeSafe). Each row
  says whether a key is added and shows the key's last four characters and its
  state ("works", "no credits", "refused").
- **Use my keys for everything:** on or off.
- **A status line**, one of:
  - "Running on OpenAgents."
  - "Running on your keys."
  - "Your OpenRouter key was refused; nothing is running on ours."

There are two modes, set by the key `models.payer`:

| Mode | Meaning |
| --- | --- |
| `ours` (default) | Today's behaviour. Stored keys are kept but not used. |
| `mine` | Every model call in section 2 goes to a provider on one of the person's keys (section 4 says which). Nothing ever falls back to ours. A call their keys cannot make fails with one plain line (section 6). |

Turning on `mine` needs a key that can answer chat, which means OpenRouter or
Vercel AI Gateway. With only a TypeSafe key, the switch says "A TypeSafe key
covers decisions only. Add an OpenRouter or Vercel AI Gateway key to run
everything on your keys." It stays off.

Adding a key never switches the mode on its own. The add flow then asks one
question: "Use your keys for everything?" Answering yes sets `mine`.

### Command line

One command, with the provider as an argument. It follows the `openagents
settings` tree in `crates/openagents-cli/src/settings.rs`:

```text
openagents settings provider-key set PROVIDER     Read the key from the prompt or stdin (never argv), test it, store it.
openagents settings provider-key show [PROVIDER]  For each stored key: the provider, the last four characters, the label, and whether it works.
openagents settings provider-key test [PROVIDER]  Test the stored keys now.
openagents settings provider-key clear PROVIDER   Remove that key. Removing the last chat-capable key returns the mode to ours.
openagents settings set models.payer mine|ours

PROVIDER is openrouter, vercel, or typesafe.
```

There is also a per-invocation form that is never stored:

- **Flags.** `--openrouter-key KEY`, `--vercel-key KEY`, and `--typesafe-key
  KEY` are global flags, parsed like `--json` in
  `crates/openagents-cli/src/main.rs`. Any of them means `mine` for that one
  command.
- **Environment variables.** `OPENAGENTS_OPENROUTER_KEY`,
  `OPENAGENTS_VERCEL_KEY`, and `OPENAGENTS_TYPESAFE_KEY` are the same thing.
  They are distinct from the ambient provider variables on purpose
  (section 1).

The help text warns that a key in argv shows up in shell history and `ps`, and
recommends `set` or the environment variables instead. `openagents terminal`,
`openagents chat`, and bare `openagents` all take the flags.

The `provider-key` commands are declared `Effect::Secret`, like `wallet
export`. The chat router never proposes a secret command (INVARIANTS.md), so no
chat turn can ask for or carry a key.

### Screens

- **Terminal `/settings`** (`crates/openagents-terminal/src/lib.rs`
  `Settings`/`Choice`). Its choices are on/off only today. It needs one new
  input kind: a masked secret field with paste, one per provider. The mode is
  an on/off choice, so it fits as it is.
- **Desktop** (`crates/openagents-desktop/src/settings.rs`). A new "Your
  keys" pane next to Coder holds one masked field per provider (paste, Test,
  Remove), the mode switch, the status line, and links to each provider's key
  page.
- **Phone.** There is no Settings screen today. The same section goes on the
  Account tab (`crates/openagents-mobile/src/account.rs`).
- **Easiest add.** A "Connect OpenRouter" button using OpenRouter's OAuth PKCE
  flow, which returns a key the person controls. They then never copy or paste
  a key. The desktop opens the browser to a loopback callback, and the phone
  uses its URL scheme. Vercel and TypeSafe keys are pasted.

### Validation

A key is tested when it is added and when the person taps Test. Each test is
the provider's cheapest call:

| Provider | Test |
| --- | --- |
| OpenRouter | `GET https://openrouter.ai/api/v1/key` (free; returns the label and credit state) |
| Vercel AI Gateway | `GET https://ai-gateway.vercel.sh/v1/credits` (free; returns the balance) |
| TypeSafe | One minimal `POST /v1/systemone` decision (a fraction of a cent), until TypeSafe offers a key endpoint |

A key that answers 401 is not stored, and the person sees "{Provider} didn't
accept that key." A key with no credits is stored with the warning "This key has
no credits; calls on it will fail."

### Storage

The keys are the person's money, so they get the same care as the keys we
already hold:

- **Desktop host and CLI.** The OS keychain, through the existing `KeySource`
  in `crates/openagents-connect` / `crates/coder-host/src/serve/keys.rs`
  (service `com.openagents.desktop`, one account per provider:
  `provider-key-openrouter`, `provider-key-vercel`, `provider-key-typesafe`).
  That is Keychain on macOS, Secret Service on Linux, and Credential Manager
  on Windows. Where no keychain answers (headless Linux, a CLI-only install),
  each key goes in the 0600 file its crate already reads, in a 0700 directory,
  written through `coder::private`:
  - `~/.openagents/openrouter.json` (`openrouter::Config::from_env`)
  - `~/.openagents/ai-gateway.json` (new)
  - `~/.openagents/jev.json` (`jev_hosted::local_key`, unchanged)
- **iPhone.** A this-device-only Keychain item per provider, as the wallet
  seed is.
- **Android.** Each key encrypted under its own Android Keystore key, in
  app-private storage with no backup.
- **The mode.** `models.payer` lives in `~/.openagents/settings.json` under a
  new `models` section. It never holds a key.
- **Never shown.** A key never appears in a log line, an error, a usage record,
  a trace, a crash report, `--json` output, or a Debug string.
  `openrouter::ApiKey` and `jev::ApiKey` already redact themselves. Records
  carry only the provider and a fingerprint: the first 8 hex characters of the
  key's SHA-256 digest.

## 4. One place decides who pays

A new small crate, `crates/model-access`, answers one question for every call
site: **who pays for this call, and through which door?** Call sites stop
reading `OPENROUTER_API_KEY`, `CODER_DOOR_KEY`, `AI_GATEWAY_API_KEY`, and
`TYPESAFE_API_KEY` themselves and ask it instead.

```rust
pub enum Provider { OpenRouter, Vercel, TypeSafe }
pub enum Payer { Ours, Theirs { provider: Provider, fingerprint: String } }
pub enum Mode { Ours, Mine }

pub struct Access { mode: Mode, keys: Vec<(Provider, ApiKey)> }

impl Access {
    /// The doors for a chat model, a decision, or an embedding: ours, or the
    /// person's keys in the order below. Mine never returns one of ours.
    pub fn chat(&self, want: Use) -> Result<Doors, NoDoor>;
    pub fn decisions(&self) -> Result<jev::doors::Failover, NoDoor>;
    pub fn embeddings(&self) -> Result<knowledge::search::Embedder, NoDoor>;
}
```

### Which of the person's keys pays

The rule is fixed, and the person has nothing to configure:

- **Jev decisions:** TypeSafe, then Vercel AI Gateway, then OpenRouter. That
  is the most direct door first: TypeSafe serves Jev itself, and the gateway
  routes Jev to TypeSafe.
- **Everything else** (chat, personalization, embeddings, Microcoder, judges):
  OpenRouter, then Vercel AI Gateway.

The first provider in that order with a stored key answers. A call moves to the
person's next key only for that key's own reasons: 401, 402, 429, or no
connection, using the existing `FallbackDoor` and `jev::doors::Failover`
machinery with only their keys inside. A key that answered 401 or 402 is
benched for five minutes, as `jev::doors::BENCH` does. Under `mine` the list
never holds one of our keys, so "fall back" only ever means another key of
theirs. The status line names the provider that paid when it was not the first
one ("Answered on your Vercel key; your OpenRouter key has no credits").

### The rest of the design

- **The hosted decision service** (`jev_hosted::resolve`) is skipped under
  `mine`. The computer asks Jev directly on the person's key. This is the
  cheapest and simplest piece, because the computer already runs Jev locally
  whenever it holds a key.
- **Which surfaces use it.** Each surface (the desktop host, `openagents chat`,
  the terminal, Microcoder, the delegate door, plugin evals run locally) builds
  one `Access` at start from the settings and the flags, then passes it down.
- **No routing on text.** Nothing chooses a door from message text. The mode
  and the keys are typed settings, the order above is fixed in code, and the
  chat's routing stays the router's typed judgment.

### Model IDs per provider

Checked against OpenRouter's public catalog (`/api/v1/models`,
`/api/v1/embeddings/models`) and the Vercel AI Gateway's (`/v1/models`) on
2026-10-02.

| Use | Ours today | On OpenRouter | On Vercel AI Gateway | On TypeSafe |
| --- | --- | --- | --- | --- |
| Chat primary | `stealth/space-bunny-alpha` (OpenRouter; retires 2026-10-05) | Our current primary, whatever it is (section 9, decision 2) | Our current primary when the gateway serves it. Space Bunny it does not, so until then this falls to the chat fallback | — |
| Chat fallback | `google/gemini-3.8-flash` (Gateway) | `google/gemini-3.8-flash` | `google/gemini-3.8-flash` | — |
| Personalization | `google/gemini-2.5-flash-lite` (OpenRouter) | same | `google/gemini-2.5-flash-lite` | — |
| Jev | `typesafe-ai/jev` → `typesafe/jev-1.13` → `jev-1.13.0` | `typesafe/jev-1.13` at `POST /api/alpha/decisions` (Phase 1 test: any key?) | `typesafe-ai/jev` at `/typesafe/v1/systemone` | `jev-1.13.0` at `POST /v1/systemone` |
| Embeddings | `openai/text-embedding-3-small` (Gateway) | `openai/text-embedding-3-small` | `openai/text-embedding-3-small` | — |
| Microcoder cloud | OpenAgents cloud (Vertex) | `openai/gpt-6.1-sol` | `openai/gpt-6.1-sol` | — |
| Knowledge harvest | `openai/gpt-6-luna` | same | checked in Phase 2 | — |
| Eval `judge` graders | `google/gemini-3.8-flash` (Gateway) | same | same | — |

Notes on the table:

- **Embeddings** give the same vectors on every provider, so caches and
  indexes stay valid.
- **No silent model swap.** A model none of the person's keys can call is
  never swapped for another model on our key. Under `mine` the call fails and
  names the missing model. The one designed substitution is the chat primary
  on a Vercel-only key: it uses the chat fallback, which is what our own chat
  does whenever the primary misses.
- **Gym and benchmark runs** pin door identity (`docs/gym/regression.md`). A
  run made on the person's key records `payer: theirs`, the provider, and the
  door, so it is never compared as if it had used ours.


## 5. The hosted chat

The chat runs on our worker (oa-coder-worker-1), not on the person's computer:

- the phone has no computer,
- the router, bank, knowledge index, and question sets live on the worker, and
- the website's Ask box goes there too.

For the chat to run on the person's keys, either the keys go to the worker for
their jobs, or the model calls move off the worker.

| Option | How | For | Against |
| --- | --- | --- | --- |
| **A. Keys with each job** (recommended) | The client puts the keys in a payer envelope inside the NIP-44-encrypted job. The worker uses them for that job's model, personalization, Jev, and query embedding, then drops them. | Works on the phone, desktop, terminal, and CLI alike. One worker. Keys are never at rest on our side. Revoking is "remove it in the app". | The keys exist in the worker's memory during the job. People have to trust our open-source worker not to keep them. NIP-CJ's "Payloads contain no bearer credential" rule needs an amendment. |
| B. Keys registered with the worker | The person registers their keys once. The worker stores them encrypted, keyed by the person's signing key, and uses them for every job they sign. | Fits NIP-CJ as written: the signer maps to policy outside the payload. Smaller jobs. | Many people's keys at rest on our server, which is a standing breach target. Needs a register/rotate/delete protocol. A stored key outlives the person's intent. |
| C. Model calls on the person's device | The worker returns the routed plan (instructions, retrieved context, judgment) and the client calls the provider itself. | Keys never leave the device. | Jev's judgment and the retrieval embedding still need a key on the worker, or the router moves to the client. That splits the chat into two implementations and doubles phone traffic. |

**Recommendation: A.** The keys go with each job, under four rules.

1. **A declared feature.** The job's `requires` gains `payer.keys`. NIP-CJ
   already says a worker refuses a body with features it doesn't know. An
   older worker therefore refuses the turn rather than silently answering it on
   our keys. The client says "OpenAgents chat can't use your keys yet", with no
   fallback.
2. **A separate envelope.** The keys travel as `payer: {keys: <NIP-44
   ciphertext to the worker of [{provider, key}]>}`, encrypted a second time
   apart from the rest of the body. The decrypted job `Value` never holds the
   plaintext, so a stray debug dump of the request cannot leak a key. The
   worker decrypts them into `ApiKey`s held only for that job and zeroed after,
   and picks among them by the rule in section 4.
3. **They pay; they grant nothing.** The payer credentials pay for the job's
   model calls. They never widen admission, delegation, or execution: an open
   caller still gets conversation jobs only. The usage log gets
   `payer: "theirs"`, the provider, and the fingerprint, never a key.
4. **Policy.** The NIP-CJ amendment states this exception: a caller-paid
   provider credential that is not a grant. The same change adds a matching
   row in INVARIANTS.md with tests:
   - no key in any log, usage line, or result,
   - an older worker refuses,
   - `mine` never touches our doors,
   - keys are not retained after the job.

People who want their keys never to leave their own machine can run their own
worker. Self-hosting is already in the [API design](../api/2026-10-02-openagents-api.md).
Option C can come later for the desktop and terminal, where the computer can
run the whole router locally.

The website's Ask box stays on ours. Visitors have no settings, and a key
pasted into a public page is a phishing pattern we should not teach.

## 6. API callers bring their own key

Owner decision (section 9, decision 3): a caller of the
[OpenAgents API](../api/2026-10-02-openagents-api.md) can send its own provider
key with each request. When it does, **no x402 payment is needed for the
call's model cost**. The API design otherwise answers a priced call from a
caller with no plan with an x402 `402 Payment Required`
([its section 5, "Payment: x402"](../api/2026-10-02-openagents-api.md#5-payment-x402)).

- **HTTP fronts.** The key goes in one request header, `OpenAgents-Provider-Key:
  <provider> <key>` (provider `openrouter`, `vercel`, or `typesafe`). It may be
  repeated for several providers, and section 4's order picks among them.
- **Nostr callers.** The key goes in the same payer envelope as section 5,
  inside their NIP-44-encrypted job.
- **What it covers.** A request whose model calls are all covered by the
  caller's keys gets no x402 challenge for model cost. This is the same `mine`
  rule as everywhere else: a model call the caller's keys cannot make fails
  plainly, and is never quietly moved to our key and billed. Anything else the
  API prices is unaffected.
- **Same protections.** The key is used for that request only, never stored or
  logged, and grants nothing beyond what the caller's `oak_` key or signature
  already allows. Receipts and usage records name `payer: theirs`, the
  provider, and the fingerprint.

The API design's payment section links back here.

## 7. When it fails

Each failure gets one plain line, drawn from the error's code and never from
the provider's words (the INVARIANTS rule on refusals). `{Provider}` is
OpenRouter, Vercel AI Gateway, or TypeSafe.

| What happened | What the person sees |
| --- | --- |
| 401 (bad or revoked key) | "Your {Provider} key was refused. Update it in Settings." |
| 402 (no credits) | "Your {Provider} account is out of credits. Add credits there, or switch to OpenAgents in Settings." |
| 429 (rate limit) | "{Provider} is rate-limiting your key; try again shortly." |
| Model not on any of the person's keys | "Your keys can't use {model}." |
| No connection | "Couldn't reach {Provider}; try again." |

- **Only the last failure shows.** The person sees a line only after every key
  of theirs that serves the call has failed. It names the last one tried.
- **Never our key under `mine`.** A failure there is the answer, and the line
  offers the switch to OpenAgents in Settings as the person's own choice.
- **No limits shown for our path** (owner rule, #10120). The 402 and 429 lines
  are about the person's own provider account, a limit they set and pay for.
  Nothing on our path gains a limit, and `ours` reads exactly as it does today.

## 8. Records: who paid

- **Coder run records.** `ResultRecord` (`crates/coder/src/task/owner.rs`)
  gains `payer: ours | theirs` and, for `theirs`, the provider and the
  fingerprint of each key that paid. The per-part costs (`engine_microusd`,
  `jev_microusd`) keep their meaning. A part paid on the person's key takes
  the provider's reported cost (`usage.cost` on OpenRouter, the gateway's
  `provider_metadata.gateway` cost) as the price.
- **Chat worker usage log.** The log (`crates/coder/src/relay/usage.rs`
  `Record`) gains `payer`, `payer_provider`, and `payer_fingerprint`.
  `coder-worker usage --by payer` reads them. Our spend then excludes jobs the
  person paid for.
- **Decision records and API receipts.** `openagents.decision-call.v1` records
  keep `service.upstream` (the door) and add `payer`. API receipts do the same.
- **The person sees their spend; we don't show ours.** Under `mine`, each key's
  row shows what it has spent, read from the provider's key or credits
  endpoint. A Coder run's detail view may show what that run cost on their
  keys. It is their money, and they asked to see where it goes. Our own costs
  stay in the records and stay unshown, as decided in f9a8cce433.

### What the chat says about BYOK

The knowledge corpus currently says the chat has "no API key to bring"
(`knowledge/openagents/openagents.pricing.md`). That is true until this ships.
The same change that ships BYOK updates these entries to describe it exactly:
optional, three providers, and `mine` meaning nothing on ours.

- `openagents.pricing.md`
- `openagents.chat-privacy.md`: who receives messages under `mine` (the
  person's provider and the model's provider, under the person's own account
  and its data settings)
- `openagents.jev.md`
- `openagents.microcoder.md`

The bank's `meta.privacy` and `meta.model` answers then name the person's
provider as the door when it is in use. Until then, the chat keeps saying there
is no key to bring.

## 9. Owner decisions (2026-10-02) and the one open question

The owner answered the first draft's open questions:

1. **Two modes.** `mine` fails plainly and never falls back to ours. The
   earlier `mine_then_ours` mode is dropped: the modes are `ours` and `mine`.
2. **The chat model follows ours.** After Space Bunny retires on 2026-10-05, a
   BYOK person follows our current chat primary, on their key. There is no
   model picker.
3. **API callers can bring a key.** A caller of the OpenAgents API may send its
   own provider key per request. When it does, no x402 payment is needed for
   model cost (section 6).
4. **Three providers from the start.** Vercel AI Gateway and TypeSafe keys are
   accepted alongside OpenRouter from the start. That means one `provider-key`
   command with a provider argument, per-provider storage and validation, the
   per-provider model table, and the fixed rule for which key pays (sections 3
   and 4).
5. **Key with each job.** The hosted chat takes the person's keys with each
   job (option A). Keys registered on the worker (option B) are not planned.

Still open, settled by a Phase 1 test:

- **Can any OpenRouter key call Jev?** OpenRouter serves Jev at
  `/api/alpha/decisions` as `typesafe/jev-1.13`; its chat catalog lists only
  `typesafe/jev-router`. Phase 1 calls it with a fresh non-owner key with
  credits.
  - If that works, OpenRouter alone covers everything.
  - If it doesn't, an OpenRouter-only person's decisions fail plainly under
    `mine`: "Your keys can't use Jev; add a TypeSafe or Vercel AI Gateway key."
    The settings section says so when `mine` is turned on.

## 10. Phased plan

The phases are ordered by value per effort. Each phase lists the touched crates'
tests and `cargo fmt` (lean verification).

| Phase | What | Crates | Size | Tests |
| --- | --- | --- | --- | --- |
| 1. Keys and mode | `models.payer` (`ours`/`mine`); `provider-key set/show/test/clear PROVIDER` (`Effect::Secret`); keychain-or-0600 storage per provider; the three flags and the three `OPENAGENTS_*_KEY` variables; per-provider validation; `mine` refused without a chat-capable key; the Jev-on-OpenRouter test with a fresh key | `coder` (task/settings), `openagents-cli`, `openagents-connect`, `openrouter`, `jev`, `jev-hosted` | M | No key in output, logs, or `--json`; ambient provider variables never set `mine`; a 401 key is not stored; clearing the last chat-capable key resets the mode to `ours` |
| 2. Model access layer, local calls | `crates/model-access` with the fixed order; Jev on the person's keys (skipping the hosted decision service); embeddings, Microcoder's cloud provider (`openai/gpt-6.1-sol`), and local plugin eval judges on their keys | `model-access` (new), `jev-hosted`, `coder-delegate`, `coder`, `microcoder`, `knowledge`, `ext-eval` | M–L | `mine` builds no door with our keys (fake doors per provider assert they were never called); the order per use; failover only among the person's keys; each failure line |
| 3. Hosted chat on their keys | Payer envelope and `requires: payer.keys`; the worker uses the keys for the model, personalization, Jev, and the embedding; NIP-CJ amendment and INVARIANTS row; `payer` fields in the usage log | `openagents-chat` (basic_coder), `coder` (coder-worker, relay/usage, generate, router/personalize), `nostr` (cj_conversation), `nips/openagents/NIP-CJ.md`, `INVARIANTS.md` | L | An older worker refuses; no key in any log, usage line, or result; keys are not retained after the job; open callers stay conversation-only; a 402 gives its one line |
| 4. Screens | Terminal `/settings` secret fields and mode; the desktop's "Your keys" pane; the phone's Account section; Connect OpenRouter (OAuth PKCE); the status line everywhere | `openagents-terminal`, `openagents-cli` (screen), `openagents-desktop`, `openagents-mobile`, `openagents-chat-app` | M | Snapshot and deck captures of each screen; a masked field never renders the key |
| 5. API callers | `OpenAgents-Provider-Key` on the HTTP fronts and the payer envelope for Nostr callers; no x402 challenge for model cost when keys cover the call; receipts name the payer; a cross-reference in the API doc | `openagents-web` (fronts), `x402`, `gateway`, `docs/api` | M | A covered call gets no model-cost challenge; an uncovered model call fails plainly and is never billed to ours; the header never appears in a log or receipt |
| 6. Records and the chat's own words | `payer` on run, decision, and receipt records; the person's spend shown; knowledge entries and bank answers updated | `coder` (task/owner, relay/usage), `jev`, `knowledge/openagents`, `crates/coder/answers` | S | The bank lint passes; the QA rubric checks that BYOK is described accurately and never invented |
