---
id: slip.coordination-sequence-not-topology-proof
version: 1
kind: slip
title: Do not identify a net from its coordination sequence alone
summary: >-
  A matching coordination sequence is a useful topology filter, not proof of
  net identity. Confirm candidate topology with additional local and periodic
  graph invariants or a graph-level comparison.
tags: [crystallography, topology, coordination-sequence, validation]
applies_when: >-
  Assigning an RCSR name to a periodic net inferred from a crystal structure.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - hof-topology-interpenetration
  cites:
    - O’Keeffe, M., Peskov, M. A., Ramsden, S. J. & Yaghi, O. M., “The Reticular Chemistry Structure Resource (RCSR) database of, and symbols and topology for, crystal nets,” *Accounts of Chemical Research* 41 (2008), pp. 1782–1789, “Symbols and topology for crystal nets.”
evidence: []
---

## Details

Different nets can share initial coordination-sequence terms, so matching a short sequence against a reference list can produce a false identification. Use the sequence to shortlist candidates, then compare additional structure such as vertex symbols or point symbols, coordination types, and periodic adjacency including edge gains. Where possible, compare the constructed quotient net directly with a reference net under graph isomorphism that preserves periodic translations. RCSR describes crystal-net symbols and topology in O’Keeffe et al., “The Reticular Chemistry Structure Resource (RCSR) database of, and symbols and topology for, crystal nets,” *Accounts of Chemical Research* 41 (2008), pp. 1782–1789, “Symbols and topology for crystal nets.”

## How to check

For every inequivalent vertex type, compute the coordination sequence and local vertex symbol; reject a candidate if either disagrees. Then verify that the quotient vertices, edge multiplicities, and translation gains match the candidate net. Agreement of only the first few sequence terms is not sufficient.
