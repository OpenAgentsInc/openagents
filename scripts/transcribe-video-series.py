#!/usr/bin/env python3
"""Download and transcribe OpenAgents video-series episodes.

The script reads the OpenAgents GitHub wiki video-series markdown, downloads
episode media with yt-dlp, extracts compact MP3 audio with ffmpeg, transcribes
audio through the OpenAI audio transcription API, and writes markdown transcripts
under docs/transcripts.
"""

from __future__ import annotations

import argparse
import json
import mimetypes
import os
import re
import shutil
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable, Optional


DEFAULT_WIKI_URL = (
    "https://raw.githubusercontent.com/wiki/OpenAgentsInc/openagents/Video-Series.md"
)
DEFAULT_WORK_DIR = "var/video-series-transcripts"
DEFAULT_TRANSCRIPT_DIR = "docs/transcripts"
DEFAULT_MODEL = "gpt-4o-transcribe-diarize"
DEFAULT_FORMAT = "bestaudio/best[height<=270]/worst"
TRANSCRIPTION_URL = "https://api.openai.com/v1/audio/transcriptions"


@dataclass(frozen=True)
class Episode:
    number: int
    title: str
    url: str


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def sanitize_ascii(value: str) -> str:
    replacements = {
        "\u2018": "'",
        "\u2019": "'",
        "\u201c": '"',
        "\u201d": '"',
        "\u2013": "-",
        "\u2014": "-",
        "\u2026": "...",
        "\u00a0": " ",
    }
    for source, target in replacements.items():
        value = value.replace(source, target)
    return value.encode("ascii", "ignore").decode("ascii")


def slugish(value: str) -> str:
    value = sanitize_ascii(value).lower()
    value = re.sub(r"[^a-z0-9]+", "-", value)
    return value.strip("-") or "episode"


def extraction_url(url: str) -> str:
    normalized = (
        url.replace("https://twitter.com/", "https://x.com/")
        .replace("http://twitter.com/", "https://x.com/")
    )
    if "x.com/" not in normalized:
        return normalized

    # The wiki has older @OpenAgentsInc URLs. X's media extractor sometimes
    # fails on those even when the post is now canonical under @OpenAgents.
    oembed_url = (
        "https://publish.twitter.com/oembed?url="
        + urllib.parse.quote(normalized, safe="")
    )
    try:
        with urllib.request.urlopen(oembed_url, timeout=20) as response:
            data = json.loads(response.read().decode("utf-8"))
    except (OSError, TimeoutError, urllib.error.URLError, json.JSONDecodeError):
        return normalized

    canonical = data.get("url")
    if isinstance(canonical, str) and canonical:
        return canonical
    return normalized


def load_env_file(path: Path) -> None:
    if not path.exists():
        raise SystemExit(f"env file not found: {path}")
    for raw_line in path.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip('"').strip("'")
        if key and key not in os.environ:
            os.environ[key] = value


def get_api_key() -> str:
    key = os.environ.get("OPENAI_API_KEY") or os.environ.get("PROBE_OPENAI_API_KEY")
    if not key:
        raise SystemExit(
            "missing OPENAI_API_KEY or PROBE_OPENAI_API_KEY; pass --env-file or export one"
        )
    return key


def command_path(name: str) -> str:
    path = shutil.which(name)
    if not path:
        raise SystemExit(f"required command not found on PATH: {name}")
    return path


def ytdlp_command() -> list[str]:
    if shutil.which("yt-dlp"):
        return ["yt-dlp"]
    if shutil.which("uvx"):
        return ["uvx", "yt-dlp"]
    raise SystemExit("required command not found: yt-dlp or uvx")


def run(cmd: list[str], cwd: Path, *, capture: bool = False) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(
        cmd,
        cwd=str(cwd),
        text=True,
        capture_output=capture,
        check=False,
    )
    if result.returncode != 0:
        if capture and result.stdout:
            sys.stderr.write(result.stdout)
        if capture and result.stderr:
            sys.stderr.write(result.stderr)
        raise SystemExit(f"command failed with exit code {result.returncode}: {' '.join(cmd)}")
    return result


def fetch_wiki(url: str) -> str:
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read().decode("utf-8")


