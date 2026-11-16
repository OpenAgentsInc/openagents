# Hosted tool continuation and argument admission

Checkpoint for #13620. The existing browser command-table executor now consumes
the shared gateway step and owned conversation history. Its API routes remain
the executor; no additional runner or parser is introduced. Published call and
result fields retain their identities. See the additive implementation section
in [the gateway contract](../protocol/gateway-tools-v2alpha1.md).

Calls keep their native IDs, full argument JSON and correlated error/results.
Every offered function is validated using the existing Responses flat-schema
owner before a selected tool runs. Unknown schema constructs refuse. Local
strict admission enforces the supported profile; the browser does not claim
that its provider honors an upstream strict flag. Argument errors become
paired tool results without requesting a tool route. The next model request
contains assistant function calls and matching tool messages, rather than
inserting tool output as user text.

The owned history copies its initial text, native calls and outputs into a
caller-supplied bounded byte pool. Pending batches never serialize. Completed
imported history stays inert; repeated IDs, conflicting/foreign results,
exhausted storage and the continuation limit refuse. Identical settled result
replay changes nothing. The browser retains its read-only table, route/account
authorization, explicit response truncation and three tool-bearing steps plus
a final ask without tools.

## Observations

Heavy qualification ran on the owned Linux pool. Counts overlap through imports.

| Check | Result |
| --- | --- |
| Offered-argument policy | 135 checks; 68 mutants, 60 killed and eight reviewed equivalents, zero unresolved survivors |
| Owned-history policy | 143 checks; 206 mutants, 202 killed and four reviewed equivalents, zero unresolved survivors |
| Split history checks | 146 checks pass; the same 206 root mutants remain 202 killed/four reviewed equivalents, zero unresolved survivors |
| Existing table executor | 164 checks; 421 mutants, 402 killed and 19 reviewed equivalents, zero unresolved survivors |
| Native product fixture | Coax CLI baseline and debug/release, ordinary/ASan/UBSan all passed |
| Complete evaluator baseline | 387/387 pass after explicit budgets for ten older expensive checks; expanded 399/399 also pass directly in the evaluator |
| Chat Wasm | Eight-call/error-path build passed; the CRLF collector build at `9282545962` also passes, 354,791 bytes in 14 seconds |
| Repository checks | tools-check, wire-check and layer-check passed; 400 registry rows, no allowlist increase |
| Browser host, clean read | No tool request before HTTP EOF; then both table routes and a second model request with the exact native call/result ID |
| Browser host, schema refusal | Wrongly typed slug caused no tool request; a paired error result reached the second model request |
| Browser host, transport failure | A failed response after provisional tool finish started no tool and no continuation |
| Browser host, Stop | Stop before EOF retained the chat and started no tool or continuation, including after late completion |
| Browser host, nine-call response | Entire response refused; no tool request or continuation |
| Browser host, eight calls | Zero route reads before EOF; all eight calls, 16 route reads and eight matching results in the next model request |
| Browser host, CRLF | Zero route reads before EOF; then two reads and the exact native call/result pair in a second model request |
| Browser host, malformed CR in JSON | No route read or continuation; the visible error retains the failed turn |
| Browser host, exact SSE capacity | A complete 32,768-byte event without DONE reaches clean EOF, two reads and a paired continuation |
| Request encoding exhaustion | Regression passes; preserves the draft, starts no stream and shows an explicit error |

Argument-policy receipt: `gateway-conversation-policy2-receipt.log`. Final
history-policy receipt: `gateway-conversation-policy5/build/pipeline/only.log`,
qualified at `177fa7adbe20044a4701ea381e1ab51e9503a349`. Artifacts, browser
observations and screenshots are in `/tmp/oa-chat-task-artifacts/` with the
`gateway-conversation-` prefix. The screenshot review confirms visible tool
rows, a failed schema row and final model answers on the rebuilt page.

The native fixture exercises the ordinary product backend, not the
check-only emitter. It overwrites parser/source buffers before inspecting
retained data and checks exact provider continuation bytes and schema errors.
Subsequent history changes split equivalent preflight conditions and strengthen
checks without changing those admitted fixture outcomes.

The browser uses the actual rebuilt Wasm, DOM input and stream host in muted
headless Chrome. Browser glue supplies controlled `Response`/`ReadableStream`
fixtures and records request bodies and `credentials: same-origin`. These are
host integration observations, not live provider inference, account/grant
qualification or server authorization tests. The initial fixture setup omitted
request/controller observations; only the corrected `held3`/`held4`, `clean`,
`clean2`, `schema-held2`/`schema2`, `cut`, `stop` and `excess` receipts support
the table above. The reusable page receives multiple independent user turns.

The final browser eight-call receipt is `gateway-conversation-browser-eight-final2.log`.
It records a fresh page loading the 354,725-byte `chat-web.wasm`, visible Stop
before EOF, all 16 route reads and every native call/result ID in the second
request. The single-shot fixture waits for Stop to render before releasing EOF;
an earlier single-shot attempt observed the stale Send button too early and is
not qualification. The event buffer now has 16 slots, sufficient for eight
calls plus text/reason/finish events. A request encoding refusal keeps the draft
instead of waiting for a stream that was never started.

