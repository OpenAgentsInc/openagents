#!/usr/bin/env python3
"""Check Android release artifacts without signing or uploading them."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import struct
import subprocess
import sys
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path


PACKAGE_NAME = "com.openagents.app"
RETIRED_VERSION_CODE = 16
ANDROID_NS = "http://schemas.android.com/apk/res/android"
REQUIRED_ABIS = {"arm64-v8a", "x86_64"}
FORBIDDEN_PATH_PARTS = (
    "androidtest/",
    "test-fixtures/",
    "fixtures/rust-native/",
    "assets/debug/",
    "assets/test/",
    "assets/fixtures/",
    "mockito",
    "junit/",
    "espresso/",
)


class CheckError(Exception):
    pass


def run_tool(tool: str, *args: str) -> str:
    executable = shutil.which(tool)
    if executable is None:
        raise CheckError(f"required Android tool not found: {tool}")
    result = subprocess.run(
        [executable, *args], capture_output=True, text=True, check=False
    )
    if result.returncode != 0:
        detail = result.stderr.strip() or result.stdout.strip()
        raise CheckError(f"{tool} exited {result.returncode}: {detail}")
    return result.stdout


def fingerprint(value: str) -> str:
    normalized = value.strip().replace(":", "").lower()
    if re.fullmatch(r"[0-9a-f]{64}", normalized) is None:
        raise CheckError("certificate fingerprint must contain 32 SHA-256 bytes")
    return normalized


def artifact_metadata(path: Path) -> tuple[str, int]:
    if path.suffix.lower() == ".apk":
        output = run_tool("aapt", "dump", "badging", str(path))
        match = re.search(
            r"^package: name='([^']+)' versionCode='([0-9]+)'", output, re.MULTILINE
        )
        if match is None:
            raise CheckError(f"could not read package metadata from {path.name}")
        return match.group(1), int(match.group(2))
    if path.suffix.lower() == ".aab":
        output = run_tool(
            "bundletool",
            "dump",
            "manifest",
            f"--bundle={path}",
            "--module=base",
        )
        try:
            manifest = ET.fromstring(output)
        except ET.ParseError as error:
            raise CheckError(f"bundletool returned invalid manifest XML: {error}") from error
        version = manifest.get(f"{{{ANDROID_NS}}}versionCode")
        if manifest.tag != "manifest" or version is None:
            raise CheckError(f"could not read package metadata from {path.name}")
        return manifest.get("package", ""), int(version)
    raise CheckError(f"unsupported artifact extension: {path.suffix}")


def artifact_signer(path: Path) -> str:
    if path.suffix.lower() == ".apk":
        output = run_tool("apksigner", "verify", "--print-certs", str(path))
        match = re.search(
            r"certificate SHA-256 digest:\s*([0-9a-fA-F:]+)", output, re.IGNORECASE
        )
    else:
        output = run_tool("keytool", "-printcert", "-jarfile", str(path))
        match = re.search(r"^\s*SHA256:\s*([0-9a-fA-F:]+)", output, re.MULTILINE)
    if match is None:
        raise CheckError(f"could not read a SHA-256 signer fingerprint from {path.name}")
    return fingerprint(match.group(1))


def elf_load_alignment(data: bytes, entry: str, abi: str) -> list[int]:
    if len(data) < 64 or data[:4] != b"\x7fELF":
        raise CheckError(f"{entry} is not a complete ELF file")
    elf_class, encoding = data[4], data[5]
    if encoding not in (1, 2) or elf_class not in (1, 2):
        raise CheckError(f"{entry} has an unsupported ELF class or byte order")
    order = "<" if encoding == 1 else ">"
    machine = struct.unpack_from(order + "H", data, 18)[0]
    expected_machine = {"arm64-v8a": 183, "x86_64": 62}.get(abi)
    if expected_machine is not None and machine != expected_machine:
        raise CheckError(f"{entry} ELF machine {machine} does not match ABI {abi}")
    if elf_class == 2:
        phoff = struct.unpack_from(order + "Q", data, 32)[0]
        phentsize, phnum = struct.unpack_from(order + "HH", data, 54)
        shoff = struct.unpack_from(order + "Q", data, 40)[0]
        shentsize, shnum, shstrndx = struct.unpack_from(order + "HHH", data, 58)
        header_format = order + "IIQQQQQQ"
        expected_size = struct.calcsize(header_format)
        align_offset = 48
        section_format = order + "IIQQQQIIQQ"
    else:
        phoff = struct.unpack_from(order + "I", data, 28)[0]
        phentsize, phnum = struct.unpack_from(order + "HH", data, 42)
        shoff = struct.unpack_from(order + "I", data, 32)[0]
        shentsize, shnum, shstrndx = struct.unpack_from(order + "HHH", data, 46)
        header_format = order + "IIIIIIII"
        expected_size = struct.calcsize(header_format)
        align_offset = 28
        section_format = order + "IIIIIIIIII"
    if phentsize < expected_size or phoff + phentsize * phnum > len(data):
        raise CheckError(f"{entry} has an invalid program-header table")
    alignments = []
    for index in range(phnum):
        offset = phoff + index * phentsize
        header = struct.unpack_from(header_format, data, offset)
        if header[0] == 1:
            alignment = struct.unpack_from(order + ("Q" if elf_class == 2 else "I"), data, offset + align_offset)[0]
            alignments.append(alignment)
    if not alignments:
        raise CheckError(f"{entry} has no loadable ELF segments")
    if min(alignments) < 16 * 1024:
        raise CheckError(f"{entry} has a PT_LOAD segment aligned below 16 KiB: {alignments}")
    section_size = struct.calcsize(section_format)
    if shnum and (shentsize < section_size or shstrndx >= shnum):
        raise CheckError(f"{entry} has an invalid section-header table")
    if shnum and shoff + shentsize * shnum > len(data):
        raise CheckError(f"{entry} section-header table exceeds the file")
    if shnum:
        sections = [
            struct.unpack_from(section_format, data, shoff + index * shentsize)
            for index in range(shnum)
        ]
        names_header = sections[shstrndx]
        names_start, names_size = names_header[4], names_header[5]
        if names_start + names_size > len(data):
            raise CheckError(f"{entry} section-name table exceeds the file")
        names = data[names_start : names_start + names_size]
        unwanted = []
        for section in sections:
            start = section[0]
            end = names.find(b"\0", start)
            if start < len(names) and end >= 0:
                name = names[start:end].decode("ascii", errors="replace")
                if name.startswith(".debug_") or name == ".symtab":
                    unwanted.append(name)
        if unwanted:
            raise CheckError(f"{entry} retains debug/symbol sections: {unwanted}")
    return alignments


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def check_archive(path: Path) -> str:
    with zipfile.ZipFile(path) as archive:
        entries = archive.namelist()
        normalized = [entry.lower() for entry in entries]
        bad = [
            entry
            for entry in entries
            if any(part in entry.lower() for part in FORBIDDEN_PATH_PARTS)
        ]
        if bad:
            raise CheckError(f"{path.name} contains test/debug paths: {bad[:8]}")
        dex_files = [entry for entry in entries if entry.endswith(".dex")]
        if not dex_files:
            raise CheckError(f"{path.name} contains no DEX bytecode")
        test_markers = (b"org/junit/", b"androidx/test/", b"org/mockito/")
        for entry in dex_files:
            dex = archive.read(entry)
            if any(marker in dex for marker in test_markers):
                raise CheckError(f"{path.name} contains test classes in {entry}")
        lib_prefix = "base/lib/" if path.suffix.lower() == ".aab" else "lib/"
        native = [
            entry
            for entry in entries
            if entry.startswith(lib_prefix) and entry.endswith(".so")
        ]
        abis = {
            entry[len(lib_prefix) :].split("/", 1)[0]
            for entry in native
            if "/" in entry[len(lib_prefix) :]
        }
        missing = REQUIRED_ABIS - abis
        if missing:
            raise CheckError(f"{path.name} is missing native ABIs: {sorted(missing)}")
        if not native:
            raise CheckError(f"{path.name} contains no native libraries")
        alignments = []
        for entry in native:
            abi = entry[len(lib_prefix) :].split("/", 1)[0]
            alignments.extend(elf_load_alignment(archive.read(entry), entry, abi))
        # Android manifests are binary XML, so check the archive entry names only.
        if not any(name.endswith("androidmanifest.xml") for name in normalized):
            raise CheckError(f"{path.name} has no Android manifest entry")
        return f"{len(native)} native libraries; PT_LOAD alignment >= 16 KiB"


def check_assetlinks(path: Path, expected_fingerprint: str) -> None:
    try:
        statements = json.loads(path.read_text())
    except (OSError, json.JSONDecodeError) as error:
        raise CheckError(f"could not read asset links JSON: {error}") from error
    expected = fingerprint(expected_fingerprint)
    for statement in statements:
        target = statement.get("target", {})
        if (
            target.get("namespace") == "android_app"
            and target.get("package_name") == PACKAGE_NAME
            and "delegate_permission/common.handle_all_urls" in statement.get("relation", [])
        ):
            values = {
                fingerprint(value)
                for value in target.get("sha256_cert_fingerprints", [])
            }
            if expected in values:
                return
    raise CheckError("asset links do not authorize the supplied Play app-signing fingerprint")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apk", type=Path, required=True)
    parser.add_argument("--aab", type=Path, required=True)
    parser.add_argument("--mapping", type=Path, required=True)
    parser.add_argument("--r8-rules", type=Path, required=True)
    parser.add_argument("--assetlinks", type=Path, required=True)
    parser.add_argument("--app-signing-cert-sha256", required=True)
    parser.add_argument("--apk-cert-sha256", required=True)
    parser.add_argument("--aab-cert-sha256", required=True)
    args = parser.parse_args()

    try:
        fingerprint(args.app_signing_cert_sha256)
        expected_signers = {
            args.apk: fingerprint(args.apk_cert_sha256),
            args.aab: fingerprint(args.aab_cert_sha256),
        }
        check_assetlinks(args.assetlinks, args.app_signing_cert_sha256)
        for path in (args.apk, args.aab):
            if not path.is_file():
                raise CheckError(f"artifact does not exist: {path}")
            package, version_code = artifact_metadata(path)
            if package != PACKAGE_NAME:
                raise CheckError(f"{path.name} has package {package}, expected {PACKAGE_NAME}")
            if version_code <= RETIRED_VERSION_CODE:
                raise CheckError(
                    f"{path.name} versionCode {version_code} must exceed {RETIRED_VERSION_CODE}"
                )
            signer = artifact_signer(path)
            if signer != expected_signers[path]:
                raise CheckError(
                    f"{path.name} signer {signer} does not match its approved certificate fingerprint"
                )
            contents = check_archive(path)
            digest = sha256_file(path)
            print(
                f"{path.name}: package={package} versionCode={version_code} "
                f"artifactSignerSha256={signer} sha256={digest}; {contents}"
            )
            if path.suffix.lower() == ".apk":
                run_tool("zipalign", "-c", "-P", "16", "-v", "4", str(path))
                print(f"{path.name}: zipalign 16 KiB check passed")
        for path, label in ((args.r8_rules, "R8 rules"), (args.mapping, "R8 mapping")):
            if not path.is_file() or path.stat().st_size == 0:
                raise CheckError(f"{label} is missing or empty: {path}")
        print("R8 rules and nonempty mapping file are present")
        print("asset links authorize the supplied Play app-signing certificate")
    except CheckError as error:
        print(f"release check failed: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