def parse_episodes(markdown: str) -> list[Episode]:
    episodes: list[Episode] = []
    pattern = re.compile(r"^(\d+)\.\s+\[(.*?)\]\((.*?)\)\s*$")
    for line in markdown.splitlines():
        match = pattern.match(line.strip())
        if not match:
            continue
        episodes.append(
            Episode(
                number=int(match.group(1)),
                title=sanitize_ascii(match.group(2)),
                url=match.group(3),
            )
        )
    return episodes


def parse_speaker_map(values: Iterable[str]) -> dict[str, str]:
    mapping: dict[str, str] = {}
    for value in values:
        if "=" not in value:
            raise SystemExit(f"--speaker must use label=name format, got: {value}")
        key, name = value.split("=", 1)
        mapping[key.strip()] = sanitize_ascii(name.strip())
    return mapping


def transcript_path(transcript_dir: Path, episode: Episode) -> Path:
    return transcript_dir / f"{episode.number}.md"


def select_episodes(
    episodes: list[Episode],
    transcript_dir: Path,
    *,
    episode_number: Optional[int],
    missing: bool,
    overwrite: bool,
    limit: Optional[int],
    reverse: bool,
) -> list[Episode]:
    if episode_number is not None:
        selected = [episode for episode in episodes if episode.number == episode_number]
        if not selected:
            raise SystemExit(f"episode not found in wiki: {episode_number}")
    elif missing:
        selected = [
            episode
            for episode in episodes
            if overwrite or not transcript_path(transcript_dir, episode).exists()
        ]
    else:
        raise SystemExit("choose --episode <number> or --missing")

    if reverse:
        selected = list(reversed(selected))
    if limit is not None:
        selected = selected[:limit]
    return selected


def download_metadata(episode: Episode, episode_dir: Path, root: Path) -> dict[str, Any]:
    info_path = episode_dir / "episode.info.json"
    if info_path.exists():
        return json.loads(info_path.read_text())

    cmd = ytdlp_command() + [
        "--no-warnings",
        "--dump-single-json",
        "--skip-download",
        extraction_url(episode.url),
    ]
    print(f"metadata: episode {episode.number} {extraction_url(episode.url)}")
    result = run(cmd, root, capture=True)
    info_path.write_text(result.stdout)
    return json.loads(result.stdout)


def download_media(episode: Episode, episode_dir: Path, root: Path, fmt: str, force: bool) -> list[Path]:
    media_dir = episode_dir / "media"
    media_dir.mkdir(parents=True, exist_ok=True)
    existing = sorted(
        path
        for path in media_dir.iterdir()
        if path.is_file() and path.suffix.lower() in {".mp4", ".m4a", ".webm", ".mkv", ".mov"}
    )
    if existing and not force:
        return existing

    cmd = ytdlp_command() + [
        "--yes-playlist",
        "--no-warnings",
        "--no-progress",
        "-f",
        fmt,
        "--merge-output-format",
        "mp4",
        "-o",
        str(media_dir / "media-%(id)s.%(ext)s"),
        extraction_url(episode.url),
    ]
    if force:
        cmd.append("--force-overwrites")

    print(f"download: episode {episode.number} media")
    run(cmd, root)
    media_files = sorted(
        path
        for path in media_dir.iterdir()
        if path.is_file() and path.suffix.lower() in {".mp4", ".m4a", ".webm", ".mkv", ".mov"}
    )
    if not media_files:
        raise SystemExit(f"yt-dlp produced no media files for episode {episode.number}")
    return media_files


def ffprobe_duration(path: Path, root: Path) -> float:
    command_path("ffprobe")
    result = run(
        [
            "ffprobe",
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            str(path),
        ],
        root,
        capture=True,
    )
    try:
        return float(result.stdout.strip())
    except ValueError:
        return 0.0


