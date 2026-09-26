---
id: security.http-header-control-character-validation
version: 1
kind: method
title: Reject control characters before normalizing HTTP header fields
summary: >-
  Validate header names and values against CR, LF, and NUL before any
  normalization or storage to prevent response splitting; exercise every
  public setter and supported text/byte input path.
tags: [security, http, headers, crlf-injection]
applies_when: >-
  Implementing or reviewing HTTP header maps, response header setters, or any
  code that serializes caller-provided header names and values onto an HTTP
  wire.
status: admitted
author: microcoder kb harvest (gpt-6-luna)
provenance:
  written_from:
    - fix-code-vulnerability
  cites:
    - "MITRE, CWE-93: Improper Neutralization of CRLF Sequences ('CRLF Injection'), Description and Mitigations"
    - "R. Fielding et al., RFC 9110: HTTP Semantics, Section 5, Field Lines"
evidence:
  - "admitted 2026-09-26 by review: round2-oos-review"
---

## Details

Header injection occurs when attacker-controlled line terminators in a header field are serialized as framing delimiters, allowing an input value to create additional headers or a response body. Reject carriage return (`\r`), line feed (`\n`), and NUL (`\0`) in both names and values at the common conversion boundary, before applying case/title conversion, underscore replacement, or storage. Centralizing the check ensures append, replace, assignment, and default-setting APIs cannot bypass it. Convert bytes to the library's text representation before checking so text and byte inputs receive equivalent treatment. Raise a consistent validation exception rather than silently stripping characters, since stripping can alter field semantics.

This control is consistent with CWE-93, “Improper Neutralization of CRLF Sequences,” MITRE CWE, entry 93, description and mitigations; and RFC 9110, *HTTP Semantics*, Field Lines (Section 5), which defines field-line structure and restrictions on field values.

## How to check

For every public header mutation API, test each forbidden character at the beginning, middle, and end of both a field name and value, using both text and byte inputs where supported. Include both CRLF orders. Each case should raise the documented validation error and leave the header collection unchanged. For example:

```python
for bad in ("\r", "\n", "\0", "\r\n", "\n\r"):
    for make in (str, lambda s: s.encode("ascii")):
        value = make("safe" + bad + "suffix")
        with pytest.raises(ValueError):
            response.set_header("X-Test", value)
```

Also verify valid ordinary headers still round-trip through the response serializer.
