#!/usr/bin/env python3
"""Export and package redacted Codex rollout conversations for Data Market sale."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import subprocess
import sys
from collections import Counter
from pathlib import Path
from typing import Any
from urllib.parse import urlsplit

EXPORT_SCHEMA_VERSION = 1
PACKAGE_SCRIPT = "scripts/autopilot/data_market_package.py"
EXPORT_SCRIPT = "scripts/autopilot/package_codex_conversations.py"
EXPORT_DIRNAME = "redacted-codex-conversations"
INDEX_FILENAME = "conversation-index.json"
DEFAULT_TITLE = "Redacted Codex conversation bundle"
DEFAULT_ASSET_KIND = "conversation_bundle"
DEFAULT_LIMIT = 5
LITERAL_REPLACEMENTS: list[tuple[re.Pattern[str], str]] = []

EMAIL_RE = re.compile(r"\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b", re.IGNORECASE)
URL_RE = re.compile(r"https?://[^\s\"'`)>]+", re.IGNORECASE)
HOME_PATH_RE = re.compile(r"(?P<path>(?:/home/[^/\s]+|/Users/[^/\s]+|~)(?:/[^\s\"'`)>:;,]+)*)")
TMP_PATH_RE = re.compile(r"(?P<path>/tmp(?:/[^\s\"'`)>:;,]+)*)")
LOCALHOST_URL_HOSTS = {"127.0.0.1", "localhost"}
NSEC_RE = re.compile(r"\bnsec1[0-9a-z]{20,}\b")
NPUB_RE = re.compile(r"\bnpub1[0-9a-z]{20,}\b")
LN_INVOICE_RE = re.compile(r"\bln(?:bc|tb|bcrt)[0-9a-z]+\b", re.IGNORECASE)
SECRET_PATTERNS: list[tuple[str, re.Pattern[str]]] = [
    ("ANTHROPIC_KEY", re.compile(r"sk-ant-[a-zA-Z0-9_-]{20,}")),
    ("OPENAI_KEY", re.compile(r"sk-[a-zA-Z0-9]{20,}")),
    ("GITHUB_TOKEN", re.compile(r"gh[pousr]_[a-zA-Z0-9]{20,}")),
    ("GITHUB_PAT", re.compile(r"github_pat_[a-zA-Z0-9_]{20,}")),
    ("AWS_ACCESS_KEY", re.compile(r"AKIA[0-9A-Z]{16}")),
    ("SLACK_TOKEN", re.compile(r"xox[baprs]-[0-9]+-[0-9]+-[a-zA-Z0-9-]+")),
    ("JWT", re.compile(r"eyJ[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+\.[a-zA-Z0-9_-]+")),
    ("PRIVATE_KEY", re.compile(r"-----BEGIN [A-Z ]+ PRIVATE KEY-----")),
    ("BEARER_TOKEN", re.compile(r"(?i)\bbearer\s+[a-z0-9._-]+\b")),
]
SENSITIVE_KEY_BITS = (
    "password",
    "passwd",
    "pwd",
    "secret",
    "token",
    "apikey",
    "api_key",
    "api-key",
    "auth",
    "credential",
    "private_key",
    "private-key",
    "privatekey",
    "access_key",
    "access-key",
    "accesskey",
    "secret_key",
    "secret-key",
    "secretkey",
    "seed",
    "mnemonic",
)


def short_hash(value: str) -> str:
    return hashlib.sha256(value.encode("utf-8")).hexdigest()[:10]


def redacted_token(kind: str, raw: str) -> str:
    return f"[REDACTED_{kind}:{short_hash(raw)}]"


def configure_literal_replacements(scrubs: list[str]) -> None:
    LITERAL_REPLACEMENTS.clear()
    home_name = Path.home().name
    literals: list[str] = []
    if home_name:
        literals.append(home_name)
    literals.extend(scrubs)
    seen: set[str] = set()
    for literal in literals:
        if not literal or literal in seen:
            continue
        seen.add(literal)
        placeholder = redacted_token("LITERAL", literal)
        LITERAL_REPLACEMENTS.append((re.compile(re.escape(literal), re.IGNORECASE), placeholder))


def sanitize_url(match: re.Match[str]) -> str:
    raw = match.group(0)
    try:
        parsed = urlsplit(raw)
    except ValueError:
        return redacted_token("URL", raw)
    host = parsed.hostname or "unknown-host"
    if host in LOCALHOST_URL_HOSTS:
        host = "[LOCALHOST]"
    path_hint = "/[REDACTED_PATH]" if parsed.path not in ("", "/") else ""
    query_hint = "?[REDACTED_QUERY]" if parsed.query else ""
    fragment_hint = "#[REDACTED_FRAGMENT]" if parsed.fragment else ""
    return f"{parsed.scheme}://{host}{path_hint}{query_hint}{fragment_hint}"


def sanitize_local_path(match: re.Match[str], prefix: str) -> str:
    raw = match.group("path")
    return f"{prefix}/{redacted_token('PATH', raw)}"


def redact_text(text: str) -> str:
    redacted = text
    redacted = URL_RE.sub(sanitize_url, redacted)
    redacted = HOME_PATH_RE.sub(lambda match: sanitize_local_path(match, "/home/[USER]"), redacted)
    redacted = TMP_PATH_RE.sub(lambda match: sanitize_local_path(match, "/tmp"), redacted)
    redacted = EMAIL_RE.sub(lambda match: redacted_token("EMAIL", match.group(0)), redacted)
    redacted = NSEC_RE.sub(lambda match: redacted_token("NSEC", match.group(0)), redacted)
    redacted = NPUB_RE.sub(lambda match: redacted_token("NPUB", match.group(0)), redacted)
    redacted = LN_INVOICE_RE.sub(lambda match: redacted_token("LIGHTNING_INVOICE", match.group(0)), redacted)
    for kind, pattern in SECRET_PATTERNS:
        redacted = pattern.sub(lambda match: redacted_token(kind, match.group(0)), redacted)
    for pattern, replacement in LITERAL_REPLACEMENTS:
        redacted = pattern.sub(replacement, redacted)
    return redacted


def sanitize_value(value: Any, key: str | None = None) -> Any:
    if key and any(bit in key.lower() for bit in SENSITIVE_KEY_BITS):
        return redacted_token("FIELD", key)
    if isinstance(value, str):
        return redact_text(value)
    if isinstance(value, list):
        return [sanitize_value(item) for item in value]
    if isinstance(value, dict):
        return {sub_key: sanitize_value(sub_value, sub_key) for sub_key, sub_value in value.items()}
    return value


def content_text_parts(content: Any) -> list[str]:
    if not isinstance(content, list):
        return []
    parts: list[str] = []
    for item in content:
        if not isinstance(item, dict):
            continue
        text = item.get("text")
        if isinstance(text, str) and text.strip():
            parts.append(text)
    return parts


def build_arg_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Redact Codex rollout conversations and package them into Data Market draft artifacts."
    )
    parser.add_argument(
        "--session",
        action="append",
        default=[],
        help="Explicit Codex rollout file or directory. Repeat to include multiple paths.",
    )
    parser.add_argument(
        "--session-root",
        default="~/.codex/sessions",
        help="Codex rollout root used when --session is omitted. Default: ~/.codex/sessions",
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=DEFAULT_LIMIT,
        help=f"Number of most recent rollout files to export when using --session-root. Default: {DEFAULT_LIMIT}",
    )
    parser.add_argument(
        "--output-dir",
        required=True,
        help="Directory where redacted exports and Data Market templates will be written.",
    )
    parser.add_argument(
        "--title",
        default=DEFAULT_TITLE,
        help=f"Asset title. Default: {DEFAULT_TITLE}",
    )
    parser.add_argument("--description", help="Optional asset description override.")
    parser.add_argument(
        "--price-sats",
        type=int,
        help="Default listing price hint in sats.",
    )
    parser.add_argument(
        "--default-policy",
        default="private_preview_license",
        help="Listing default policy. Passed through to the normal packaging helper.",
    )
    parser.add_argument(
        "--delivery-mode",
        action="append",
        dest="delivery_modes",
        help="Allowed delivery mode. Repeat to include multiple values.",
    )
    parser.add_argument(
        "--visibility-posture",
        default="targeted_only",
        help="Visibility posture passed to the normal packaging helper.",
    )
    parser.add_argument(
        "--sensitivity-posture",
        default="private",
        help="Sensitivity posture passed to the normal packaging helper.",
    )
    parser.add_argument("--consumer-id", help="Optional starter grant consumer id.")
    parser.add_argument("--grant-price-sats", type=int, help="Optional starter grant price in sats.")
    parser.add_argument(
        "--grant-policy-template",
        help="Optional starter grant policy template. Defaults to --default-policy when unset.",
    )
    parser.add_argument("--grant-expires-hours", type=int, help="Optional starter grant expiry window in hours.")
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
    parser.add_argument(
        "--include-developer",
        action="store_true",
        help="Include redacted developer-role messages. Default behavior drops them.",
    )
    parser.add_argument(
        "--drop-tool-io",
        action="store_true",
        help="Drop tool call arguments and tool results from the exported conversations.",
    )
    parser.add_argument(
        "--redaction-tier",
        choices=("public", "restricted"),
        default="public",
        help="Redaction tier label recorded in exported metadata. Default: public",
    )
    parser.add_argument(
        "--scrub",
        action="append",
        default=[],
        help="Literal string to replace everywhere in exported text with a stable placeholder. Repeat as needed.",
    )
    return parser


def resolve_rollout_files(args: argparse.Namespace) -> tuple[list[Path], str]:
    rollout_files: list[Path] = []
    if args.session:
        for raw in args.session:
            path = Path(raw).expanduser().resolve()
            if not path.exists():
                raise ValueError(f"session path does not exist: {raw}")
            if path.is_dir():
                rollout_files.extend(sorted(path.rglob("rollout-*.jsonl")))
            elif path.is_file():
                rollout_files.append(path)
            else:
                raise ValueError(f"unsupported session path: {raw}")
        selection_mode = "explicit_session_selection"
    else:
        root = Path(args.session_root).expanduser().resolve()
        if not root.exists():
            raise ValueError(f"session root does not exist: {root}")
        rollout_files = sorted(root.rglob("rollout-*.jsonl"))
        if args.limit > 0:
            rollout_files = rollout_files[-args.limit :]
        selection_mode = "latest_from_codex_home"
    unique = sorted(dict.fromkeys(rollout_files))
    if not unique:
        raise ValueError("no rollout files matched the selected Codex session paths")
    return unique, selection_mode


def relative_rollout_path(path: Path) -> str:
    codex_sessions_root = Path("~/.codex/sessions").expanduser().resolve()
    try:
        return path.resolve().relative_to(codex_sessions_root).as_posix()
    except ValueError:
        return path.name


def extract_session_summary(payload: dict[str, Any]) -> dict[str, Any]:
    git_payload = payload.get("git")
    git_summary: dict[str, Any] | None = None
    if isinstance(git_payload, dict):
        git_summary = {
            "present": True,
            "repository_url": sanitize_value(git_payload.get("repository_url")),
            "branch_digest": short_hash(str(git_payload.get("branch", ""))) if git_payload.get("branch") else None,
            "commit_digest": short_hash(str(git_payload.get("commit_hash", ""))) if git_payload.get("commit_hash") else None,
        }
    return {
        "session_id": sanitize_value(payload.get("id")),
        "timestamp": sanitize_value(payload.get("timestamp")),
        "cli_version": sanitize_value(payload.get("cli_version")),
        "model_provider": sanitize_value(payload.get("model_provider")),
        "originator": sanitize_value(payload.get("originator")),
        "source": sanitize_value(payload.get("source")),
        "cwd": sanitize_value(payload.get("cwd")),
        "git": git_summary,
    }


def collect_turn_context(payload: dict[str, Any], summary: dict[str, set[str]]) -> None:
    for key in ("model", "approval_policy", "sandbox_policy", "timezone", "personality"):
        value = payload.get(key)
        if value:
            summary.setdefault(key, set()).add(str(sanitize_value(value)))
    collaboration_mode = payload.get("collaboration_mode")
    if collaboration_mode:
        summary.setdefault("collaboration_mode", set()).add(json.dumps(sanitize_value(collaboration_mode), sort_keys=True))


def extract_response_items(
    payload: dict[str, Any],
    *,
    include_developer: bool,
    include_tool_io: bool,
) -> list[dict[str, Any]]:
    items: list[dict[str, Any]] = []
    role = payload.get("role")
    content = payload.get("content")
    text_parts = content_text_parts(content)
    if role == "developer" and not include_developer:
        text_parts = []
    if text_parts:
        items.append(
            {
                "kind": "message",
                "role": role,
                "phase": payload.get("phase"),
                "text": redact_text("\n\n".join(text_parts)),
            }
        )
    if include_tool_io and isinstance(payload.get("arguments"), str):
        items.append(
            {
                "kind": "tool_call",
                "name": sanitize_value(payload.get("name")),
                "call_id": sanitize_value(payload.get("call_id")),
                "arguments_text": redact_text(payload["arguments"]),
            }
        )
    if include_tool_io and isinstance(payload.get("output"), str):
        items.append(
            {
                "kind": "tool_result",
                "call_id": sanitize_value(payload.get("call_id")),
                "output_text": redact_text(payload["output"]),
            }
        )
    if payload.get("action") or payload.get("status"):
        items.append(
            {
                "kind": "status",
                "action": sanitize_value(payload.get("action")),
                "status": sanitize_value(payload.get("status")),
            }
        )
    return items


def export_rollout(
    path: Path,
    export_dir: Path,
    *,
    include_developer: bool,
    include_tool_io: bool,
    redaction_tier: str,
) -> dict[str, Any]:
    session_summary: dict[str, Any] | None = None
    turn_context_summary: dict[str, set[str]] = {}
    exported_items: list[dict[str, Any]] = []
    counters: Counter[str] = Counter()

    lines = path.read_text(encoding="utf-8").splitlines()
    for line in lines:
        envelope = json.loads(line)
        item_type = envelope.get("type")
        payload = envelope.get("payload")
        counters[f"line_type:{item_type}"] += 1
        if not isinstance(payload, dict):
            continue
        if item_type == "session_meta":
            session_summary = extract_session_summary(payload)
            continue
        if item_type == "turn_context":
            collect_turn_context(payload, turn_context_summary)
            continue
        if item_type != "response_item":
            continue
        items = extract_response_items(
            payload,
            include_developer=include_developer,
            include_tool_io=include_tool_io,
        )
        if payload.get("role") == "developer" and not include_developer and content_text_parts(payload.get("content")):
            counters["dropped_developer_messages"] += 1
        for item in items:
            item["timestamp"] = envelope.get("timestamp")
            exported_items.append(item)
            counters[f"exported:{item['kind']}"] += 1

    session_id = "unknown-session"
    if session_summary and isinstance(session_summary.get("session_id"), str):
        session_id = session_summary["session_id"]
    file_stem = f"{path.stem}-redacted"
    output_path = export_dir / f"{file_stem}.json"

    export_payload = {
        "schema_version": EXPORT_SCHEMA_VERSION,
        "export_kind": "codex_redacted_conversation",
        "redaction_tier": redaction_tier,
        "source_format": "codex_rollout_jsonl",
        "source_relative_path": relative_rollout_path(path),
        "source_session_id": session_id,
        "session": session_summary,
        "turn_context": {key: sorted(values) for key, values in sorted(turn_context_summary.items())},
        "stats": {
            "rollout_line_count": len(lines),
            "exported_item_count": len(exported_items),
            "message_count": sum(1 for item in exported_items if item["kind"] == "message"),
            "tool_call_count": sum(1 for item in exported_items if item["kind"] == "tool_call"),
            "tool_result_count": sum(1 for item in exported_items if item["kind"] == "tool_result"),
            "status_item_count": sum(1 for item in exported_items if item["kind"] == "status"),
            "dropped_developer_messages": counters["dropped_developer_messages"],
        },
        "items": exported_items,
    }
    output_path.write_text(json.dumps(export_payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return {
        "session_id": session_id,
        "source_relative_path": relative_rollout_path(path),
        "export_path": str(output_path),
        "message_count": export_payload["stats"]["message_count"],
        "tool_call_count": export_payload["stats"]["tool_call_count"],
        "tool_result_count": export_payload["stats"]["tool_result_count"],
        "dropped_developer_messages": export_payload["stats"]["dropped_developer_messages"],
    }


def run_data_packager(args: argparse.Namespace, export_dir: Path, output_dir: Path) -> None:
    script_path = Path(__file__).resolve().parent / "data_market_package.py"
    command = [
        sys.executable,
        str(script_path),
        "--source",
        str(export_dir),
        "--output-dir",
        str(output_dir),
        "--title",
        args.title,
        "--asset-kind",
        DEFAULT_ASSET_KIND,
        "--default-policy",
        args.default_policy,
        "--visibility-posture",
        args.visibility_posture,
        "--sensitivity-posture",
        args.sensitivity_posture,
    ]
    if args.description:
        command.extend(["--description", args.description])
    if args.price_sats is not None:
        command.extend(["--price-sats", str(args.price_sats)])
    if args.consumer_id:
        command.extend(["--consumer-id", args.consumer_id])
    if args.grant_price_sats is not None:
        command.extend(["--grant-price-sats", str(args.grant_price_sats)])
    if args.grant_policy_template:
        command.extend(["--grant-policy-template", args.grant_policy_template])
    if args.grant_expires_hours is not None:
        command.extend(["--grant-expires-hours", str(args.grant_expires_hours)])
    if args.grant_warranty_window_hours is not None:
        command.extend(["--grant-warranty-window-hours", str(args.grant_warranty_window_hours)])
    if args.skip_grant_template:
        command.append("--skip-grant-template")
    for delivery_mode in args.delivery_modes or []:
        command.extend(["--delivery-mode", delivery_mode])
    subprocess.run(command, check=True)


def update_emitted_templates(
    output_dir: Path,
    *,
    redaction_tier: str,
    session_count: int,
    selection_mode: str,
    include_developer: bool,
    include_tool_io: bool,
) -> None:
    metadata_patch = {
        "codex_conversation_export": "true",
        "codex_redaction_tier": redaction_tier,
        "codex_session_count": str(session_count),
        "codex_selection_mode": selection_mode,
        "codex_include_developer_messages": str(include_developer).lower(),
        "codex_include_tool_io": str(include_tool_io).lower(),
        "codex_export_index_path": f"{EXPORT_DIRNAME}/{INDEX_FILENAME}",
        "codex_export_script": EXPORT_SCRIPT,
    }
    for filename in ("listing-template.json", "grant-template.json"):
        path = output_dir / filename
        if not path.exists():
            continue
        payload = json.loads(path.read_text(encoding="utf-8"))
        metadata = payload.get("metadata")
        if not isinstance(metadata, dict):
            metadata = {}
            payload["metadata"] = metadata
        metadata.update(metadata_patch)
        path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = build_arg_parser()
    args = parser.parse_args()
    try:
        configure_literal_replacements(args.scrub)
        rollout_files, selection_mode = resolve_rollout_files(args)
        output_dir = Path(args.output_dir).expanduser().resolve()
        output_dir.mkdir(parents=True, exist_ok=True)
        export_dir = output_dir / EXPORT_DIRNAME
        export_dir.mkdir(parents=True, exist_ok=True)

        exports = [
            export_rollout(
                path,
                export_dir,
                include_developer=args.include_developer,
                include_tool_io=not args.drop_tool_io,
                redaction_tier=args.redaction_tier,
            )
            for path in rollout_files
        ]
        index_payload = {
            "schema_version": EXPORT_SCHEMA_VERSION,
            "export_kind": "codex_redacted_conversation_index",
            "redaction_tier": args.redaction_tier,
            "selection_mode": selection_mode,
            "session_count": len(exports),
            "total_message_count": sum(item["message_count"] for item in exports),
            "total_tool_call_count": sum(item["tool_call_count"] for item in exports),
            "total_tool_result_count": sum(item["tool_result_count"] for item in exports),
            "exports": exports,
        }
        (export_dir / INDEX_FILENAME).write_text(
            json.dumps(index_payload, indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

        if not args.description:
            args.description = (
                f"Redacted Codex conversation bundle exported from {len(exports)} session(s) "
                f"with {args.redaction_tier} redaction."
            )

        run_data_packager(args, export_dir, output_dir)
        update_emitted_templates(
            output_dir,
            redaction_tier=args.redaction_tier,
            session_count=len(exports),
            selection_mode=selection_mode,
            include_developer=args.include_developer,
            include_tool_io=not args.drop_tool_io,
        )

        summary = {
            "schema_version": EXPORT_SCHEMA_VERSION,
            "title": args.title,
            "description": args.description,
            "redaction_tier": args.redaction_tier,
            "selection_mode": selection_mode,
            "session_count": len(exports),
            "output_dir": str(output_dir),
            "export_dir": str(export_dir),
            "index_path": str(export_dir / INDEX_FILENAME),
            "listing_template_path": str(output_dir / "listing-template.json"),
            "grant_template_path": str(output_dir / "grant-template.json")
            if not args.skip_grant_template
            else None,
        }
        print(json.dumps(summary, indent=2, sort_keys=True))
        return 0
    except Exception as exc:  # pylint: disable=broad-except
        print(f"error: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
