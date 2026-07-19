# Oracles and falsifiers

An oracle states the observable condition and names the evaluator that decides
the result. Make the condition precise enough that the same observation yields
the same verdict. Avoid assertions such as “works,” “looks right,” or “tests
pass” without a bounded object and decision rule.

Every required obligation also needs a falsifier: a controlled counterexample
or mutation that must produce `REFUTED`. The falsifier demonstrates that the
oracle can reject a relevant failure. If no meaningful falsifier can be named,
the oracle is not yet an adequate proof design. Retain `missing_falsifier` or
`obligation_needs_design` rather than inventing confidence.

Do not rewrite an oracle or falsifier after observing a failure merely to make
the current implementation pass. That is a reviewed proof-design change.
