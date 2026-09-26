---
id: security.html-xss-sanitization-preservation
version: 1
kind: method
title: Sanitize HTML by parsing and removing executable constructs
summary: >-
  When filtering untrusted HTML, remove active elements and executable
  attributes/URLs while preserving ordinary markup; validate security and
  preservation separately. Parser-based serialization may normalize
  formatting, so it cannot meet byte-preservation requirements.
tags: [html, xss, sanitization, python]
applies_when: >-
  A program rewrites HTML supplied by users or files to remove script
  execution while retaining safe content and structure.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - filter-js-from-html
  cites:
    - OWASP, Cross Site Scripting Prevention Cheat Sheet, HTML Sanitization and Dangerous Contexts
    - WHATWG, HTML Living Standard, Parsing HTML documents and The script element
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

HTML sanitization is context-sensitive: executable content is not limited to `<script>` elements. Remove script-capable elements according to policy, event-handler attributes (`on...`), dangerous URL schemes in URL-bearing attributes, and unsafe CSS constructs. Handle nested HTML contexts such as `srcdoc`, and meta refresh navigation, rather than assuming a single regex over the source is sufficient. Canonicalize/decode entities and obfuscation for security checks, but make removal decisions against parsed attributes and elements; preserve unrelated attributes and content.

A parser/serializer commonly changes whitespace, attribute ordering/quoting, void-element syntax, and malformed-markup recovery. If exact source formatting matters, use a source-preserving strategy or explicitly accept parser normalization; do not claim byte preservation based on a DOM-equivalent result. Treat sanitizer policy as defense in depth and test on the browser/parser behavior relevant to the application.

Sources: OWASP, *Cross Site Scripting Prevention Cheat Sheet*, sections “HTML Sanitization” and “Dangerous Contexts”; WHATWG, *HTML Living Standard*, sections “Parsing HTML documents” and “The `script` element”.

## How to check

Run adversarial and benign fixtures separately. After sanitization, parse the output and assert that script-capable elements, event attributes, and dangerous schemes are absent, while representative benign structure and text remain. Also compare source bytes when preservation is required:

```python
from bs4 import BeautifulSoup
soup = BeautifulSoup(output, "html.parser")
assert not soup.find("script")
assert not any(name.lower().startswith("on") for tag in soup.find_all(True)
               for name in tag.attrs)
assert soup.find("table") is not None
```

Add cases with entity-encoded and whitespace-obfuscated schemes, CSS, meta refresh, and nested document attributes; test serialized output in a browser when practical.