def extract_audio(
    media_files: list[Path],
    episode_dir: Path,
    root: Path,
    force: bool,
    max_audio_seconds: int,
) -> list[Path]:
    command_path("ffmpeg")
    audio_dir = episode_dir / "audio"
    audio_dir.mkdir(parents=True, exist_ok=True)
    audio_files: list[Path] = []

    for index, media_path in enumerate(media_files, start=1):
        pattern = f"audio-part-{index:02d}-*.mp3"
        existing = sorted(audio_dir.glob(pattern))
        if existing and not force:
            audio_files.extend(existing)
            continue

        for stale in existing:
            stale.unlink()

        output_pattern = audio_dir / f"audio-part-{index:02d}-%03d.mp3"
        print(f"audio: {media_path.name} -> {output_pattern.name}")
        run(
            [
                "ffmpeg",
                "-y",
                "-i",
                str(media_path),
                "-vn",
                "-ac",
                "1",
                "-ar",
                "16000",
                "-b:a",
                "64k",
                "-f",
                "segment",
                "-segment_time",
                str(max_audio_seconds),
                "-reset_timestamps",
                "1",
                str(output_pattern),
            ],
            root,
        )
        generated = sorted(audio_dir.glob(pattern))
        if not generated:
            raise SystemExit(f"ffmpeg produced no audio chunks for {media_path}")
        audio_files.extend(generated)
    return audio_files


def multipart_body(fields: dict[str, str], file_field: str, file_path: Path) -> tuple[bytes, str]:
    boundary = f"----openagents-{uuid.uuid4().hex}"
    body = bytearray()

    def add(value: bytes) -> None:
        body.extend(value)

    for name, value in fields.items():
        add(f"--{boundary}\r\n".encode())
        add(f'Content-Disposition: form-data; name="{name}"\r\n\r\n'.encode())
        add(str(value).encode())
        add(b"\r\n")

    content_type = mimetypes.guess_type(file_path.name)[0] or "application/octet-stream"
    add(f"--{boundary}\r\n".encode())
    add(
        (
            f'Content-Disposition: form-data; name="{file_field}"; '
            f'filename="{file_path.name}"\r\n'
        ).encode()
    )
    add(f"Content-Type: {content_type}\r\n\r\n".encode())
    add(file_path.read_bytes())
    add(b"\r\n")
    add(f"--{boundary}--\r\n".encode())
    return bytes(body), f"multipart/form-data; boundary={boundary}"


def transcribe_audio(
    audio_path: Path,
    output_json: Path,
    *,
    api_key: str,
    model: str,
    language: str,
    force: bool,
) -> dict[str, Any]:
    if output_json.exists() and not force:
        return json.loads(output_json.read_text())

    fields = {"model": model, "language": language}
    if model.endswith("-diarize"):
        fields["response_format"] = "diarized_json"
        fields["chunking_strategy"] = "auto"
    elif model == "whisper-1":
        fields["response_format"] = "verbose_json"
        fields["timestamp_granularities[]"] = "segment"
    else:
        fields["response_format"] = "json"

    body, content_type = multipart_body(fields, "file", audio_path)
    request = urllib.request.Request(
        TRANSCRIPTION_URL,
        data=body,
        method="POST",
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": content_type,
        },
    )

    print(f"transcribe: {audio_path.name} using {model}")
    try:
        with urllib.request.urlopen(request, timeout=900) as response:
            raw = response.read().decode("utf-8")
    except urllib.error.HTTPError as error:
        raw_error = error.read().decode("utf-8", errors="replace")
        raise SystemExit(f"transcription API failed: HTTP {error.code}: {raw_error}") from error

    output_json.write_text(raw)
    return json.loads(raw)


def segment_start(segment: dict[str, Any]) -> float:
    for key in ("start", "start_time", "start_ms"):
        value = segment.get(key)
        if isinstance(value, (int, float)):
            return float(value) / (1000.0 if key.endswith("_ms") else 1.0)
    return 0.0


def segment_text(segment: dict[str, Any]) -> str:
    text = segment.get("text")
    if isinstance(text, str):
        return text.strip()
    return ""


def segment_speaker(segment: dict[str, Any]) -> str:
    for key in ("speaker", "speaker_label", "speaker_id"):
        value = segment.get(key)
        if value is not None:
            return str(value)
    return "speaker"


def normalize_speaker(label: str, speaker_map: dict[str, str]) -> str:
    if label in speaker_map:
        return speaker_map[label]
    lower = label.lower()
    if lower in speaker_map:
        return speaker_map[lower]
    match = re.search(r"(\d+)$", lower)
    if match:
        return f"Speaker {int(match.group(1))}"
    if len(label) == 1 and label.isalpha():
        return f"Speaker {label.upper()}"
    if lower in {"speaker", "unknown"}:
        return "Speaker"
    return sanitize_ascii(label.replace("_", " ").title())


