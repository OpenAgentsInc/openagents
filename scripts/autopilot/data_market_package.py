#!/usr/bin/env python3
"""Package local files into deterministic Data Market draft artifacts."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any

DEFAULT_ASSET_KIND = "conversation_bundle"
DEFAULT_POLICY = "private_preview_license"
DEFAULT_DELIVERY_MODES = ["encrypted_pointer", "delivery_bundle_ref"]
DEFAULT_VISIBILITY = "targeted_only"
DEFAULT_SENSITIVITY = "private"
LISTING_FILENAME = "listing-template.json"
GRANT_FILENAME = "grant-template.json"
SUMMARY_FILENAME = "packaging-summary.json"
MANIFEST_FILENAME = "packaging-manifest.json"
SCHEMA_VERSION = 1


@dataclass(frozen=True)
class PackagedFile:
    package_path: str
    source_path: str
    source_kind: str
    size_bytes: int
    file_digest: str

    def manifest_row(self) -> dict[str, Any]:
        return {
            "package_path": self.package_path,
            "source_kind": self.source_kind,
            "size_bytes": self.size_bytes,
            "file_digest": self.file_digest,
        }

    def summary_row(self) -> dict[str, Any]:
        return {
            "package_path": self.package_path,
            "source_path": self.source_path,
            "source_kind": self.source_kind,
            "size_bytes": self.size_bytes,
            "file_digest": self.file_digest,
        }


def sha256_prefixed_bytes(payload: bytes) -> str:
    return f"sha256:{hashlib.sha256(payload).hexdigest()}"


def canonical_slug(value: str) -> str:
    slug = re.sub(r"[^a-z0-9]+", "-", value.strip().lower()).strip("-")
    return slug or "data-package"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Build deterministic Data Market listing/grant draft artifacts from local files."
    )
    parser.add_argument(
        "--source",
        action="append",
        required=True,
        help="Local file or directory to include in the package boundary. Repeat for multiple roots.",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="Directory where listing-template.json, grant-template.json, and summary files will be written.",
    )
    parser.add_argument("--title", required=True, help="Asset title.")
    parser.add_argument("--description", help="Asset description.")
    parser.add_argument(
        "--asset-kind",
        default=DEFAULT_ASSET_KIND,
        help=f"Data asset kind. Default: {DEFAULT_ASSET_KIND}",
    )
    parser.add_argument(
        "--package-label",
        help="Optional explicit package label. Defaults to a slug derived from --title.",
    )
    parser.add_argument("--price-sats", type=int, help="Default listing price hint in sats.")
    parser.add_argument(
        "--default-policy",
        default=DEFAULT_POLICY,
        help=f"Listing default policy. Default: {DEFAULT_POLICY}",
    )
    parser.add_argument(
        "--delivery-mode",
        action="append",
        dest="delivery_modes",
        help="Allowed delivery mode. Repeat to add more than one mode.",
    )
    parser.add_argument(
        "--visibility-posture",
        default=DEFAULT_VISIBILITY,
        help=f"Visibility posture. Default: {DEFAULT_VISIBILITY}",
    )
    parser.add_argument(
        "--sensitivity-posture",
        default=DEFAULT_SENSITIVITY,
        help=f"Sensitivity posture. Default: {DEFAULT_SENSITIVITY}",
    )
    parser.add_argument(
        "--consumer-id",
        help="Optional starter grant consumer id to prefill grant-template.json.",
    )
    parser.add_argument(
        "--grant-price-sats",
        type=int,
        help="Optional starter grant price hint in sats. Defaults to --price-sats when unset.",
    )
    parser.add_argument(
        "--grant-policy-template",
        help="Optional starter grant policy template. Defaults to --default-policy when unset.",
    )
    parser.add_argument(
        "--grant-expires-hours",
        type=int,
        help="Optional starter grant expiry window in hours.",
    )
    parser.add_argument(
        "--grant-warranty-window-hours",
        type=int,
        help="Optional starter grant warranty window in hours.",
    )
    parser.add_argument(
        "--skip-grant-template",
        action="store_true",
        help="Do not emit grant-template.json.",
    )
    return parser.parse_args()


def normalize_sources(raw_sources: list[str]) -> list[Path]:
    normalized: list[Path] = []
    for raw in raw_sources:
        path = Path(raw).expanduser()
        if not path.exists():
            raise ValueError(f"source does not exist: {raw}")
        if path.is_symlink():
            raise ValueError(f"symlink package roots are not supported: {path}")
        resolved = path.resolve()
        normalized.append(resolved)
    return normalized


def walk_files(source: Path) -> list[PackagedFile]:
    if source.is_symlink():
        raise ValueError(f"symlink package roots are not supported: {source}")
    root_name = source.name or "root"
    files: list[PackagedFile] = []
    if source.is_file():
        files.append(
            PackagedFile(
                package_path=root_name,
                source_path=str(source),
                source_kind="file",
                size_bytes=source.stat().st_size,
                file_digest=sha256_prefixed_bytes(source.read_bytes()),
            )
        )
        return files
    if not source.is_dir():
        raise ValueError(f"unsupported source boundary: {source}")
    for path in sorted(source.rglob("*")):
        if path.is_symlink():
            raise ValueError(f"symlink members are not supported: {path}")
        if not path.is_file():
            continue
        relative_path = path.relative_to(source).as_posix()
        files.append(
            PackagedFile(
                package_path=f"{root_name}/{relative_path}",
                source_path=str(path),
                source_kind="directory_member",
                size_bytes=path.stat().st_size,
                file_digest=sha256_prefixed_bytes(path.read_bytes()),
            )
        )
    return files


def collect_package_files(sources: list[Path]) -> list[PackagedFile]:
    packaged: list[PackagedFile] = []
    seen_paths: dict[str, str] = {}
    for source in sorted(sources):
        for row in walk_files(source):
            previous = seen_paths.get(row.package_path)
            if previous is not None:
                raise ValueError(
                    "package path collision for "
                    f"{row.package_path}: {previous} vs {row.source_path}"
                )
            seen_paths[row.package_path] = row.source_path
            packaged.append(row)
    packaged.sort(key=lambda row: row.package_path)
    return packaged


def build_manifest(package_label: str, files: list[PackagedFile]) -> dict[str, Any]:
    return {
        "schema_version": SCHEMA_VERSION,
        "package_label": package_label,
        "files": [row.manifest_row() for row in files],
    }


def write_json(path: Path, payload: dict[str, Any]) -> None:
    path.write_text(
        json.dumps(payload, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def build_listing_template(
    args: argparse.Namespace,
    package_label: str,
    content_digest: str,
    provenance_ref: str,
    manifest_digest: str,
    files: list[PackagedFile],
) -> dict[str, Any]:
    delivery_modes = args.delivery_modes or list(DEFAULT_DELIVERY_MODES)
    description = args.description or (
        f"Packaged data bundle with {len(files)} file(s) and manifest {manifest_digest}."
    )
    return {
        "asset_kind": args.asset_kind,
        "title": args.title,
        "description": description,
        "content_digest": content_digest,
        "provenance_ref": provenance_ref,
        "default_policy": args.default_policy,
        "price_hint_sats": args.price_sats,
        "delivery_modes": delivery_modes,
        "visibility_posture": args.visibility_posture,
        "sensitivity_posture": args.sensitivity_posture,
        "metadata": {
            "packaging_schema_version": str(SCHEMA_VERSION),
            "packaging_package_label": package_label,
            "packaging_manifest_digest": manifest_digest,
            "packaging_file_count": str(len(files)),
            "packaging_total_bytes": str(sum(row.size_bytes for row in files)),
            "packaging_source_roots": json.dumps(
                sorted({row.package_path.split("/", 1)[0] for row in files})
            ),
            "packaging_script": "scripts/autopilot/data_market_package.py",
        },
    }


def build_grant_template(
    args: argparse.Namespace,
    package_label: str,
    manifest_digest: str,
) -> dict[str, Any]:
    delivery_modes = args.delivery_modes or list(DEFAULT_DELIVERY_MODES)
    return {
        "default_policy": args.grant_policy_template or args.default_policy,
        "policy_template": args.grant_policy_template or args.default_policy,
        "consumer_id": args.consumer_id,
        "price_hint_sats": args.grant_price_sats or args.price_sats,
        "delivery_modes": delivery_modes,
        "visibility_posture": args.visibility_posture,
        "expires_in_hours": args.grant_expires_hours,
        "warranty_window_hours": args.grant_warranty_window_hours,
        "metadata": {
            "packaging_schema_version": str(SCHEMA_VERSION),
            "packaging_package_label": package_label,
            "packaging_manifest_digest": manifest_digest,
            "packaging_script": "scripts/autopilot/data_market_package.py",
        },
    }


def main() -> int:
    try:
        args = parse_args()
        sources = normalize_sources(args.source)
        files = collect_package_files(sources)
        if not files:
            raise ValueError("package boundary resolved to zero files")

        output_dir = Path(args.output_dir).expanduser().resolve()
        output_dir.mkdir(parents=True, exist_ok=True)

        package_label = canonical_slug(args.package_label or args.title)
        manifest = build_manifest(package_label, files)
        manifest_bytes = json.dumps(
            manifest,
            sort_keys=True,
            separators=(",", ":"),
        ).encode("utf-8")
        manifest_digest = sha256_prefixed_bytes(manifest_bytes)
        content_digest = manifest_digest
        provenance_ref = (
            f"oa://local-packages/{package_label}/"
            f"{manifest_digest.removeprefix('sha256:')}"
        )

        manifest_path = output_dir / MANIFEST_FILENAME
        listing_path = output_dir / LISTING_FILENAME
        grant_path = output_dir / GRANT_FILENAME
        summary_path = output_dir / SUMMARY_FILENAME

        listing = build_listing_template(
            args,
            package_label,
            content_digest,
            provenance_ref,
            manifest_digest,
            files,
        )
        write_json(manifest_path, manifest)
        write_json(listing_path, listing)

        grant_written = False
        if not args.skip_grant_template:
            grant = build_grant_template(args, package_label, manifest_digest)
            write_json(grant_path, grant)
            grant_written = True

        summary = {
            "schema_version": SCHEMA_VERSION,
            "package_label": package_label,
            "title": args.title,
            "asset_kind": args.asset_kind,
            "content_digest": content_digest,
            "provenance_ref": provenance_ref,
            "source_paths": [str(path) for path in sorted(sources, key=str)],
            "file_count": len(files),
            "total_bytes": sum(row.size_bytes for row in files),
            "manifest_digest": manifest_digest,
            "manifest_path": str(manifest_path),
            "listing_template_path": str(listing_path),
            "grant_template_path": str(grant_path) if grant_written else None,
            "files": [row.summary_row() for row in files],
        }
        write_json(summary_path, summary)

        print(
            json.dumps(
                summary,
                indent=2,
                sort_keys=True,
            )
        )
        return 0
    except Exception as exc:  # pylint: disable=broad-except
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
