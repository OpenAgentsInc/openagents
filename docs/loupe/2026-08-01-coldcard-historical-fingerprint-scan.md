# Coldcard historical fingerprint scan

OFR-016 turns the postmortem's transaction-builder observations into a
deterministic, bounded historical evaluator. It is not a live-wallet hunter and
it cannot attribute a person, intent, theft, or victim.

## Inputs and capability

A historical bundle densely binds each height, block hash, raw block digest,
decoded transaction, source identity, range, network, and genesis hash into one
canonical content digest. Wider scans use a separately admitted, broker-bound
private Bitcoin Core capability. The capability exposes only selected read-only
chain methods. Its contract has no host, port, URL, cookie, RPC credential,
provider credential, arbitrary endpoint, external guest IP, or wallet RPC.

## Scan order

The scanner first reproduces every frozen positive at its expected exact fee
rate and with complete required prevout data. A failed self-test stops before
the wide scan. Cheap filtering then evaluates transaction version, zero
locktime, one P2WPKH output, homogeneous supported inputs, allowed sequences,
and integer fee-table divisibility. Expensive resolution receives exactly that
ordered candidate set. Missing fee data fails the run; missing required prevout
data fails the candidate. Neither becomes a zero-hit success.

Raw hits retain integer satoshi fee and value inputs, estimated vbytes, exact
fee rate, input/script shape, sequence values, signature-R width counts, source
coincidence, block time, block height, and the fingerprint revision. No floating
BTC conversion participates in matching.

## Resume and base rates

Each completed block appends a checkpoint binding block hash, prior checkpoint,
fingerprint revision, and all raw hits so far. Resume validates every binding.
Starting after any completed checkpoint must reproduce the same canonical raw-
hit and normalized txid-set digests as a one-shot scan.

Base rates retain eligible and match counts plus matches per million for each
fee-rate, block-era, input-script, and fingerprint-revision stratum. These rates
make collision limits visible; a match supports program similarity only.

## Controls

A block-range negative control cannot discriminate on real chain data. The
exact-integer fee rule fires somewhere in essentially every mainnet block, so
"this block holds no hit" is either unsatisfiable or fitted, and a declared
negative-control block the bundle does not even contain now fails instead of
passing by silence. The control that does discriminate perturbs the fee table:
every definition carries at least one mutation control, and a scan only
succeeds when no known positive survives the perturbed table.

## What is frozen, and what it measures

The checked-in fixture is no longer synthetic. Blocks 960,189, 960,359, and
960,365-960,367 were extracted read-only from our own archival node and frozen
as content-addressed bundles under `fixtures/forensics/coldcard/`. Between them
they carry all eight known-positive transactions the postmortem published, and
all eight reproduce at their published exact fee rates through this scanner.
Moving the fixed overhead by a single vbyte in either direction leaves zero of
them matching and drops the whole bundle's match count several-fold.

The bundles are too small to carry a base rate. That denominator comes from the
wide-scan ledger: the per-block record of 1,701 blocks and 7,122,744 eligible
transactions scanned on the same node, retained with each block's hash,
eligibility count, match count, and prevout-error count, and folded into era
totals and a fee-rate histogram whose per-million figures must rebuild from the
counts. The measured false-match rate is 2,820-6,154 per million at 2 sat/vB
and 24-701 per million at 30 sat/vB. At 2 sat/vB the fingerprint alone carries
nothing. At 30 sat/vB it carries a claim only in combination with shape,
clustering, and value.

The ledger refuses a match count its own retained blocks do not sum to, refuses
matches at all when the known-positive self-test did not pass, and refuses a
credible match count when any block reported missing prevout data. Zero hits is
never evidence of absence unless the run proved it had the data.

Real data also found a real defect. Checkpoint digests were computed over hits
in the order a run happened to append them, which is stable only when a block
holds at most one hit. On real blocks a resumed scan rebuilt the same hits in a
different order and its checkpoint digest stopped matching. Checkpoints and
resumes now both commit to the canonical deduplicated, height-then-txid
ordering. The four-block synthetic fixture could not have surfaced that.

Implementation and tests live in
`packages/forensic-contract/src/historical-scan.ts` and its adjacent test file.
The fixture carries an opaque node-identity digest and server binding ref and no
node secret, endpoint, cookie, credential, wallet method, or live query. Nothing
here establishes a person, an intent, a theft, or live-wallet scope: the claim
ceiling on every report and on the ledger is `program_similarity_only`.
