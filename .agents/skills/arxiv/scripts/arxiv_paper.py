#!/usr/bin/env python3
# Firm reason: 5 (CLI helper script for the arxiv skill: fetches arXiv metadata, PDF text, and LaTeX source into ~/.openagents/papers/)
"""OpenAgents arXiv paper helper (pure Python 3 stdlib).

Makes discovering, caching, searching, and reading full arXiv papers effortless for agents:
  - `fetch <id-or-url>`: pulls metadata (Atom API) + full text (HTML -> clean Markdown with
    preserved LaTeX equations, or LaTeX source `.tex`, or PDF) into ~/.openagents/papers/<id>/
    with a line-numbered TOC.md for surgical Read/Grep.
  - `search <query>`: structured arXiv Atom API search (category/date/author/exact IDs).
  - `toc <id-or-url>`: print the cached line-numbered table of contents (fetching if needed).
  - `list`: list locally cached papers.
"""

from __future__ import annotations

import argparse
import gzip
import html
import io
import json
import os
import re
import shutil
import subprocess
import sys
import tarfile
import urllib.error
import urllib.parse
import urllib.request
import xml.etree.ElementTree as ET
from html.parser import HTMLParser
from pathlib import Path
from typing import Any

USER_AGENT = "OpenAgents-ArxivSkill/1.0 (+https://openagents.com)"
ATOM_NS = {
    "atom": "http://www.w3.org/2005/Atom",
    "arxiv": "http://arxiv.org/schemas/atom",
}

NEW_ID_RE = re.compile(r"\b(\d{4}\.\d{4,5})(v\d+)?\b")
OLD_ID_RE = re.compile(r"\b([a-z\-]+(?:\.[A-Z]{2})?/\d{7})(v\d+)?\b")


def papers_root() -> Path:
    env = os.environ.get("OA_PAPERS_DIR", "").strip()
    if env:
        return Path(env).expanduser()
    return Path.home() / ".openagents" / "papers"


def parse_arxiv_id(raw: str) -> tuple[str, str]:
    """Return (canonical_id_with_optional_version, base_id_without_version)."""
    s = raw.strip()
    s = re.sub(r"^arXiv:", "", s, flags=re.IGNORECASE)
    s = re.sub(r"\.pdf$", "", s, flags=re.IGNORECASE)
    # Strip query/fragment if URL
    s = s.split("?", 1)[0].split("#", 1)[0]

    m = NEW_ID_RE.search(s)
    if m:
        base = m.group(1)
        ver = m.group(2) or ""
        return base + ver, base

    m = OLD_ID_RE.search(s)
    if m:
        base = m.group(1)
        ver = m.group(2) or ""
        return base + ver, base

    raise ValueError(f"Could not parse arXiv ID from: {raw!r}")


def cache_dir_for(arxiv_id: str) -> Path:
    safe = arxiv_id.replace("/", "_")
    return papers_root() / safe


