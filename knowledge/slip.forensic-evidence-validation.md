---
id: slip.forensic-evidence-validation
version: 1
kind: slip
title: Do not trust a matching string without validating its evidence context
summary: >-
  Raw string searches can expose decoys, partial remnants, or unrelated data;
  validate a proposed recovered value against file structure, offsets,
  lengths, and integrity metadata before recording it.
tags: [digital-forensics, data-recovery, validation]
applies_when: >-
  A candidate secret or file payload is found by scanning strings in a disk
  image, binary file, or damaged archive.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - password-recovery
  cites:
    - "NIST, SP 800-86: Guide to Integrating Forensic Techniques into Incident Response, section 3.2"
    - "PKWARE, APPNOTE.TXT: ZIP File Format Specification, sections Local file header and Central directory structure"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

A string that resembles the target format is only a candidate. It may be a decoy, a fragment, or bytes unrelated to the deleted file. Establish provenance by tying it to a recognized file structure and calculating its exact position within the payload. Check independent constraints such as declared payload length, format prefixes/suffixes, character set, and archive CRC. If feasible, reconstruct the enclosing object and have a separate parser validate and extract it.

Keep validation layers distinct: matching a CRC supports byte consistency but is not cryptographic authentication; format checks constrain the candidate but do not establish where it came from. Report only values supported by the combined evidence.

Sources: NIST, *Guide to Integrating Forensic Techniques into Incident Response*, SP 800-86, section 3.2 (forensic examination and evidence handling); PKWARE, *APPNOTE.TXT: ZIP File Format Specification*, sections “Local file header” and “Central directory structure.”
