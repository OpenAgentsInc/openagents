---
id: edge-case.infinite-periodic-clusters
version: 1
kind: edge-case
title: Do not treat an unbounded periodic contact component as a discrete cluster node
summary: >-
  A contact component that propagates through unit-cell translations is an
  extended periodic subnetwork, not a finite cluster with a well-defined
  cluster centroid. Detect unboundedness before applying cluster-node rules.
tags: [crystallography, periodic-graphs, hydrogen-bonding, supramolecular]
applies_when: >-
  Grouping intermolecular contacts into candidate supramolecular nodes in a
  periodic structure.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - hof-topology-interpenetration
  cites:
    - Gross, J. L. & Tucker, T. W., *Topological Graph Theory*, §2.4, “Voltage Graphs” (1987).
    - Arunan, E. et al., “Definition of the hydrogen bond (IUPAC Recommendations 2011),” *Pure and Applied Chemistry* 83(8) (2011), §2.
evidence: []
---

## Details

A connected component in the finite quotient graph does not necessarily represent a finite physical cluster: contacts may connect it to translated images of itself. In the lifted graph, a cycle with nonzero translation gain can be repeated indefinitely, so the component is unbounded. Do not force such a component into one centroid node or infer its extent from an arbitrary search cutoff. Model the extended motif as a periodic subnetwork, or split it into chemically justified finite interaction units before constructing the higher-level net. A bounded cluster can be assigned a geometric centroid only after its contributing atoms or groups have been unambiguously assigned to one finite set of images.

This follows from the voltage-graph lift and cycle-gain interpretation in Gross and Tucker, *Topological Graph Theory*, §2.4, “Voltage Graphs.” For hydrogen-bond identification terminology, see Arunan et al., “Definition of the hydrogen bond (IUPAC Recommendations 2011),” *Pure and Applied Chemistry* 83(8), §2.