The table executor's receipt is
`gateway-conversation-hosted3/build/pipeline/only.log`, qualified at
`3d655775c8ad498185e2b787d8fe32a90fa72864`. Exact tests cover offered property
descriptions, closed schemas, 64/65-byte names, 512/513-byte arguments, escaped
route identifiers, resetting an earlier admitted route, 4,096-byte output and
two/three/four-byte UTF-8 characters across the truncation boundary. Remaining
equivalents include the fixed table's file-ID property position and equal
branches at the exact body-length cap. The final Wasm build includes these
checks; product route behavior and the qualified native/browser outcomes are
unchanged.

The large history fixture was split into independent state, import, final-batch
and replay checks. The final-batch check compares every expected row directly
instead of reusing the product validator for a second full-history pass. Two
maximum-size cases have explicit per-check step/traversal budgets. The final
batch passes in the evaluator at 87,663,278 steps/2,380 ms; all seven hosted
browser scenarios and both Gemini loop scenarios likewise pass directly in
the evaluator. Native checks are retained separately. This avoids the previous
aggregate-limit fallback without rewriting the shared UTF-8 implementation.

The full 387-check evaluator run exposed ten expensive checks exceeding their
old 20-million-step budgets, including older settings and local-agent cases.
Each now declares its measured budget. The expanded 399-check baseline passes
in 124 seconds. These are actual `--profile steps` evaluator results; the
default CLI's automatic native selection is a separate observation.

The first complete page-root mutation audit is not a pass: all 1,400 sites
finished in 32 bounded shards, with 979 killed, 420 unresolved survivors and
one reviewed equivalent. Two SSH failures were retried; one shard that reached
its 720-second limit completed with the native-first path in 327 seconds.
Earlier half/eighth-sized attempts exhausted their time limits before the
evaluator budget diagnosis. None is counted as successful qualification.
The current browser source is still undergoing assertion and invariant review,
including older branches in the changed large procedures. New checks cover
exact provider JSON, per-step assistant text, a provider attempting a fourth
batch, account/Project read completion, context bounds and request cleanup.

The first native-first recheck finishes 177 of 1,409 page-root mutants in 573
seconds: 154 killed, 19 unresolved and four reviewed equivalents. The native
and evaluator paths agree on every verdict. Later checks directly cover
cancellation after a failed partial tool read, a successful call following a
failed call, presentation-buffer exhaustion with whole native results,
transcript capacity, escaped session IDs and stale request state. These checks
retain complete evidence rather than counting the earlier failing shards as
passes.

The byte review also found a real collector error: removing every carriage
return could repair an invalid JSON tool name. The collector now keeps those
bytes and accepts CRLF at event boundaries through the existing SSE reader.
Both the CRLF/malformed-name cases and a complete 32,768-byte event pass in the
evaluator (`gateway-page-crlf-checks.log`). The actual 354,791-byte Wasm then
passes the corresponding three DOM/stream-host cases. Receipts are
`gateway-page-browser-crlf.log`, `gateway-page-browser-badcr.log` and
`gateway-page-browser-exact.log`; each records `ok: true`. Screenshot review
confirms the completed CRLF answer and the visible malformed-stream refusal.

The later 417-check source's eight mutation shards reached their 720-second
limits. A smaller 32-way audit completed 31 shards; shard 11 reached its
600-second limit. The completed shards found 91 unresolved sites. This is
an audit, not a pass. Three SSH/session failures completed on retry. Records
have the `gateway-page-final32-` prefix, and their unresolved-site inventory is
`gateway-page-final32-survivors.txt` in the same artifact directory.

The broader callback matrix exposed a real queued-message bug: a failed
background import returned before the draft resumed. The completion path now
uses the same background-read predicate as send admission and resumes a
deferred draft on every exit. Five new matrix checks pass directly in the
evaluator, covering all nine reads, unqueued drafts, whole-response bounds,
exact-fit history, one-letter Project names, SSE separators and diagnostics
(`gateway-page-audit-matrix2.log`). A further regression reproduces owned
tool-result exhaustion starting another route; the executor now checks the
failed history before advancing to another read. Its expanded qualification
and the remaining page-root audit are still in progress.

The current CLI's portable C was emitted on `oa-build-11` in 17 seconds and
linked against Apple's SDK using the documented `--link-c` path. The shared
`~/.openagents/bin/openagents` now selects the versioned `tasks-cli-9282545962`
binary. Its existing staging profile still authenticates successfully; no
account key or login profile was replaced. This is installation/authentication
evidence, not a live native task receipt.

Initial policy mutation runs found weak assertions around descriptions, owned
history and complete batches. Independent boundary/outcome checks replaced
them. After three history rounds, a complete per-guard refusal matrix and an
invariant audit replaced incremental assertions. Reviewed equivalents concern
descendant JSON indices that cannot be zero, duplicate copy-helper capacity
checks, the call getter's refusal of unsealed/failed steps, and the upstream
step owner's eight physical slots. They are documented at the exact sites.

## Remaining issue scope

This browser history is ephemeral. It does not establish a durable shared
executor, adoption by the account/mobile chat, provider-account qualification
or a staging coding task with an expected patch and test result. Native mobile
controls already exist; durable server capture paging and reconciled expired
Stop recovery are now separately qualified in
[the recovery checkpoint](2026-11-15-chat-native-recovery.md). #13620's
end-to-end acceptance boxes stay open. This checkpoint includes no production
deployment or TestFlight upload.
