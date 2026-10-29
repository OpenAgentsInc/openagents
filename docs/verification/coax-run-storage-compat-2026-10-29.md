# Coax aggregate storage and the Nostr JSON boundary

The 0.18.1 live-slot limit of 1,048,576 refused an existing Nostr JSON
`#test` that creates a 1,048,577-byte input to check the inclusive original
input boundary. This was a real compatibility regression, found before the
Nostr event task was resolved. The source test and its boundary remain intact.

Coax 0.18.2 raises the checked Coax policy's live aggregate capacity to
1,572,864 slots, retaining the independent 131,072-container limit and the
unchanged 100,000 default evaluation-step limit. The slot ceiling is 50%
higher than the first limit and leaves bounded headroom for the existing
one-megabyte test and its surrounding values; it is not a total memory or
hostile-source CPU limit. An isolated run of that unchanged JSON check measured
1,049,439 peak tracked slots and 9 containers across its frontend and check.
The previously identified unmetered traversal work remains a separate task.

With the candidate compiler (SHA-256
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`):

- All 177 checks in the in-flight Nostr event graph passed, including the
  original JSON boundary; compiling that graph with its authority checks to C
  also passed, and the emitted C compiled with the native C compiler.
- All 146 checks of the already-shipped experimental phrase-gate graph passed.
- The ten existing resource tests passed after adapting the retained-constant
  boundary to the new ceiling. A new integration test runs the actual Nostr JSON
  checks so a too-small future ceiling is caught in the compiler suite.
- The Coax resource policy passed four check groups and killed all 35 mutants;
  a current-source Python-free stage0 regenerated identical compiler C.
  Release/status drift and book structure checks passed.

The event's independent mutation and crypto evidence remains tied to its own
pinned compiler and source snapshot. These checks establish compatibility with
this measured graph, not a guarantee that every possible program fits the cap.
