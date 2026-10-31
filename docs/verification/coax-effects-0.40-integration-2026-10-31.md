# Coax effect inventory on the merged 0.40.0 frontend

The existing compiler-owned may-effect inventory remains compatible with the
new built-in generic constraints. A private compiler built from immutable
publication commit `3c588727e91a124ea38dfcd3768cb5645c7ae6bd` passed eleven
focused constrained-generic cases and both original consumer inspections.
The full machine-readable [record](coax-effects-0.40-integration-2026-10-31.json)
contains source, generated-artifact and compiler hashes, exact case sources,
commands, captured reports and preserved harness diagnostics.

The private compiler identifies itself as `coax0 0.40.0`; its SHA-256 is
`fe2a8269257f87010f9a65df929f7856ed299db7df4395e8b65acee26f1763e2`.
The compiler-input fingerprint before and after the build is
`95ac0f5c76deecdffb6a17fd348b4e2ac52fa6d1469205454d0a56b26f6752ea`.
This is a focused private integration build, rather than an installed release
or a repeated full bootstrap qualification.

The new cases cover integer and floating `numeric` specializations, their
separate instance keys, checked versus wrapping overflow, nested constrained
calls with a four-procedure foreign-call witness, `ordered` comparison,
`copyable` generic structs, excluded unused templates, and `E0806` refusals for
invalid numeric, ordered and equatable bindings. Rejected bindings emit no
partial effect report.

Both consumer reports match their earlier frozen reports field for field
after canonical source-directory relocation:

| Checked procedure | Procedures | Edges | Sites | Reachable | May-effect mask |
| --- | ---: | ---: | ---: | ---: | ---: |
| Panda `pc_step` | 53 | 236 | 894 | 35 | 125 |
| Verse `verse_orbit_follow` | 81 | 564 | 200 | 19 | 68 |

The unchanged Coax policy and native extractor retain their
[earlier qualification](coax-effects-inventory-2026-10-31.md): seven policy
checks, 199 killed root mutants with zero survivors, and 247 independent
native cases in debug, release and ASan/UBSan. Those standalone gates were
not repeated. Ordinary graph emission and report inspection retained the
compiler's mandatory authority checks. Required check programs ran in the compiler VM; no operational Panda or Verse
session, model inference, GPU dispatch or simulator episode ran.

Constraints govern which operations and bindings a generic body admits.
The inventory follows the checked concrete bodies and records unused templates
separately; it does not infer runtime effects for every possible future binding.
Foreign and intrinsic resources and borrowed regions remain unqualified.
There is no new proof of allocation, blocking, alias/escape, stack use, clocks
or `#bounded`, and this does not complete R11.
