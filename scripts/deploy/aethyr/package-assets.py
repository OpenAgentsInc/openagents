#!/usr/bin/env python3
"""Freeze only a scene's compiled browser dependency closure."""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
from urllib.parse import unquote

ASSET_SUFFIXES = {'.json', '.bin', '.png', '.jpg', '.webp', '.jpeg'}
URL_PREFIXES = ('/private-assets/', '/starter/')


@dataclass(frozen=True)
class AssetFile:
    """One admitted source file or generated document in a compiled closure."""
    name: str
    path: Path | None = None
    document: bytes | None = None

    def read_bytes(self) -> bytes:
        return self.document if self.document is not None else self.path.read_bytes()


def asset_name(name: str) -> str:
    if (not isinstance(name, str) or not name or name != unquote(name)
            or '?' in name or '#' in name or '\\' in name
            or name.startswith('/') or any(part.startswith('.') for part in name.split('/'))
            or any(not part for part in name.split('/'))
            or Path(name).suffix.lower() not in ASSET_SUFFIXES):
        raise ValueError('Use a supported asset file inside its declared root')
    return name


def source_file(source: Path, name: str) -> Path:
    name = asset_name(name)
    root = source.resolve(strict=True)
    path = root
    for part in name.split('/'):
        path = path / part
        if path.is_symlink():
            raise ValueError('Asset dependencies cannot follow symbolic links')
    path = path.resolve(strict=True)
    if not path.is_relative_to(root) or not path.is_file():
        raise ValueError('Asset dependency is outside its declared root')
    return path


def references(value, url_prefix='/private-assets/'):
    """Find runtime asset URLs without treating source provenance as a dependency."""
    if url_prefix not in URL_PREFIXES:
        raise ValueError('Choose the private-assets or starter URL root')
    if isinstance(value, str):
        if value.startswith(url_prefix):
            yield asset_name(value.removeprefix(url_prefix))
        elif any(value.startswith(prefix) for prefix in URL_PREFIXES):
            raise ValueError('Compiled dependencies cannot cross asset roots')
    elif isinstance(value, list):
        for item in value:
            yield from references(item, url_prefix)
    elif isinstance(value, dict):
        for item in value.values():
            yield from references(item, url_prefix)


def dependency_closure(source: Path, roots: list[str], url_prefix='/private-assets/',
                       generated: dict[str, bytes] | None = None) -> dict[str, AssetFile]:
    """Resolve the same recursive compiled closure for deployment and cell cooking."""
    source = source.resolve(strict=True)
    generated = generated or {}
    pending = list(roots)
    found: dict[str, AssetFile] = {}
    while pending:
        name = asset_name(pending.pop())
        if name in found:
            continue
        item = AssetFile(name, document=generated[name]) if name in generated else AssetFile(name, path=source_file(source, name))
        if Path(name).suffix.lower() == '.json':
            try:
                data = item.read_bytes()
                value = json.loads(data, parse_constant=lambda _: (_ for _ in ()).throw(ValueError('Asset JSON needs finite numbers')))
                item = AssetFile(name, path=item.path, document=data)
            except (UnicodeDecodeError, json.JSONDecodeError) as error:
                raise ValueError(f'Asset JSON could not open: {name}') from error
            pending.extend(references(value, url_prefix))
        found[name] = item
    return dict(sorted(found.items()))


def package_assets(source: Path, out: Path, profile='medieval-town'):
    profiles = {
        'medieval-town': ['scene.json', 'scene-night.json'],
        'starter': ['scene.json', 'scene-night.json', 'colliders.json', 'game.json'],
        'haunted-house': ['haunt-profile.json'],
        'personal-planet': ['planet-profile.json'],
        'universe': [],
    }
    if profile not in profiles:
        raise ValueError('Choose a supported browser asset profile')
    prefix = '/starter/' if profile == 'starter' else '/private-assets/'
    files = dependency_closure(source, profiles[profile], prefix)
    out.mkdir(parents=True, exist_ok=True)
    index = []
    for name, item in files.items():
        data = item.read_bytes()
        destination = out / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        index.append({'path': name, 'sha256': hashlib.sha256(data).hexdigest(), 'bytes': len(data)})
    body = json.dumps(index, sort_keys=True, separators=(',', ':')).encode()
    identity = hashlib.sha256(body).hexdigest()
    (out.parent / 'assets-manifest.json').write_text(json.dumps({
        'schema': 'aethyrforge.browser-assets.v1', 'sha256': identity, 'files': index}, indent=2) + '\n')
    return {'sha256': identity, 'files': len(index), 'bytes': sum(item['bytes'] for item in index)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('out', type=Path)
    parser.add_argument('profile', nargs='?', default='medieval-town')
    args = parser.parse_args()
    print(json.dumps(package_assets(args.source, args.out, args.profile)))


if __name__ == '__main__':
    main()
