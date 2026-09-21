# Source and verification notes

The [transcription](../design/thoughts-on-a-typesafe-coding-agent.md) preserves the
content of the public Google Doc
[[public] thoughts on a typesafe coding agent](https://docs.google.com/document/d/1G61uUB0FifUnmmrPzFQojZ3KpczYKmXGpgEXDJ2l_Zg/preview),
retrieved on 2026-09-21.

The text was checked against all four document-model chunks in the Desktop
file `[public] thoughts on a typesafe coding agent - Google Docs.html`.
Its companion folder contains Google Docs application assets, not the
document's embedded images. The public export supplied all six images.

- [original.pdf](original.pdf) is the unchanged, 12-page Google Docs PDF
  export. Use it for the rendered fonts, colors, spacing, code block, and
  image sizes. Google adds a cover page with the tab name, “Why yet another
  agent.”
- [original.html](original.html) retains the HTML export's markup and CSS.
  Only the six embedded image URLs change, from inline data to local files
  under `images/`. The export references Google's hosted Roboto Mono font.
  Google's HTML export retains internal code-block delimiters; the PDF is
  the reference for that block's appearance.
- `images/` contains the original image bytes, in document order. No image
  was resized, cropped, or recreated.

The Markdown transcription preserves the wording, punctuation, capitalization,
typos, heading levels, nested lists, emphasis, links, code indentation, and
image order. Markdown uses the reader's fonts and spacing; use the PDF for
the original rendered formatting. The title and source notes above the
transcription, and the image alt text, identify the imported material.

Verification compared the export with all 13,499 characters in the saved
document model, accounting for Google's image placeholders, horizontal rule,
code-block delimiters, tab heading, and terminal empty paragraph. The rendered
Markdown was checked for text, all 169 list items and their nesting, nine
headings, bold and italic spans, 15 links, and code whitespace. All six image
placements and dimensions match the saved model. The PDF was visually reviewed;
the retained PDF and image files match the public export byte for byte.
