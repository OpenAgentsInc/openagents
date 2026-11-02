#!/usr/bin/env python3
"""Hash-bound test-only projection of complete authored Coax declarations.

No type/field/constructor is fabricated. This is a bounded native-test input,
not qualification of the full desktop unit whose unrelated analysis refuses.
"""
from pathlib import Path
import hashlib
import json
import re


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def mask(text):
    # Preserve offsets/newlines while removing comments and quoted contents.
    pattern = r'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\''
    return re.sub(pattern, lambda m: ''.join('\n' if c == '\n' else ' ' for c in m[0]), text)


def declarations(root, entry='apps/openagents-desktop/zeron/settings.coax'):
    root = Path(root).resolve()
    files, result = {}, {}
    def visit(path):
        path = path.resolve()
        if path in files:
            return
        raw = path.read_bytes(); text = raw.decode(); files[path] = dict(path=str(path.relative_to(root)), sha256=sha(raw), bytes=len(raw))
        for m in re.finditer(r'^\s*#load\s+"([^"]+)"', text, re.M):
            visit(path.parent / m[1])
        clean = mask(text)
        candidates = list(re.finditer(r'\b([A-Za-z_]\w*)\s*::|#(?:load|import|import_proto|test|forall|authority)\b', clean))
        depth = 0; cursor = 0; top = []
        for m in candidates:
            for c in clean[cursor:m.start()]:
                if c in '({[': depth += 1
                elif c in ')}]': depth -= 1
            cursor = m.start()
            if depth == 0:
                top.append(m)
        for i, m in enumerate(top):
            name = m.group(1)
            if not name:
                continue
            end = top[i + 1].start() if i + 1 < len(top) else len(text)
            body = text[m.start():end].rstrip() + '\n'
            tail = clean[m.end():end].lstrip()
            kind = 'type' if re.match(r'(struct|enum|union)\b', tail) else 'procedure' if tail.startswith('(') else 'constant'
            row = dict(name=name, kind=kind, path=str(path.relative_to(root)), line=text.count('\n', 0, m.start())+1,
                       start=m.start(), end=end, sha256=sha(body.encode()), text=body)
            if name in result and result[name]['text'] != body:
                raise ValueError('ambiguous source declaration: '+name)
            result[name] = row
    visit(root / entry)
    return result, list(files.values())


def emit_subset(root, out, roots, entry='apps/openagents-desktop/zeron/settings.coax'):
    """Write subset.coax + extraction.json, returning the manifest dictionary.

    roots are actual declaration names (complete types or procedures). Referenced
    authored types/constants and directly called procedures are closed recursively.
    """
    out = Path(out); out.mkdir(parents=True, exist_ok=True)
    index, files = declarations(root, entry)
    selected = {}
    def add(name):
        if name in selected:
            return
        if name not in index:
            raise ValueError('missing authored declaration: '+name)
        row = index[name]; selected[name] = row
        clean = mask(row['text'])
        for token in set(re.findall(r'\b[A-Za-z_]\w*\b', clean)):
            if token in index and index[token]['kind'] != 'procedure':
                add(token)
        for call in re.findall(r'\b([A-Za-z_]\w*)\s*\(', clean):
            if call in index:
                add(call)
    for name in roots:
        add(name)
    text = '#import "Basic";\n\n' + '\n'.join(row['text'] for row in selected.values())
    (out / 'subset.coax').write_text(text)
    manifest = dict(schema_version=1, scope='exact authored declaration projection; full desktop emission is not established',
                    entry=entry, roots=roots, files=files,
                    declarations=[{k:v for k,v in row.items() if k!='text'} for row in selected.values()],
                    subset_sha256=sha(text.encode()))
    (out / 'extraction.json').write_text(json.dumps(manifest, indent=2)+'\n')
    return manifest