def response_segments(
    response: dict[str, Any],
    *,
    offset_seconds: float,
    speaker_map: dict[str, str],
) -> list[dict[str, Any]]:
    raw_segments = response.get("segments")
    if isinstance(raw_segments, list) and raw_segments:
        output = []
        for raw_segment in raw_segments:
            if not isinstance(raw_segment, dict):
                continue
            text = sanitize_ascii(segment_text(raw_segment))
            if not text:
                continue
            output.append(
                {
                    "start": offset_seconds + segment_start(raw_segment),
                    "speaker": normalize_speaker(segment_speaker(raw_segment), speaker_map),
                    "text": text,
                }
            )
        if output:
            return output

    text = response.get("text")
    if isinstance(text, str) and text.strip():
        return [
            {
                "start": offset_seconds,
                "speaker": normalize_speaker("speaker", speaker_map),
                "text": sanitize_ascii(text.strip()),
            }
        ]
    return []


def format_timestamp(seconds: float) -> str:
    seconds = max(0, int(seconds))
    hours = seconds // 3600
    minutes = (seconds % 3600) // 60
    secs = seconds % 60
    if hours:
        return f"{hours:02d}:{minutes:02d}:{secs:02d}"
    return f"{minutes:02d}:{secs:02d}"


def wrap_text(text: str) -> str:
    text = re.sub(r"\s+", " ", text).strip()
    return text


def write_markdown(
    episode: Episode,
    *,
    info: dict[str, Any],
    segments: list[dict[str, Any]],
    transcript_file: Path,
    wiki_url: str,
    model: str,
) -> None:
    title = sanitize_ascii(info.get("title") or episode.title)
    upload_date = sanitize_ascii(str(info.get("upload_date") or "unknown"))
    generated_at = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    lines = [
        f"# Transcription: OpenAgents Episode {episode.number} - {episode.title}",
        "",
        f"Source: {episode.url}",
        f"Wiki source: {wiki_url}",
        f"Media title: {title}",
        f"Upload date: {upload_date}",
        f"Transcription model: {model}",
        f"Generated at: {generated_at}",
        "",
        "Machine-generated transcript. Review speaker labels and wording before",
        "using this as quote-grade source material.",
        "",
    ]

    if not segments:
        lines.append("_No transcript segments were returned._")
    else:
        for segment in segments:
            timestamp = format_timestamp(float(segment["start"]))
            speaker = sanitize_ascii(str(segment["speaker"]))
            text = wrap_text(sanitize_ascii(str(segment["text"])))
            lines.append(f"**[{timestamp}] {speaker}:** {text}")
            lines.append("")

    transcript_file.parent.mkdir(parents=True, exist_ok=True)
    transcript_file.write_text("\n".join(lines).rstrip() + "\n")


def process_episode(args: argparse.Namespace, episode: Episode, root: Path, speaker_map: dict[str, str]) -> None:
    transcript_dir = root / args.output_dir
    output_path = transcript_path(transcript_dir, episode)
    if output_path.exists() and not args.overwrite:
        print(f"skip: episode {episode.number} transcript exists at {output_path}")
        return

    episode_dir = root / args.work_dir / f"{episode.number:03d}-{slugish(episode.title)}"
    episode_dir.mkdir(parents=True, exist_ok=True)

    info = download_metadata(episode, episode_dir, root)
    media_files = download_media(episode, episode_dir, root, args.format, args.force_download)
    audio_files = extract_audio(
        media_files,
        episode_dir,
        root,
        args.force_audio,
        args.max_audio_seconds,
    )

    if args.download_only:
        print(f"download-only: episode {episode.number} media/audio retained under {episode_dir}")
        return

    api_key = get_api_key()
    transcript_json_dir = episode_dir / "transcriptions"
    transcript_json_dir.mkdir(parents=True, exist_ok=True)

    all_segments: list[dict[str, Any]] = []
    offset = 0.0
    for index, audio_path in enumerate(audio_files, start=1):
        output_json = transcript_json_dir / f"transcription-part-{index:02d}.json"
        response = transcribe_audio(
            audio_path,
            output_json,
            api_key=api_key,
            model=args.model,
            language=args.language,
            force=args.force_transcribe,
        )
        all_segments.extend(
            response_segments(response, offset_seconds=offset, speaker_map=speaker_map)
        )
        offset += ffprobe_duration(audio_path, root)
        time.sleep(args.api_pause_seconds)

    write_markdown(
        episode,
        info=info,
        segments=all_segments,
        transcript_file=output_path,
        wiki_url=args.wiki_url,
        model=args.model,
    )
    print(f"wrote: {output_path}")


