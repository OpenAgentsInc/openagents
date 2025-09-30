#!/usr/bin/env bash
set -euo pipefail

# Quick extractor for SRD 5.2.1 into text chunks
# Requires: pdftotext (poppler)

PDF="docs/srd/SRD_CC_v5.2.1.pdf"
OUT_DIR="docs/srd/.tmp"

if [[ ! -f "$PDF" ]]; then
  echo "Missing PDF: $PDF" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"

echo "Extracting full text (layout) …"
pdftotext -layout "$PDF" "$OUT_DIR/full.txt"

# Page ranges are approximate and may be adjusted as needed.
echo "Extracting Spell Descriptions (pp.104–180) …"
pdftotext -layout -f 104 -l 180 "$PDF" "$OUT_DIR/spells_104_180.txt"

echo "Extracting Monsters (pp.254–343) …"
pdftotext -layout -f 254 -l 343 "$PDF" "$OUT_DIR/monsters_254_343.txt"

echo "Extracting Animals (pp.344–end) …"
pdftotext -layout -f 344 "$PDF" "$OUT_DIR/animals_344_end.txt"

echo "Done. Output in $OUT_DIR"

