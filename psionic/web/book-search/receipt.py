"""Psionic chapter 59's receipt (tooling only; never shipped).

Reads what check.sh recorded: the native result lines (bundle/reference.txt)
and the page's report from headless Chrome for the real bundle and the three
refused ones. Compares the browser's result lines with the native ones byte
for byte and writes the receipt. Exits 1 unless the lines are equal and each
refused bundle got its own verdict.
"""
import hashlib
import json
import sys
from pathlib import Path

# BbVerdict in psionic/lib/book-search/bundle.coax.
EXPECTED = {"bundle": 0, "wrong-encoder": 3, "other-encoder": 6, "stale": 7}


def main() -> int:
    out, receipt = Path(sys.argv[1]), Path(sys.argv[2])
    reference = (out / "bundle" / "reference.txt").read_bytes()
    manifest = (out / "bundle" / "manifest.txt").read_text()
    pages = {name: json.loads((out / f"{name}.json").read_text()) for name in EXPECTED}
    main_page = pages["bundle"]
    browser = main_page.get("lines", "").encode()
    (out / "browser.txt").write_bytes(browser)
    equal = browser == reference and len(reference) > 0
    first_difference = next((i for i, (a, b) in enumerate(zip(browser.splitlines(), reference.splitlines())) if a != b), None)
    refusals = {
        name: {"verdict": pages[name].get("verdict"), "message": pages[name].get("message"), "answered": "lines" in pages[name]}
        for name in EXPECTED if name != "bundle"
    }
    verdicts_ok = all(pages[name].get("done") and pages[name].get("verdict") == want for name, want in EXPECTED.items())
    answered_refused = any(r["answered"] for r in refusals.values())
    fields = dict(line.split(" ", 1) for line in manifest.strip().splitlines()[1:])
    data = {
        "chapter": "psionic/book/ch059-run-in-the-browser.md",
        "what": "chapter 54's search (bs-attnpool-d64-v1 query encoder, BM25, exhaustive vector search, fusion) compiled to Wasm and run in headless Chrome on the 36 held-out queries, compared with the native CPU tool",
        "browser": main_page.get("userAgent"),
        "served_by": "coax/web/serve.sh --static on 127.0.0.1 (no compression)",
        "manifest": {"encoder": fields["encoder"], "index": fields["index"], "corpus": fields["corpus"]},
        "queries": len(reference.splitlines()),
        "tolerance": "none: every query embedding (64 binary32 values) and the first ten passages and binary64 scores of each ranking must be identical",
        "equal": equal,
        "first_difference": first_difference,
        "reference_sha256": hashlib.sha256(reference).hexdigest(),
        "browser_sha256": hashlib.sha256(browser).hexdigest(),
        "download_bytes": main_page.get("bytes"),
        "load_ms": {k: round(v, 1) for k, v in main_page.get("ms", {}).items()},
        "memory": main_page.get("memory"),
        "counts": main_page.get("counts"),
        "refusals": refusals,
        "error": main_page.get("error"),
    }
    receipt.parent.mkdir(parents=True, exist_ok=True)
    receipt.write_text(json.dumps(data, indent=2) + "\n")
    print(f"browser lines equal to native: {equal}; refusals as expected: {verdicts_ok and not answered_refused}; receipt: {receipt}")
    return 0 if equal and verdicts_ok and not answered_refused else 1


if __name__ == "__main__":
    sys.exit(main())
