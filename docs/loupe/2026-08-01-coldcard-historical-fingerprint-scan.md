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

Implementation and tests live in
`packages/forensic-contract/src/historical-scan.ts` and its adjacent test file.
The checked-in fixture is synthetic and contains no node secret or live query.
