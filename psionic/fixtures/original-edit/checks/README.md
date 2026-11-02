# Focused check receipts

Initial mutation logs are historical failures, before isolated boundary
checks and proposal sentinel changes. Final logs bind the current authority
sources in `authority.sha256`: 123/123 and45/45 killed, zero survivors.
Dependencies are inherited from base `eead95dc9d`; new authority sources are
in the containing commit. No equivalent-mutant exemptions were added.

The independent oracle verifies these source bindings. Passing unchanged
mutation evidence is carried rather than rerun. Native/example/release logs
are captured separately after the experiment. Compiler setup diagnostics
were observed in the terminal but their initial full output was not archived;
the verification record reports that limitation rather than inventing receipts.
