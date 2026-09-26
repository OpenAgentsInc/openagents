---
id: slip.golden-gate-simulate-before-reporting
version: 1
kind: slip
title: Do not treat plausible overhangs or primer Tm as proof of Golden Gate assembly
summary: >-
  After designing type IIS primers, simulate fragment amplification, cleavage,
  and ligation, then compare the resulting construct sequence—not just
  fragment sizes or junction labels—to the intended product.
tags: [molecular-biology, golden-gate, validation, sequence-assembly]
applies_when: >-
  Primer sequences encode restriction sites and overhangs for Golden Gate or
  another type IIS assembly.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - dna-assembly
  cites:
    - New England Biolabs, Golden Gate Assembly Protocol, type IIS cleavage and assembly design sections
    - Engler, Kandzia & Marillonnet, A One Pot, One Step, Precision Cloning Method with High Throughput Capability, PLoS ONE 3(11), 2008, Methods
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

A primer pair can have good annealing Tm and plausible overhangs yet amplify the wrong interval, reverse a fragment, omit a wraparound backbone segment, or encode an incorrect junction. Validate the complete sequence-level operation: locate each annealing region in the intended template and orientation; derive the PCR product; model cleavage at the enzyme's actual offset from its recognition site; remove the recognition-site-bearing tails as appropriate; and ligate the released ends in the intended circular order. Compare the full simulated product to the target, including the circular boundary, and scan for residual recognition sites that could cause unwanted recutting. Also verify primer names/count and that the written FASTA contains the exact sequences checked.

A useful independent check is to reconstruct the circular product from the source-derived intervals and junction overhangs and assert exact sequence equality with the target (allowing rotation only if the target is circular and its chosen origin differs). Do not use approximate lengths or a matching coordination/feature map as a substitute for this check.

Source: New England Biolabs, *Golden Gate Assembly Protocol*, sections on type IIS cleavage and assembly design; Engler, Kandzia & Marillonnet, “A One Pot, One Step, Precision Cloning Method with High Throughput Capability,” *PLoS ONE* 3(11), 2008, Methods.
