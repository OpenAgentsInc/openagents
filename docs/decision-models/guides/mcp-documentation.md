# MCP documentation tools

`oak-mcp` serves four public documentation tools over both its stdio and
Streamable HTTP transports. These tools do not read credentials, resolve
inference configuration, or contact a network service. Inference tools retain
their separate configured authentication. This implementation does not add
OAuth.

| Tool | Arguments | Result |
| --- | --- | --- |
| `list_docs` | Optional `limit` and `cursor` | Document metadata in corpus order. |
| `read_doc` | Required `id`; optional `max_bytes` and `cursor` | An exact UTF-8 page from a bundled document. |
| `search_docs` | Required `query`; optional `limit` and `cursor` | At most one matching line per document, with a bounded snippet. |
| `get_examples` | Optional `limit` and `cursor` | Example metadata; retrieve content with `read_doc`. |

Results use `openagents.docs.v1`. Each result names the corpus SHA-256; document
metadata includes a stable ID, title, repository path, source link, content
SHA-256, byte length, and example flag. The corpus digest binds the ordered IDs,
titles, paths, contents, and example flags. Source links use the repository's
`main` branch and can change; hashes identify the exact bundled content.

The corpus includes the caller, classification, gateway, admission, API,
OpenAPI, classification-schema, and MCP server documents, plus native and
classification examples.
It preserves each document's statements about implemented and planned behavior.
Reading a specification does not establish deployment availability. Example
model IDs and synthetic policy values require caller-specific configuration
and evidence before production use.

## Pagination and bounds

Lists default to 10 results and accept 1–50. Search accepts 1–256 UTF-8 bytes
of nonblank text and performs a case-insensitive literal line search. It returns
the first matching line per document, numbered from 1, with at most 240 Unicode
scalar values and an explicit snippet truncation flag. It does not execute
regular expressions or rank semantic relevance.

Reads default to 8,192 bytes and accept 4–16,384 bytes per page. Pages end at
UTF-8 boundaries and return their byte offset, truncation status, and next
cursor. Concatenating pages reconstructs the exact document. JSON encoding and
metadata add framing bytes beyond the content bound.

Cursors bind the corpus and tool-specific query or document ID. A cursor from
another tool, query, or corpus is rejected as `stale_cursor`; callers restart
pagination after a binary update changes the corpus. A malformed position or
an offset outside the document is rejected. Document IDs never resolve to an
arbitrary file path or URL. Unknown arguments and out-of-range bounds fail
without inference. A missing document returns `document_not_found`.

## Verification

Library tests reconstruct every bundled document through short UTF-8 pages,
exercise cursor isolation and stale versions, traverse list pages without
duplicates, retrieve examples, and reject invalid bounds and path-like IDs.
Subprocess tests negotiate both supported MCP versions, run all four tools
without endpoint or key configuration, compare structured and text results,
and confirm inference still fails without its configuration.

The tools follow the MCP [tools contract](https://modelcontextprotocol.io/specification/2025-11-25/server/tools).
Protocol fixture clients are not evidence of interoperability with specific
third-party agent applications; that evidence stays with the operators who
deploy it. The Streamable HTTP transport and the inference tool set are
covered in the [MCP server guide](mcp-server.md).
