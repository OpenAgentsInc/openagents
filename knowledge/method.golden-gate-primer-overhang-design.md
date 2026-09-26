---
id: method.golden-gate-primer-overhang-design
version: 1
kind: method
title: Design Golden Gate PCR primers from intended assembly junctions
summary: >-
  Design type IIS assembly primers by mapping each template-derived fragment
  to the desired final construct, choosing directional junction overhangs, and
  separating 5′ tails from template-annealing regions. Applies to BsaI
  Golden Gate PCR assembly and similar type IIS workflows.
tags: [molecular-biology, golden-gate, primer-design, type-iis]
applies_when: >-
  A construct must be assembled from PCR-amplified template fragments using a
  type IIS restriction enzyme, with the primer tails encoding the designed
  junctions.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - dna-assembly
  cites:
    - New England Biolabs, Golden Gate Assembly Protocol, primer design and overhang selection sections
    - Thermo Fisher Scientific, GeneArt Type IIs Assembly, primer design section
    - Untergasser et al., Primer3—new capabilities and interfaces, Nucleic Acids Research 40(15), 2012, Thermodynamic calculations section
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

First map each source template to the exact interval and orientation needed in the final construct. For circular constructs, identify the retained backbone interval and its wraparound junction explicitly; do not infer fragment order solely from template names or overlaps. Choose a distinct, directional 4-nt overhang at every intended ligation junction, checking that adjacent fragment ends are complementary in the assembly orientation and that the resulting junctions reproduce the target sequence exactly.

For BsaI, the recognition site is `GGTCTC` and cleavage is outside that site. A common primer-tail architecture is a short 5′ flanking sequence, the recognition site, the designed 4-nt overhang, and then the template-annealing sequence. The reverse primer's annealing region is written 5′→3′ as the reverse complement of the desired sequence at the opposite end of the amplicon; its tail must be designed in the strand orientation that yields the intended overhang after digestion. Avoid choosing overhangs that create unwanted internal recognition sites or incompatible junctions.

Choose primer-annealing regions from the template itself; evaluate their melting temperatures independently of the non-annealing 5′ tails, since those tails do not define the initial template-binding event. Match forward and reverse annealing Tm values and check length, GC content, terminal bases, and hairpin/self- and cross-dimer risks. Primer3's `oligotm` can calculate Tm under explicitly supplied salt and concentration assumptions; report those assumptions because Tm depends on them.

Source: New England Biolabs, *Golden Gate Assembly Protocol*, sections on primer design and overhangs; Thermo Fisher Scientific, *GeneArt Type IIs Assembly*, section on primer design; Untergasser et al., “Primer3—new capabilities and interfaces,” *Nucleic Acids Research* 40(15), 2012, section “Thermodynamic calculations.”
