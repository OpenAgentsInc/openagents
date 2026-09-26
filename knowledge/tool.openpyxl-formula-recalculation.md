---
id: tool.openpyxl-formula-recalculation
version: 2
kind: tool
title: Openpyxl writes formulas but does not calculate them
summary: >-
  When a workbook contains formulas, saving it with openpyxl does not evaluate
  them. Validate formula-driven deliverables with a spreadsheet calculation
  engine or compatible formula evaluator, and compare calculated outputs with
  the independently generated results.
tags: [python, openpyxl, excel, validation]
applies_when: >-
  Generating a spreadsheet with formulas using openpyxl and needing to verify
  that formula results reproduce a separate calculation.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - fin-saccr-rwa
  cites:
    - openpyxl documentation, Usage, Using formulae
    - formulas project documentation, ExcelModel API
evidence: []
---

## Details

Openpyxl stores formula expressions but does not calculate them. A workbook loaded with `data_only=True` may therefore expose cached values from an earlier spreadsheet-engine save, or no calculated value at all. Do not treat the presence of formula strings—or a successful save—as evidence that the workbook reproduces the calculation.

For a check, evaluate the saved workbook with a compatible calculation engine or formula evaluator, then compare selected and aggregate outputs against the independently produced results. Include both trade-level intermediates and final totals where possible. Formula evaluators may not implement every Excel function or behavior, so verify that the formulas used are supported; for high-assurance delivery, recalculate with the intended spreadsheet application and reopen the saved workbook to inspect cached values.

Sources: openpyxl documentation, “Using formulae”; formulas project documentation, `ExcelModel` API.

## How to check

After saving, reopen the workbook and inspect that expected cells contain formulas (for example, strings beginning with `=`). Then recalculate with a compatible engine and assert that calculated final totals agree with the independently generated output within the chosen numeric tolerance. Also verify that each sheet contains the required formula-driven intermediate rows.
