#!/usr/bin/env python3
"""Reviewed prose/feature/evidence bindings; book tooling, never a runtime policy."""
import hashlib
import json
from pathlib import Path
import re
import sys

ROOT = Path(__file__).resolve().parents[2]


def digest(data):
    return hashlib.sha256(data).hexdigest()


def feature_digest(feature):
    return digest(json.dumps(feature, sort_keys=True, ensure_ascii=False, separators=(",", ":")).encode())


def problems(root):
    """Invalidate reviewed bindings; this does not prove prose semantics."""
    root = Path(root).resolve()
    errors = []

    def file(name):
        if not isinstance(name, str) or Path(name).is_absolute():
            raise ValueError("expected a repository-relative file")
        p = (root / name).resolve()
        if not p.is_relative_to(root) or not p.is_file():
            raise ValueError(f"missing or outside-repository file: {name}")
        return p

    def hash_ok(value):
        return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) is not None

    try:
        ledger = json.loads(file("coax/book/capability-claims.json").read_text())
        if not isinstance(ledger, dict) or type(ledger.get("schema_version")) is not int or ledger["schema_version"] != 1:
            raise ValueError("unsupported claim ledger schema")
        entries = ledger.get("claims")
        if not isinstance(entries, list) or not entries:
            raise ValueError("claim ledger must contain reviewed claims")
        catalogue = json.loads(file(ledger.get("catalogue")).read_text())
        if not isinstance(catalogue, dict) or type(catalogue.get("schema_version")) is not int or catalogue["schema_version"] != 1:
            raise ValueError("unsupported capability catalogue schema")
        features = {}
        for feature in catalogue["features"]:
            if feature["id"] in features:
                raise ValueError("duplicate catalogue feature")
            features[feature["id"]] = feature
    except (OSError, ValueError, TypeError, KeyError) as exc:
        return [f"capability claims: {exc}"]

    documents = set((root / "coax/book").glob("*.md")) | set((root / "psionic/book").glob("*.md"))
    documents.update(root / name for name in ["coax/README.md", "psionic/README.md", "psionic/docs/roadmap.md"] if (root / name).is_file())
    allowed_documents = {p.resolve() for p in documents}
    registered = {}
    for claim in entries:
        try:
            identity = claim["id"]
            if not isinstance(identity, str) or not re.fullmatch(r"[a-z][a-z0-9.-]*", identity):
                raise ValueError("invalid claim identity")
            if identity in registered:
                raise ValueError(f"duplicate claim: {identity}")
            document = file(claim["document"])
            if document not in allowed_documents:
                raise ValueError(f"{identity}: document is outside the book inventory")
            if not isinstance(claim.get("scope"), str) or not claim["scope"].strip():
                raise ValueError(f"{identity}: missing reviewed scope")
            if not hash_ok(claim.get("text_sha256")):
                raise ValueError(f"{identity}: invalid prose digest")
            selected = claim.get("features", [])
            evidence = claim.get("evidence", [])
            if not isinstance(selected, list) or not isinstance(evidence, list) or not (selected or evidence):
                raise ValueError(f"{identity}: no feature or evidence binding")
            registered[identity] = (claim, document)
            documents.add(document)
            seen_features = set()
            for binding in selected:
                name = binding["id"]
                if name in seen_features or name not in features or not hash_ok(binding.get("sha256")):
                    errors.append(f"{identity}: invalid or unknown feature: {name}")
                elif feature_digest(features[name]) != binding["sha256"]:
                    errors.append(f"{identity}: feature changed; review {name}")
                seen_features.add(name)
            seen_evidence = set()
            for binding in evidence:
                try:
                    name = binding["path"]
                    p = file(name)
                    if name in seen_evidence or not hash_ok(binding.get("sha256")):
                        raise ValueError("duplicate evidence or invalid digest")
                    if digest(p.read_bytes()) != binding["sha256"]:
                        errors.append(f"{identity}: evidence changed; review {name}")
                    seen_evidence.add(name)
                except (OSError, ValueError, TypeError, KeyError) as exc:
                    errors.append(f"{identity}: invalid evidence: {exc}")
        except (OSError, ValueError, TypeError, KeyError) as exc:
            errors.append(f"capability claims: {exc}")

    found = set()
    pattern = re.compile(r"<!-- capability-claim: ([a-z][a-z0-9.-]*) -->\n(.*?)\n<!-- /capability-claim -->", re.S)
    scanned = set()
    for name in sorted(documents):
        try:
            document = file(str(name.relative_to(root)))
            if document in scanned:
                continue
            scanned.add(document)
            text = document.read_text()
        except (OSError, ValueError) as exc:
            errors.append(f"capability claims: {exc}")
            continue
        blocks = list(pattern.finditer(text))
        if text.count("<!-- capability-claim:") != len(blocks) or text.count("<!-- /capability-claim -->") != len(blocks):
            errors.append(f"{document.relative_to(root)}: malformed capability claim marker")
        for match in blocks:
            identity, body = match.groups()
            if identity in found:
                errors.append(f"duplicate capability claim block: {identity}")
            found.add(identity)
            if identity not in registered:
                errors.append(f"unregistered capability claim: {identity}")
                continue
            claim, expected_document = registered[identity]
            if document != expected_document:
                errors.append(f"{identity}: claim is in the wrong document")
            if not body.strip() or "<!-- capability-claim:" in body or digest(body.encode()) != claim["text_sha256"]:
                errors.append(f"{identity}: text changed; review the claim and its bindings")
    for identity in registered.keys() - found:
        errors.append(f"missing capability claim block: {identity}")
    return errors


if __name__ == "__main__":
    failures = problems(ROOT)
    for failure in failures:
        print(failure, file=sys.stderr)
    if failures:
        sys.exit(1)
    print("reviewed book claims match prose, catalogue features and retained evidence")
