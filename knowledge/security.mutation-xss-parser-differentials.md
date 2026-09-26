---
id: security.mutation-xss-parser-differentials
version: 1
kind: edge-case
title: Account for parser differentials in HTML sanitization
summary: >-
  A sanitizer's parse tree may differ from the browser DOM produced after
  serialization, especially across HTML/SVG parsing contexts. Treat sanitizer
  output as untrusted until reparsed and checked under browser-equivalent
  parsing semantics.
tags: [security, xss, html, svg, sanitization]
applies_when: >-
  HTML is parsed and rewritten with a non-browser parser, then delivered to
  browsers; particularly relevant to raw-text elements, foreign content, and
  namespace transitions.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - break-filter-js-from-html
  cites:
    - WHATWG, HTML Living Standard, “The rules for parsing tokens in foreign content” and “The rules for parsing tokens in RCDATA and RAWTEXT elements”
    - Beautiful Soup documentation, “Modifying the tree” and “Differences between parsers”
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

HTML sanitization is a parse-transform-serialize pipeline, not merely string filtering. A server-side parser can represent markup differently from the browser's HTML parser; serialization followed by browser parsing may create active elements or attributes that were absent from the sanitizer's in-memory tree. This class of parser differential is commonly called mutation XSS (mXSS).

Do not rely solely on removing known tag names or attributes from one parser's tree. Use a maintained HTML sanitizer designed for the target browser parsing model, apply a restrictive allowlist, and validate the serialized result by reparsing it with browser-equivalent semantics. Include foreign-content and raw-text parsing contexts in security regression tests. Avoid modifying already-sanitized markup with a second parser or unsafe string substitutions.

Sources: WHATWG, *HTML Living Standard*, sections “The rules for parsing tokens in foreign content” and “The rules for parsing tokens in RCDATA and RAWTEXT elements”; Beautiful Soup documentation, *Modifying the tree* and parser differences documentation (the selected parser affects the resulting tree).
