# Requested guard directions: finite prototype evaluation

This candidate is `everglade-directions-v1`. It uses a separate authored
Directions intent and only the current visible objective. Its choices are
quiet, quest_board, east_gate, north_trail and west_gate. It changes no quests,
movement, combat, crime, rewards or generated dialogue. The existing ambient
v1 policy and its recorded corpus remain unchanged.

The [frozen labels](../../fixtures/guard-guide-labels-v1.jsonl) have SHA-256
`fb63e7861e32e6c42178f9405ac7bcf96d845df306cb27fbc44dfea0a426efee`.
They were fixed before candidate calls. Cases 0, 1 and 4 are development;
cases 2 and 3 are held out. The five unique remote goals are find_work,
waystone, ruins, river and return_to_board. Cases 5–12 separately test local
refusals: no intent, ineligible gameplay, unavailable route, already arrived,
already-visible guide tip, and goals 0, -1 and 6. Never query these local cases.
Do not pad the remote sample with irrelevant time/weather duplicates.

The [five exact requests](../../fixtures/guard-guide-requests-v1/) were frozen
before calls; `SHA256SUMS` records their byte identities. No labels, case IDs or
split names are sent to the model. Do not change prompts between development
and held-out calls, and do not relabel after outputs. Changed requests require
a new candidate/corpus and a fresh pre-call freeze. Preserve failed attempts;
do not replace them silently with successful retries. Duplicate case IDs are
rejected by the summary.

The independent route reference is `[1,2,3,4,1]`. Always quiet gives no useful
directions in these five requested contexts. The cheap deterministic goal-to-tip
rule is a stronger baseline and should match all five without remote calls.
A model can match useful-tip coverage over quiet without earning an advantage
over that rule. This tiny authored cohort does not measure player benefit,
calibration, production reliability or general game quality. Nothing promotes
ambient behavior automatically.

From the repository root:

```shell
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-evaluate.coax manifest
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-evaluate.coax request 0
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-evaluate.coax observation capture-case0.json
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-evaluate.coax summary captures.jsonl
shasum -a 256 -c psionic/fixtures/guard-guide-requests-v1/SHA256SUMS
```

The shared tool runner invalidates cached binaries for imported Coax changes in
`coax/`, `verse/`, `psionic/`, `apps/` and `proto/`. It also retains the entry
file, native platform and compiler checks, including entry files outside the
repository. A focused transitive-import smoke kept that external entry unchanged
and observed 3 → 4 → 13 → 112 after isolated proto, Coax web and Verse edits.

The Coax CLI uses the shared `sys` argv/file effects. Required capture fields
are `case`, `request_sha256`, `elapsed_ms`, `status` and `body`. The request hash
must match that frozen case. Known fields must occur once, including escaped
key aliases. Cases are 0–4, status is an integer 0–599, elapsed time is an integer
0–2^53-1, and body is a string of at most 8192 decoded bytes. A record is at most
65536 input bytes and a JSONL report at most 262144. Extra metadata is inert.
Malformed, mismatched, duplicate or unscorable evidence fails visibly.

Reports separate raw choice correctness, quiet, wrong nonquiet choice, contract,
deadline, semantic legality, admission and effective fallback. A rejected or
late model choice does not gain accuracy credit from a correct local action.
A legal admitted quiet answer counts as admission but not useful directions.
Development and held-out coverage remain separate, and missing cases remain
missing. Attempt timings include failures; admitted timings contain only covered
answers. Median is exact, and nearest-rank p95 is the maximum for at most five
samples. These ranks do not estimate production tail latency.

The tokenizer-only preflight opens no model and makes no network request:

```shell
coax/web/tool.sh psionic/tests/guard-guide/guard-guide-token-capacity.coax psionic/fixtures/text/ch05/qwen35.pst
```

It checks the pinned original table and the frozen request hashes, then runs the
same request decoder, renderer and preparation path as the backend. It refuses
any request exceeding the measured resident context capacity of 512 tokens.
The first frozen measurement is 237/237/237/236/238 full tokens, with prefixes
107/107/107/106/108 and suffix 130 in every case.

Focused checks, without broad imported mutation or whole-model CPU inference:

```shell
coax/bin/coax test verse/everglade/guard-guide-eval.coax
coax/bin/coax mutate verse/everglade/guard-guide-eval.coax --root-only --jobs 6
coax/bin/coax test verse/everglade/guard-guide-eval-wire.coax
coax/bin/coax mutate verse/everglade/guard-guide-eval-wire.coax --root-only --jobs 6
coax/bin/coax mutate psionic/tests/guard-guide/token-policy.coax --root-only --jobs 4
psionic/tests/guard-guide/check-evaluation.sh /tmp/guard-guide-evaluation-check
```

The C native harness is an independent oracle for masks, counters, time bounds
and ABI canaries. Its debug, release and ASan/UBSan checks use synthetic model
responses only and are never reported as live model-quality evidence.

`effective_action` is the candidate enum after admission and quiet fallback.
It is not evidence that a player saw a tip. The tools and synthetic/native
checks do not qualify page rendering or establish player-visible benefit.
