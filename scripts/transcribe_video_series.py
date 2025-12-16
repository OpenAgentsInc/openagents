#!/usr/bin/env python3
"""
Download OpenAgents video series entries and transcribe them with Whisper.

Usage:
  python scripts/transcribe_video_series.py --limit 2
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import List, Optional, Sequence, Set, Tuple
from urllib.parse import urlparse, urlunparse

try:
    import whisper  # type: ignore
except ImportError as exc:  # pragma: no cover - runtime dependency hint
    raise SystemExit(
        "The `whisper` package is required. Install with `pip install openai-whisper`."
    ) from exc


EPISODE_LINE = re.compile(r"^\s*(\d+)\.\s*\[(.+?)\]\((https?://[^)]+)\)")


@dataclass
class Episode:
    index: int
    title: str
    url: str


def parse_series(series_path: Path) -> List[Episode]:
    episodes: List[Episode] = []
    for line in series_path.read_text(encoding="utf-8").splitlines():
        match = EPISODE_LINE.match(line)
        if not match:
            continue
        episodes.append(
            Episode(index=int(match.group(1)), title=match.group(2).strip(), url=match.group(3).strip())
        )
    return episodes


def gather_existing_numbers(transcripts_dir: Path) -> Set[int]:
    transcripts_dir.mkdir(parents=True, exist_ok=True)
    numbers: Set[int] = set()
    for path in transcripts_dir.glob("*.md"):
        for part in re.findall(r"(\d{1,3})", path.stem):
            numbers.add(int(part))
        try:
            with path.open("r", encoding="utf-8", errors="ignore") as handle:
                for _ in range(32):
                    line = handle.readline()
                    if not line:
                        break
                    match = re.search(r"Episode\s+(\d{1,3})", line)
                    if match:
                        numbers.add(int(match.group(1)))
                        break
        except OSError:
            continue
    return numbers


def yt_dlp_download(episode: Episode, download_dir: Path, cookies_from_browser: Optional[str]) -> Tuple[Path, dict]:
    download_dir.mkdir(parents=True, exist_ok=True)
    output_template = download_dir / f"episode-{episode.index:03}.%(ext)s"
    errors: List[str] = []

    for candidate in candidate_urls(episode.url):
        cmd = [
            "yt-dlp",
            "--no-progress",
            "--print-json",
            "-o",
            str(output_template),
            candidate,
        ]
        if cookies_from_browser:
            cmd.extend(["--cookies-from-browser", cookies_from_browser])

        result = subprocess.run(cmd, capture_output=True, text=True)
        if result.returncode != 0:
            errors.append(result.stderr.strip() or result.stdout.strip())
            continue

        info = _extract_json_line(result.stdout)
        media_path = _resolve_download_path(download_dir, episode.index)
        if media_path:
            return media_path, info or {}
        errors.append("Download completed but media file was not found.")

    failure_msg = errors[-1] if errors else "Unknown error"
    raise RuntimeError(f"yt-dlp failed for {episode.url}: {failure_msg}")


def _extract_json_line(output: str) -> Optional[dict]:
    for line in output.splitlines():
        line = line.strip()
        if not line.startswith("{"):
            continue
        try:
            return json.loads(line)
        except json.JSONDecodeError:
            continue
    return None


def _resolve_download_path(download_dir: Path, index: int) -> Optional[Path]:
    candidates = sorted(download_dir.glob(f"episode-{index:03}.*"))
    media_exts = {".mp4", ".webm", ".mkv", ".m4a", ".mov"}
    for path in candidates:
        if path.suffix in media_exts and not path.name.endswith(".part"):
            return path
    return None


def candidate_urls(url: str) -> List[str]:
    parsed = urlparse(url)
    candidates = [url]
    if "twitter.com" in parsed.netloc:
        candidates.append(urlunparse(parsed._replace(netloc="x.com")))
    elif parsed.netloc == "x.com":
        candidates.append(urlunparse(parsed._replace(netloc="twitter.com")))
    return candidates


def format_duration(seconds: Optional[float]) -> str:
    if not seconds:
        return "Unknown"
    seconds_int = int(round(seconds))
    hours, remainder = divmod(seconds_int, 3600)
    minutes, secs = divmod(remainder, 60)
    if hours:
        return f"{hours:d}:{minutes:02d}:{secs:02d}"
    return f"{minutes:d}:{secs:02d}"


def format_date(yyyymmdd: Optional[str]) -> str:
    if not yyyymmdd:
        return "Unknown"
    try:
        parsed = dt.datetime.strptime(yyyymmdd, "%Y%m%d")
        return parsed.strftime("%B %d, %Y")
    except ValueError:
        return "Unknown"


def render_markdown(
    episode: Episode,
    info: dict,
    transcript: dict,
    model_name: str,
) -> str:
    title = info.get("fulltitle") or info.get("title") or episode.title
    creator = info.get("uploader") or info.get("channel") or "Unknown"
    duration = format_duration(info.get("duration"))
    published = format_date(info.get("upload_date"))
    language = transcript.get("language") or "unknown"
    transcribed = dt.datetime.utcnow().strftime("%B %d, %Y")

    header = [
        f"# OpenAgents Video Series - Episode {episode.index:03d}",
        "",
        "## Video Information",
        "",
        f"- **Title**: {title}",
        f"- **Creator**: {creator}",
        f"- **Duration**: {duration}",
        f"- **Published**: {published}",
        f"- **URL**: {episode.url}",
        f"- **Transcribed**: {transcribed}",
        f"- **Language**: {language}",
        "",
        f"This transcript was automatically generated using Whisper ({model_name}).",
        "",
        "## Full Transcript",
        "",
        transcript.get("text", "").strip(),
        "",
        "## Timestamped Transcript",
        "",
    ]

    body_lines: List[str] = []
    for segment in transcript.get("segments", []):
        start = _seconds_to_timestamp(segment.get("start", 0))
        end = _seconds_to_timestamp(segment.get("end", 0))
        text = segment.get("text", "").strip()
        body_lines.append(f"[{start} - {end}] {text}")

    return "\n".join(header + body_lines) + "\n"


def _seconds_to_timestamp(seconds: float) -> str:
    seconds_int = int(round(seconds))
    minutes, secs = divmod(seconds_int, 60)
    hours, minutes = divmod(minutes, 60)
    if hours:
        return f"{hours:d}:{minutes:02d}:{secs:02d}"
    return f"{minutes:02d}:{secs:02d}"


def main(argv: Optional[Sequence[str]] = None) -> int:
    repo_root = Path(__file__).resolve().parent.parent
    default_series = repo_root.parent / "openagents.wiki" / "Video-Series.md"
    default_transcripts = repo_root / "docs" / "transcripts"

    parser = argparse.ArgumentParser(description="Download and transcribe OpenAgents video series episodes.")
    parser.add_argument("--series-file", type=Path, default=default_series, help="Path to Video-Series.md")
    parser.add_argument("--transcripts-dir", type=Path, default=default_transcripts, help="Destination for transcripts")
    parser.add_argument("--limit", type=int, default=0, help="Process only the first N missing episodes")
    parser.add_argument("--model", type=str, default="base", help="Whisper model name")
    parser.add_argument("--language", type=str, default=None, help="Force language (e.g., en)")
    parser.add_argument("--cookies-from-browser", type=str, default=None, help="Pass to yt-dlp if login is required")
    parser.add_argument("--overwrite", action="store_true", help="Recreate transcripts even if present")
    parser.add_argument(
        "--order",
        choices=["asc", "desc"],
        default="asc",
        help="Order to process missing episodes (default: ascending/index order)",
    )
    args = parser.parse_args(argv)

    episodes = parse_series(args.series_file)
    if not episodes:
        raise SystemExit(f"No episodes found in {args.series_file}")

    existing = set() if args.overwrite else gather_existing_numbers(args.transcripts_dir)
    missing = [ep for ep in episodes if ep.index not in existing]
    missing.sort(key=lambda ep: ep.index, reverse=args.order == "desc")
    if args.limit and args.limit > 0:
        missing = missing[: args.limit]

    if not missing:
        print("No missing episodes to process.")
        return 0

    print(f"Found {len(missing)} missing episodes. Preparing to process {len(missing)}.")
    model = whisper.load_model(args.model)

    failures: List[Tuple[Episode, Exception]] = []
    for episode in missing:
        print(f"Processing episode {episode.index:03d}: {episode.title}")
        try:
            with tempfile.TemporaryDirectory() as tmpdir:
                tmp_path = Path(tmpdir)
                media_path, info = yt_dlp_download(episode, tmp_path, args.cookies_from_browser)
                result = model.transcribe(str(media_path), language=args.language, verbose=False)
            markdown = render_markdown(episode, info, result, args.model)
            output_path = args.transcripts_dir / f"{episode.index:03d}.md"
            output_path.write_text(markdown, encoding="utf-8")
            print(f"Saved transcript to {output_path}")
        except Exception as exc:  # pragma: no cover - runtime workflow
            failures.append((episode, exc))
            print(f"Failed episode {episode.index:03d}: {exc}")

    if failures:
        print("The following episodes failed:")
        for episode, exc in failures:
            print(f"- {episode.index:03d} ({episode.url}): {exc}")
        return 1

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