def write_failure(episode: Episode, episode_dir: Path, error: BaseException) -> None:
    payload = {
        "episode": episode.number,
        "title": episode.title,
        "url": episode.url,
        "failed_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "error": str(error),
    }
    failure_path = episode_dir / "failure.json"
    failure_path.parent.mkdir(parents=True, exist_ok=True)
    failure_path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        description="Download and transcribe OpenAgents wiki video-series episodes."
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--episode", type=int, help="transcribe one episode number")
    group.add_argument(
        "--missing",
        "--all-missing",
        action="store_true",
        help="transcribe every wiki episode missing docs/transcripts/<episode>.md",
    )
    parser.add_argument("--wiki-url", default=DEFAULT_WIKI_URL)
    parser.add_argument("--output-dir", default=DEFAULT_TRANSCRIPT_DIR)
    parser.add_argument("--work-dir", default=DEFAULT_WORK_DIR)
    parser.add_argument("--env-file", type=Path, help="optional env file containing OPENAI_API_KEY or PROBE_OPENAI_API_KEY")
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--language", default="en")
    parser.add_argument("--format", default=DEFAULT_FORMAT, help="yt-dlp format selector")
    parser.add_argument("--speaker", action="append", default=[], help="map model speaker label to name, e.g. speaker_0=Christopher David")
    parser.add_argument("--limit", type=int, help="limit selected episodes, useful with --missing")
    parser.add_argument("--reverse", action="store_true", help="process selected episodes in descending wiki order")
    parser.add_argument("--overwrite", action="store_true", help="overwrite existing markdown transcript")
    parser.add_argument("--force-download", action="store_true", help="force redownload of media")
    parser.add_argument("--force-audio", action="store_true", help="force regeneration of audio")
    parser.add_argument("--force-transcribe", action="store_true", help="force API transcription even if raw JSON exists")
    parser.add_argument("--max-audio-seconds", type=int, default=1200, help="maximum seconds per extracted audio chunk")
    parser.add_argument("--download-only", action="store_true", help="download media and audio but do not call transcription API")
    parser.add_argument("--dry-run", action="store_true", help="print selected episodes and exit")
    parser.add_argument("--keep-going", action="store_true", help="continue batch runs after an episode failure")
    parser.add_argument("--api-pause-seconds", type=float, default=0.0, help="pause between audio-part transcription requests")
    return parser


def main() -> int:
    args = build_parser().parse_args()
    root = repo_root()

    if args.env_file:
        load_env_file((root / args.env_file).resolve() if not args.env_file.is_absolute() else args.env_file)

    markdown = fetch_wiki(args.wiki_url)
    episodes = parse_episodes(markdown)
    if not episodes:
        raise SystemExit(f"no episodes parsed from wiki source: {args.wiki_url}")

    selected = select_episodes(
        episodes,
        root / args.output_dir,
        episode_number=args.episode,
        missing=args.missing,
        overwrite=args.overwrite,
        limit=args.limit,
        reverse=args.reverse,
    )

    if args.dry_run:
        for episode in selected:
            print(f"{episode.number}: {episode.title} <{episode.url}>")
        print(f"selected {len(selected)} episode(s)")
        return 0

    speaker_map = parse_speaker_map(args.speaker)
    failures = []
    for episode in selected:
        try:
            process_episode(args, episode, root, speaker_map)
        except SystemExit as error:
            code = error.code
            if isinstance(code, int) and code == 0:
                continue
            episode_dir = root / args.work_dir / f"{episode.number:03d}-{slugish(episode.title)}"
            write_failure(episode, episode_dir, error)
            failures.append((episode, str(error)))
            print(f"failed: episode {episode.number}: {error}", file=sys.stderr)
            if not args.keep_going:
                raise

    if failures:
        print("episode failures:")
        for episode, error in failures:
            print(f"- {episode.number}: {episode.title}: {error}")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
