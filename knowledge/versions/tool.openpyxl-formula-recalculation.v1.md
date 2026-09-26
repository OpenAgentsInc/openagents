---
id: tool.openpyxl-formula-recalculation
version: 1
kind: tool
title: Openpyxl writes formulas but does not calculate them
summary: >-
  An .xlsx workbook can contain valid formulas without containing updated
  calculated values. Use a spreadsheet calculation engine to recalculate, or
  independently validate formula results before claiming that workbook outputs
  tie.
tags: [openpyxl, excel, formulas, validation]
applies_when: >-
  Generating or checking workbooks with formulas using openpyxl or another
  library that does not implement spreadsheet formula evaluation.
status: candidate
author: microcoder kb harvest (openai/gpt-6-luna)
provenance:
  written_from:
    - fin-saccr-rwa
  cites:
    - openpyxl contributors, *openpyxl documentation*, “Working with formulas”
evidence: []
---

## Details

Openpyxl can write and preserve formula expressions, but it does not evaluate those formulas. Loading a workbook with cached values enabled only reads values previously saved by a spreadsheet application; it does not calculate missing or stale results. Setting workbook calculation properties to automatic may request recalculation when a compatible spreadsheet application opens the file, but is not itself a recalculation step.

When no calculation engine is available, validate the underlying calculations in the generating code and inspect the saved formula expressions and cell references. Be precise in reporting the result: formula presence or an independent code-level tie-out is not proof that the workbook's cached formula values were recalculated. Source: openpyxl contributors, *openpyxl documentation*, “Working with formulas.”

## How to check

```python
from openpyxl import load_workbook

path = "workbook.xlsx"
formulas = load_workbook(path, data_only=False)
values = load_workbook(path, data_only=True)

# Inspect formula expressions and any saved cached values separately.
for sheet in formulas.sheetnames:
    ws_f = formulas[sheet]
    ws_v = values[sheet]
    for row in ws_f.iter_rows():
        for cell in row:
            if isinstance(cell.value, str) and cell.value.startswith("="):
                print(sheet, cell.coordinate, cell.value,
                      "cached:", ws_v[cell.coordinate].value)
```

For a true workbook recalculation check, open and save a copy with Excel or LibreOffice, then reload that copy with `data_only=True` and compare the calculated cells against independent expected values.
