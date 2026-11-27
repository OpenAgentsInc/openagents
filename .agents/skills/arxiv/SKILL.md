---
name: arxiv
description: >-
  Use when searching arXiv for research papers, reading full arXiv papers
  (HTML, LaTeX source, or PDF), checking specific sections/equations/tables/ablations,
  or comparing multiple papers without blowing up context.
---

# arXiv Research Paper Workflow

Use the right layer for the depth of the question:

## 1. Discovery (Find Papers)
- **Semantic / topic search (best default):** Call `WebSearch` with:
  - `category: "research paper"`
  - `allowed_domains: ["arxiv.org"]`
  - Natural-language query describing the concept, method, or benchmark.
- **Exact metadata / recent category feeds / author lookup:** Use the arXiv Atom API helper:
  ```sh
  python3 .agents/skills/arxiv/scripts/arxiv_paper.py search "agent memory" --category cs.AI --sort submittedDate --max 8
  ```

## 2. Quick Single-Question Read on a Paper URL or PDF
- Call `WebFetch` directly on any `https://arxiv.org/abs/...`, `/html/...`, or `/pdf/...` URL.
- Pass `query` to extract focused passages from a long paper/PDF without loading the whole document:
  ```json
  { "url": "https://arxiv.org/pdf/2505.00675.pdf", "query": "six atomic memory operations", "max_characters": 12000 }
  ```

## 3. Deep Full-Paper Reading, Searching Inside a Paper, Equations & Multi-Paper Comparison
Never dump an entire 30-page paper into one tool output. Fetch it once to the local paper cache (`~/.openagents/papers/<id>/`), inspect its line-numbered `TOC.md`, and use **`Grep`** + **`Read(offset, limit)`**:

```sh
python3 .agents/skills/arxiv/scripts/arxiv_paper.py fetch 2505.00675
```

This creates `~/.openagents/papers/2505.00675/` containing:
- `paper.md` — clean Markdown with preserved LaTeX equations (`$...$` / `$$...$$`) and tables (auto-converted from arXiv HTML, falling back to LaTeX `.tex` source or PDF).
- `TOC.md` — every section heading with exact 1-based line ranges (`L42-L138`) for direct `Read(file_path, offset, limit)`.
- `meta.json` — canonical title, authors, dates, categories, abstract, and URLs.
- `cite.bib` — ready-to-use BibTeX entry.

Then work surgically:
1. Read the printed `TOC.md` to pick relevant section line ranges.
2. Call `Grep(path="~/.openagents/papers/2505.00675/paper.md", pattern="ablation|Table 2|forgetting")` to pinpoint exact terms, tables, or equations.
3. Call `Read(file_path="~/.openagents/papers/2505.00675/paper.md", offset=..., limit=150)` on just the sections you need.
4. Need exact raw LaTeX source (macros, TikZ, exact tables)? Run:
   ```sh
   python3 .agents/skills/arxiv/scripts/arxiv_paper.py fetch 2505.00675 --source tex --force
   ```
   and inspect `~/.openagents/papers/2505.00675/tex/`.
5. List already-cached papers anytime:
   ```sh
   python3 .agents/skills/arxiv/scripts/arxiv_paper.py list
   ```
