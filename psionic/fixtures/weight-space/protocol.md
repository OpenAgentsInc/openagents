# Chapter 253 experiment protocol

Matrix rules written before matrix evaluation, 2026-11-02. Model prompts and
budgets were fixed in the runner before its first call; this note was assembled
while the first model batch was running. This is not an independently
preregistered study. The implementation author has read chapter 119 and knows
its seed-119 result. Fresh GPT-OSS requests have
no conversation history, file access, tools, grader feedback or held-out inputs.
This is a local exploratory study, not a replication of the frontier-model blog.

- Decoder: chapter 119's 664-parameter architecture and 300 AdamW steps; seeds
  119–123, hint weight 0.75; five healthy controls with hint weight 0. Training
  contains agreeing hints and no hints, never conflicting hints. Saved binary64
  artifacts are the experiment identity. Native training can differ from the
  old compile-time fixture; do not claim bit-identical training across builds.
- Development: answers 1–3, each with five conflicting hints. Held out: answers
  4–6, each with five conflicts. Retained cases are each answer with its agreeing
  hint and with no hint, six cases per partition. No held-out-driven retries.
- Baselines: unchanged; exact no-op; informed fixed-head-1 removal; both head
  removals; best of ten head/unit removals selected by development NLL while
  retaining development clean answers; three deterministic 32-position random
  zero patches; untrained and erased-readout controls; function-preserving head
  swap and reversed FFN order. Permuted and original versions are paired cases,
  not independent seeds. Random patches match head-edit count, not norm.
- Report held-out correct/15, retained/6, mean full-vocabulary NLL, confident
  errors (wrong argmax with probability >0.9), hint-induced logit-gap change,
  changed scalars and edit size. A descriptive repair passes only with NLL gain
  >0.01 nats, no loss in held-out correct/retained counts and no extra confident
  errors. This is not a generalization confidence interval or promotion gate.
- Complete-network positive control: constructed 2→4 ReLU→2 network, 22 scalar
  weights/biases, four cyclic hidden-unit orderings, each with a strong nuisance
  circuit, a healthy output or erased output. Class 1 iff x0>0; class 0 wins
  ties. Only the network, architecture and task are sent. Proposer may remove
  two units' outgoing weights or say NONE. Score all 42 combinations of
  x0∈{-3,-2,-1,1,2,3}, x1∈{-3,-2,-1,0,1,2,3}. This construction demonstrates
  reading explicit circuitry; it is not evidence about learned representations.
- GPT-OSS: original 20b on Psionic's Metal Responses server, stateless greedy
  requests, low reasoning, at most 512 output tokens (256 for the longer
  selected-weight decoder prompts), no tools or gradients. Twelve complete
  network calls; ten selected-weight calls; ten development-attention calls.
  Selected-weight prompts omit the FFN and output matrices and round visible
  weights to 0.01. They are explicitly partial observations of a model.
- Decoder activation condition supplies only mean development attention at
  the query to answer/hint positions. It is observational, not a causal oracle.
  Output is a head index or NONE. Only completed final messages with the exact
  declared answer format become edits. Refusals, incomplete outputs and HTTP
  errors are retained as invalid trials. No self-reported explanation is a label.
- Keep every request/response and the source/artifact identities. Routine checks
  replay saved artifacts and proposals; they never require a live server or
  retrain the ten models. Any additional study gets its own protocol/ledger.