def http_get(url: str, timeout: int = 30) -> tuple[int, bytes, str]:
    req = urllib.request.Request(
        url,
        headers={
            "User-Agent": USER_AGENT,
            "Accept-Encoding": "gzip",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read()
            if resp.headers.get("Content-Encoding", "").lower() == "gzip":
                raw = gzip.decompress(raw)
            ctype = resp.headers.get("Content-Type", "")
            return resp.status, raw, ctype
    except urllib.error.HTTPError as e:
        return e.code, b"", ""
    except urllib.error.URLError as e:
        raise RuntimeError(f"Network error fetching {url}: {e}") from e


def clean_ws(s: str) -> str:
    return re.sub(r"\s+", " ", s or "").strip()


def fetch_metadata(arxiv_id: str) -> dict[str, Any]:
    url = f"https://export.arxiv.org/api/query?id_list={urllib.parse.quote(arxiv_id)}"
    status, data, _ = http_get(url, timeout=25)
    if status != 200 or not data:
        raise RuntimeError(f"arXiv Atom API returned HTTP {status} for {arxiv_id}")
    root = ET.fromstring(data)
    entry = root.find("atom:entry", ATOM_NS)
    if entry is None:
        raise RuntimeError(f"No arXiv entry found for {arxiv_id}")

    raw_id = clean_ws(entry.findtext("atom:id", "", ATOM_NS))
    canonical_id, base_id = parse_arxiv_id(raw_id or arxiv_id)
    title = clean_ws(entry.findtext("atom:title", "", ATOM_NS))
    abstract = clean_ws(entry.findtext("atom:summary", "", ATOM_NS))
    published = clean_ws(entry.findtext("atom:published", "", ATOM_NS))
    updated = clean_ws(entry.findtext("atom:updated", "", ATOM_NS))
    authors = [
        clean_ws(a.findtext("atom:name", "", ATOM_NS))
        for a in entry.findall("atom:author", ATOM_NS)
        if clean_ws(a.findtext("atom:name", "", ATOM_NS))
    ]
    categories = [
        c.attrib.get("term", "")
        for c in entry.findall("atom:category", ATOM_NS)
        if c.attrib.get("term")
    ]
    prim_el = entry.find("arxiv:primary_category", ATOM_NS)
    primary_category = prim_el.attrib.get("term", categories[0] if categories else "") if prim_el is not None else (categories[0] if categories else "")
    comment = clean_ws(entry.findtext("arxiv:comment", "", ATOM_NS))
    journal_ref = clean_ws(entry.findtext("arxiv:journal_ref", "", ATOM_NS))
    doi = clean_ws(entry.findtext("arxiv:doi", "", ATOM_NS))

    return {
        "id": canonical_id,
        "base_id": base_id,
        "title": title,
        "authors": authors,
        "published": published,
        "updated": updated,
        "primary_category": primary_category,
        "categories": categories,
        "abstract": abstract,
        "comment": comment,
        "journal_ref": journal_ref,
        "doi": doi,
        "abs_url": f"https://arxiv.org/abs/{canonical_id}",
        "html_url": f"https://arxiv.org/html/{canonical_id}",
        "pdf_url": f"https://arxiv.org/pdf/{canonical_id}.pdf",
        "eprint_url": f"https://arxiv.org/e-print/{canonical_id}",
    }


def make_bibtex(meta: dict[str, Any]) -> str:
    first_author = (meta["authors"][0].split()[-1] if meta["authors"] else "arxiv").lower()
    first_author = re.sub(r"[^a-z0-9]", "", first_author) or "arxiv"
    year = (meta["published"] or "")[:4] or "2026"
    slug = re.sub(r"[^a-z0-9]", "", meta["base_id"].lower())
    key = f"{first_author}{year}_{slug}"
    authors_str = " and ".join(meta["authors"])
    lines = [
        f"@article{{{key},",
        f"  title = {{{meta['title']}}},",
        f"  author = {{{authors_str}}},",
        f"  year = {{{year}}},",
        f"  eprint = {{{meta['base_id']}}},",
        f"  archivePrefix = {{arXiv}},",
    ]
    if meta.get("primary_category"):
        lines.append(f"  primaryClass = {{{meta['primary_category']}}},")
    if meta.get("doi"):
        lines.append(f"  doi = {{{meta['doi']}}},")
    lines.append(f"  url = {{{meta['abs_url']}}},")
    lines.append("}")
    return "\n".join(lines) + "\n"


SKIP_TAGS = {"script", "style", "nav", "footer", "noscript", "svg", "button"}
SKIP_CLASS_TOKENS = {
    "ltx_page_navbar",
    "ltx_page_footer",
    "ltx_navigation",
    "ltx_page_logo",
    "ltx_ERROR",
    "ltx_role_refnum",
    "sr-only",
    "ar5iv-footer",
    "package-alerts",
}


class ArxivHTMLToMarkdown(HTMLParser):
    """Converts arXiv LaTeXML HTML into clean Markdown while preserving LaTeX equations."""

    def __init__(self) -> None:
        super().__init__(convert_charrefs=True)
        self.out: list[str] = []
        self.skip_depth = 0
        self.math_depth = 0
        self.in_pre = False
        self.in_code = False
        self.in_heading = 0
        self.list_stack: list[tuple[str, int]] = []
        self.in_table = False
        self.table_rows: list[list[str]] = []
        self.current_row: list[str] = []
        self.cell_buf: list[str] | None = None
        self.in_article = False
        self.saw_article = False
        self.article_depth = 0

    def _emit(self, text: str) -> None:
        if self.cell_buf is not None:
            self.cell_buf.append(text)
        else:
            self.out.append(text)

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        tag = tag.lower()
        attr_map = {k.lower(): (v or "") for k, v in attrs}
        classes = set(attr_map.get("class", "").split())

        if tag == "article":
            self.saw_article = True
            self.in_article = True
            self.article_depth += 1

        if self.skip_depth > 0:
            self.skip_depth += 1
            return

        if tag in SKIP_TAGS or (classes & SKIP_CLASS_TOKENS):
            self.skip_depth = 1
            return

        if self.math_depth > 0:
            self.math_depth += 1
            return

        if tag == "math":
            alt = clean_ws(attr_map.get("alttext", ""))
            display = attr_map.get("display", "").lower() == "block"
            if alt:
                if display and self.cell_buf is None:
                    self._emit(f"\n\n$$\n{alt}\n$$\n\n")
                else:
                    self._emit(f"${alt}$")
            self.math_depth = 1
            return

        if tag in ("h1", "h2", "h3", "h4", "h5", "h6"):
            lvl = int(tag[1])
            self.in_heading = lvl
            self._emit(f"\n\n{'#' * lvl} ")
        elif tag in ("p", "div", "section", "figcaption"):
            if tag == "figcaption":
                self._emit("\n\n**Figure/Table Caption:** ")
            elif tag == "p":
                self._emit("\n\n")
        elif tag == "br":
            self._emit("\n")
        elif tag == "pre":
            self.in_pre = True
            self._emit("\n\n```\n")
        elif tag == "code" and not self.in_pre:
            self.in_code = True
            self._emit("`")
        elif tag in ("strong", "b"):
            self._emit("**")
        elif tag in ("em", "i"):
            self._emit("*")
        elif tag == "ul":
            self.list_stack.append(("ul", 0))
            self._emit("\n")
        elif tag == "ol":
            self.list_stack.append(("ol", 0))
            self._emit("\n")
        elif tag == "li":
            indent = "  " * max(0, len(self.list_stack) - 1)
            if self.list_stack and self.list_stack[-1][0] == "ol":
                kind, cnt = self.list_stack[-1]
                cnt += 1
                self.list_stack[-1] = (kind, cnt)
                self._emit(f"\n{indent}{cnt}. ")
            else:
                self._emit(f"\n{indent}- ")
        elif tag == "blockquote":
            self._emit("\n\n> ")
        elif tag == "table" and "ltx_eqn_table" not in classes and "ltx_equationgroup" not in classes:
            self.in_table = True
            self.table_rows = []
        elif tag == "tr" and self.in_table:
            self.current_row = []
        elif tag in ("td", "th") and self.in_table:
            self.cell_buf = []

    def handle_endtag(self, tag: str) -> None:
        tag = tag.lower()
        if tag == "article" and self.article_depth > 0:
            self.article_depth -= 1
            if self.article_depth == 0:
                self.in_article = False

        if self.skip_depth > 0:
            self.skip_depth -= 1
            return

        if self.math_depth > 0:
            self.math_depth -= 1
            return

        if tag in ("h1", "h2", "h3", "h4", "h5", "h6"):
            self.in_heading = 0
            self._emit("\n\n")
        elif tag == "p":
            self._emit("\n\n")
        elif tag == "pre":
            self.in_pre = False
            self._emit("\n```\n\n")
        elif tag == "code" and not self.in_pre:
            self.in_code = False
            self._emit("`")
        elif tag in ("strong", "b"):
            self._emit("**")
        elif tag in ("em", "i"):
            self._emit("*")
        elif tag in ("ul", "ol"):
            if self.list_stack:
                self.list_stack.pop()
            self._emit("\n\n")
        elif tag in ("td", "th") and self.in_table and self.cell_buf is not None:
            cell = clean_ws("".join(self.cell_buf)).replace("|", "\\|")
            self.current_row.append(cell)
            self.cell_buf = None
        elif tag == "tr" and self.in_table:
            if any(c for c in self.current_row):
                self.table_rows.append(self.current_row)
            self.current_row = []
        elif tag == "table" and self.in_table:
            self.in_table = False
            if self.table_rows:
                width = max(len(r) for r in self.table_rows)
                if width > 0 and width <= 20:
                    norm = [r + [""] * (width - len(r)) for r in self.table_rows]
                    lines = [
                        "| " + " | ".join(norm[0]) + " |",
                        "| " + " | ".join(["---"] * width) + " |",
                    ]
                    for r in norm[1:]:
                        lines.append("| " + " | ".join(r) + " |")
                    self._emit("\n\n" + "\n".join(lines) + "\n\n")
            self.table_rows = []

    def handle_data(self, data: str) -> None:
        if self.skip_depth > 0 or self.math_depth > 0:
            return
        if self.saw_article and not self.in_article:
            return
        if self.in_pre:
            self._emit(data)
            return
        text = re.sub(r"[ \t\r\n]+", " ", data)
        if text == " " and (not self.out or self.out[-1].endswith(("\n", " "))):
            return
        self._emit(text)

    def get_markdown(self) -> str:
        raw = "".join(self.out)
        # Clean heading lines (collapse internal newlines inside headings)
        lines: list[str] = []
        for line in raw.splitlines():
            s = line.rstrip()
            if s.startswith("#"):
                s = re.sub(r"\s+", " ", s).strip()
            lines.append(s)
        text = "\n".join(lines)
        text = re.sub(r"\n{3,}", "\n\n", text)
        return text.strip() + "\n"


def convert_html_to_markdown(html_bytes: bytes) -> str:
    text = html_bytes.decode("utf-8", errors="replace")
    parser = ArxivHTMLToMarkdown()
    parser.feed(text)
    parser.close()
    return parser.get_markdown()


def safe_extract_tar(data: bytes, dest: Path) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    base = dest.resolve()
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:*") as tf:
        for member in tf.getmembers():
            if not (member.isfile() or member.isdir()):
                continue
            target = (base / member.name).resolve()
            if not str(target).startswith(str(base) + os.sep) and target != base:
                continue
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                f = tf.extractfile(member)
                if f is not None:
                    target.write_bytes(f.read())


def expand_tex_inputs(tex_path: Path, root_dir: Path, seen: set[Path], depth: int = 0) -> str:
    if depth > 8 or tex_path in seen or not tex_path.is_file():
        return ""
    seen.add(tex_path)
    content = tex_path.read_text(encoding="utf-8", errors="replace")
    # Remove full-line and inline unescaped comments
    content = re.sub(r"(?<!\\)%[^\n]*", "", content)

    def _repl(match: re.Match[str]) -> str:
        inc = match.group(1).strip()
        cand = tex_path.parent / inc
        if not cand.exists() and not inc.endswith(".tex"):
            cand = tex_path.parent / (inc + ".tex")
        if not cand.exists():
            cand = root_dir / inc
            if not cand.exists() and not inc.endswith(".tex"):
                cand = root_dir / (inc + ".tex")
        if cand.is_file():
            return "\n" + expand_tex_inputs(cand, root_dir, seen, depth + 1) + "\n"
        return ""

    return re.sub(r"\\(?:input|include)\{([^}]+)\}", _repl, content)


def tex_to_markdown(tex_root: Path) -> str:
    tex_files = sorted(tex_root.rglob("*.tex"))
    if not tex_files:
        return ""
    main_tex = None
    for f in tex_files:
        txt = f.read_text(encoding="utf-8", errors="replace")
        if "\\begin{document}" in txt:
            main_tex = f
            break
    if main_tex is None:
        main_tex = max(tex_files, key=lambda p: p.stat().st_size)

    raw = expand_tex_inputs(main_tex, tex_root, set())
    if "\\begin{document}" in raw:
        raw = raw.split("\\begin{document}", 1)[1]
    if "\\end{document}" in raw:
        raw = raw.split("\\end{document}", 1)[0]

    # Convert common sectioning commands to Markdown headings
    raw = re.sub(r"\\begin\{abstract\}", "\n\n## Abstract\n\n", raw)
    raw = re.sub(r"\\end\{abstract\}", "\n\n", raw)
    raw = re.sub(r"\\section\*?\{([^{}]*)\}", r"\n\n## \1\n\n", raw)
    raw = re.sub(r"\\subsection\*?\{([^{}]*)\}", r"\n\n### \1\n\n", raw)
    raw = re.sub(r"\\subsubsection\*?\{([^{}]*)\}", r"\n\n#### \1\n\n", raw)
    raw = re.sub(r"\\paragraph\*?\{([^{}]*)\}", r"\n\n**\1.** ", raw)
    raw = re.sub(r"\\(?:textbf|emph|textit)\{([^{}]*)\}", r"**\1**", raw)
    raw = re.sub(r"\\item\b", "\n- ", raw)
    raw = re.sub(r"\\(?:label|ref|eqref|cite[a-z]*|vspace|hspace|centering|maketitle)\*?(?:\[[^\]]*\])?\{[^{}]*\}", "", raw)
    raw = re.sub(r"\n{3,}", "\n\n", raw)
    return raw.strip() + "\n"


def format_header(meta: dict[str, Any], source_kind: str) -> str:
    lines = [
        f"# {meta['title']}",
        "",
        f"- **arXiv ID:** `{meta['id']}`",
        f"- **Authors:** {', '.join(meta['authors'])}",
        f"- **Published:** {meta['published'][:10]} (Updated: {meta['updated'][:10]})",
        f"- **Categories:** {', '.join(meta['categories'])}",
        f"- **URLs:** [abs]({meta['abs_url']}) | [html]({meta['html_url']}) | [pdf]({meta['pdf_url']})",
        f"- **Extracted From:** `{source_kind}`",
    ]
    if meta.get("comment"):
        lines.append(f"- **Comment:** {meta['comment']}")
    if meta.get("journal_ref"):
        lines.append(f"- **Journal:** {meta['journal_ref']}")
    if meta.get("doi"):
        lines.append(f"- **DOI:** {meta['doi']}")
    lines += [
        "",
        "## Abstract",
        "",
        meta["abstract"],
        "",
        "---",
        "",
    ]
    return "\n".join(lines)


def build_toc(paper_md: str) -> str:
    lines = paper_md.splitlines()
    headings: list[tuple[int, int, str]] = []
    in_code = False
    for idx, line in enumerate(lines, start=1):
        if line.strip().startswith("```"):
            in_code = not in_code
            continue
        if in_code:
            continue
        m = re.match(r"^(#{1,4})\s+(.+?)\s*$", line)
        if m:
            lvl = len(m.group(1))
            title = m.group(2).strip()
            headings.append((idx, lvl, title))

    total_lines = len(lines)
    toc_lines = [f"# Table of Contents ({total_lines} lines total)", ""]
    for i, (line_no, lvl, title) in enumerate(headings):
        end_line = (headings[i + 1][0] - 1) if i + 1 < len(headings) else total_lines
        indent = "  " * (lvl - 1)
        toc_lines.append(f"{indent}- L{line_no}-L{end_line}: {title}")
    return "\n".join(toc_lines) + "\n"


def fetch_paper(arxiv_ref: str, source: str = "auto", force: bool = False) -> Path:
    req_id, _ = parse_arxiv_id(arxiv_ref)
    meta = fetch_metadata(req_id)
    out_dir = cache_dir_for(meta["base_id"])
    paper_path = out_dir / "paper.md"
    toc_path = out_dir / "TOC.md"
    meta_path = out_dir / "meta.json"
    bib_path = out_dir / "cite.bib"

    if not force and paper_path.exists() and toc_path.exists() and meta_path.exists():
        # If user specifically asked for tex and tex/ isn't there yet, continue to fetch tex
        if source != "tex" or (out_dir / "tex").exists():
            return out_dir

    out_dir.mkdir(parents=True, exist_ok=True)
    body_md = ""
    used_source = ""

    # 1. arXiv HTML (cleanest structured full text with LaTeX alttext math)
    if source in ("auto", "html"):
        st, raw, ctype = http_get(meta["html_url"], timeout=35)
        if st == 200 and raw and "html" in ctype.lower():
            md = convert_html_to_markdown(raw)
            if len(md) > 800:
                body_md = md
                used_source = "arxiv-html"

    # 2. arXiv LaTeX e-print source (fallback or explicit --source tex)
    if (source == "tex") or (source == "auto" and not body_md):
        st, raw, ctype = http_get(meta["eprint_url"], timeout=45)
        if st == 200 and raw and "pdf" not in ctype.lower():
            tex_dir = out_dir / "tex"
            if tex_dir.exists():
                shutil.rmtree(tex_dir)
            tex_dir.mkdir(parents=True, exist_ok=True)
            extracted = False
            try:
                safe_extract_tar(raw, tex_dir)
                extracted = True
            except tarfile.TarError:
                try:
                    plain = gzip.decompress(raw)
                    (tex_dir / "main.tex").write_bytes(plain)
                    extracted = True
                except OSError:
                    if b"\\documentclass" in raw or b"\\section" in raw:
                        (tex_dir / "main.tex").write_bytes(raw)
                        extracted = True
            if extracted:
                tex_md = tex_to_markdown(tex_dir)
                if len(tex_md) > 400 and (source == "tex" or not body_md):
                    body_md = tex_md
                    used_source = "arxiv-tex"

    # 3. PDF fallback
    if (source == "pdf") or (not body_md):
        st, raw, _ = http_get(meta["pdf_url"], timeout=45)
        if st == 200 and raw:
            pdf_path = out_dir / "paper.pdf"
            pdf_path.write_bytes(raw)
            pdftotext = shutil.which("pdftotext")
            if pdftotext:
                proc = subprocess.run(
                    [pdftotext, str(pdf_path), "-"],
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                if proc.returncode == 0 and len(proc.stdout.strip()) > 300:
                    body_md = proc.stdout.strip() + "\n"
                    used_source = "arxiv-pdf-pdftotext"
            if not body_md:
                body_md = (
                    "_Full HTML/TeX source was unavailable and `pdftotext` is not installed._\n\n"
                    f"PDF downloaded to `{pdf_path}`.\n"
                    f"You can also inspect it cleanly via `WebFetch(url=\"{meta['pdf_url']}\")`.\n"
                )
                used_source = "arxiv-pdf-saved"

    if not body_md:
        body_md = "_Could not retrieve full paper body; abstract is included above._\n"
        used_source = "abstract-only"

    full_md = format_header(meta, used_source) + body_md
    toc_md = build_toc(full_md)

    meta["extracted_source"] = used_source
    meta["paper_md"] = str(paper_path)
    meta["toc_md"] = str(toc_path)
    meta["lines"] = len(full_md.splitlines())
    meta["words"] = len(full_md.split())

    paper_path.write_text(full_md, encoding="utf-8")
    toc_path.write_text(toc_md, encoding="utf-8")
    meta_path.write_text(json.dumps(meta, indent=2) + "\n", encoding="utf-8")
    bib_path.write_text(make_bibtex(meta), encoding="utf-8")
    return out_dir


def cmd_fetch(args: argparse.Namespace) -> int:
    out_dir = fetch_paper(args.id, source=args.source, force=args.force)
    meta = json.loads((out_dir / "meta.json").read_text(encoding="utf-8"))
    toc = (out_dir / "TOC.md").read_text(encoding="utf-8")
    print(f"Title: {meta['title']}")
    print(f"arXiv: {meta['id']} ({meta['published'][:10]})")
    print(f"Authors: {', '.join(meta['authors'])}")
    print(f"Source: {meta.get('extracted_source', 'cached')}")
    print(f"Size: {meta.get('lines', '?')} lines, {meta.get('words', '?')} words")
    print(f"Paper: {out_dir / 'paper.md'}")
    print(f"TOC:   {out_dir / 'TOC.md'}")
    if (out_dir / "tex").exists():
        print(f"TeX:   {out_dir / 'tex'}")
    print(f"BibTeX:{out_dir / 'cite.bib'}")
    print()
    print(toc.rstrip())
    return 0


def cmd_toc(args: argparse.Namespace) -> int:
    out_dir = fetch_paper(args.id, source="auto", force=False)
    print(f"Paper: {out_dir / 'paper.md'}")
    print((out_dir / "TOC.md").read_text(encoding="utf-8").rstrip())
    return 0


def cmd_search(args: argparse.Namespace) -> int:
    q = args.query.strip()
    parts: list[str] = []
    if q:
        if any(tok in q for tok in ("ti:", "au:", "abs:", "cat:", "all:")):
            parts.append(q)
        else:
            parts.append(f"all:{q}")
    if args.category:
        parts.append(f"cat:{args.category.strip()}")
    if args.author:
        parts.append(f"au:{args.author.strip()}")
    search_query = " AND ".join(parts)

    params = urllib.parse.urlencode(
        {
            "search_query": search_query,
            "start": 0,
            "max_results": max(1, min(args.max, 25)),
            "sortBy": args.sort,
            "sortOrder": "descending",
        }
    )
    url = f"https://export.arxiv.org/api/query?{params}"
    status, data, _ = http_get(url, timeout=25)
    if status != 200 or not data:
        print(f"arXiv search failed (HTTP {status})", file=sys.stderr)
        return 1

    root = ET.fromstring(data)
    entries = root.findall("atom:entry", ATOM_NS)
    if not entries:
        print("No arXiv papers found.")
        return 0

    for i, entry in enumerate(entries, start=1):
        raw_id = clean_ws(entry.findtext("atom:id", "", ATOM_NS))
        try:
            cid, _ = parse_arxiv_id(raw_id)
        except ValueError:
            cid = raw_id
        title = clean_ws(entry.findtext("atom:title", "", ATOM_NS))
        pub = clean_ws(entry.findtext("atom:published", "", ATOM_NS))[:10]
        authors = [clean_ws(a.findtext("atom:name", "", ATOM_NS)) for a in entry.findall("atom:author", ATOM_NS)]
        author_str = ", ".join(authors[:5]) + (" et al." if len(authors) > 5 else "")
        cats = [c.attrib.get("term", "") for c in entry.findall("atom:category", ATOM_NS) if c.attrib.get("term")]
        abstract = clean_ws(entry.findtext("atom:summary", "", ATOM_NS))
        if len(abstract) > 360:
            abstract = abstract[:357] + "..."
        print(f"{i}. [{cid}] {title}")
        print(f"   Date: {pub} | Authors: {author_str} | Categories: {', '.join(cats[:4])}")
        print(f"   URL: https://arxiv.org/abs/{cid}")
        print(f"   Abstract: {abstract}\n")
    return 0


def cmd_list(_args: argparse.Namespace) -> int:
    root = papers_root()
    if not root.exists():
        print(f"No cached papers yet in {root}")
        return 0
    found = 0
    for meta_file in sorted(root.glob("*/meta.json")):
        try:
            m = json.loads(meta_file.read_text(encoding="utf-8"))
        except Exception:
            continue
        found += 1
        print(f"- {m.get('id', meta_file.parent.name)} ({m.get('published', '')[:10]}) — {m.get('title', '')}")
        print(f"  {meta_file.parent / 'paper.md'} ({m.get('lines', '?')} lines, {m.get('words', '?')} words)")
    if found == 0:
        print(f"No cached papers yet in {root}")
    return 0


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser(description="OpenAgents arXiv paper search & full-text Markdown cache")
    sub = p.add_subparsers(dest="cmd", required=True)

    pf = sub.add_parser("fetch", help="Fetch full paper to ~/.openagents/papers/<id>/paper.md + TOC.md")
    pf.add_argument("id", help="arXiv ID or URL (e.g. 2505.00675 or https://arxiv.org/abs/2505.00675)")
    pf.add_argument("--source", choices=["auto", "html", "tex", "pdf"], default="auto")
    pf.add_argument("--force", action="store_true", help="Re-download even if cached")
    pf.set_defaults(func=cmd_fetch)

    pt = sub.add_parser("toc", help="Show line-numbered TOC for a paper (fetches if missing)")
    pt.add_argument("id", help="arXiv ID or URL")
    pt.set_defaults(func=cmd_toc)

    ps = sub.add_parser("search", help="Search arXiv Atom API by keyword/category/author")
    ps.add_argument("query", help="Search query (supports arXiv prefixes like ti:, au:, abs:, cat:)")
    ps.add_argument("--category", default="", help="Optional arXiv category filter (e.g. cs.AI, cs.CL, cs.LG)")
    ps.add_argument("--author", default="", help="Optional author filter")
    ps.add_argument("--max", type=int, default=8, help="Max results (1-25, default 8)")
    ps.add_argument("--sort", choices=["relevance", "submittedDate", "lastUpdatedDate"], default="relevance")
    ps.set_defaults(func=cmd_search)

    pl = sub.add_parser("list", help="List locally cached arXiv papers")
    pl.set_defaults(func=cmd_list)

    args = p.parse_args(argv)
    try:
        return args.func(args)
    except Exception as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
