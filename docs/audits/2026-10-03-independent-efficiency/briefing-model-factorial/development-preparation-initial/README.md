# Initial structured development preview

This preview failed the intended retrieval coverage check before the packer
froze and before any scored model call used it. A named documentation file
fell back to the root Cargo manifest, admitting unrelated workspace test
filenames. The 24-file read bound excluded the useful crate test, and omission
metadata consumed much of the 16 KiB payload.

The exact [payload](treatment.md) and [measurement](preparation.json) remain
retained. It includes the fault declaration and relevant specification
sections, but no test fixture. This is development evidence, not an executor
result. The next revision restricts automatic test admission to packages
identified by explicit Rust or manifest references.

Preparation took 0.226 seconds with the supplied index. A fresh index artifact
took 1.944 seconds. The old focused-policy output remained byte-identical
under the new binary.
