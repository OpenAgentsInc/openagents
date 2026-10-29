# Compiler provenance verification

The site release now checks a compiler built from a private copy of the
declared inputs against the clean release tree before staging. A matching
record lets an unrelated repository change reuse the verified compiler.

For the 0.22.0 change, the Python-free bootstrap completed its existing
compiler and importer checks and produced compiler SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
The resulting record verified against the original tree, and the generated
release status passed its binary check. The Coax provenance policy passed
its complete reference mutation run (23/23 killed). Ten targeted tests
passed, including input and binary changes, unrelated-source reuse, malformed
records, symlinks, the private-snapshot race, and rejection before a release
can write to staging.

The identity is a consistency check, not a signature or a proof that the
compiler is correct. A local writer able to replace both the record and the
artifacts can forge it. The environment and toolchain are identified, but
system headers and libraries are not content-pinned.
