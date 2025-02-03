# Audio/Video Transcription

This tool transcribes audio and video files using Groq's Whisper API.

## Prerequisites

1. Install ffmpeg (required for video support):

**Ubuntu/Debian:**

```bash
sudo apt update
sudo apt install ffmpeg
```

**macOS:**

```bash
brew install ffmpeg
```

**Windows:**

```bash
choco install ffmpeg
```

2. Set your Groq API key in `.env`:

```bash
GROQ_API_KEY=your-api-key-here
```

## Usage

```bash
cargo run --bin transcribe path/to/file.[mp4|mp3|flac|...]
```

Supported formats:

- Video: `.mp4` (automatically extracts audio)
- Audio: `.flac`, `.mp3`, `.mpeg`, `.mpga`, `.m4a`, `.ogg`, `.wav`, `.webm`

## Output

Transcriptions are saved to `docs/transcripts/` with filenames in the format:

```
YYYYMMDD-HHMM-slug.md
```

For example:

```
20240305-1423-ep-157.md
```

The output file contains:

- YAML frontmatter with metadata
- Markdown-formatted transcription text with proper paragraph breaks

Example output:

```markdown
---
source_file: ep157.mp4
timestamp: 2024-03-05T14:23:00-06:00
model: whisper-large-v3
---

This is the first paragraph of the transcription.

This is the second paragraph, showing a new speaker or topic.

And this continues with proper Markdown formatting.
```

## Features

- Uses `whisper-large-v3` for highest accuracy transcription
- Automatically converts video to audio using ffmpeg
- Generates readable slugs from filenames using LLaMA
- Formats transcriptions with proper Markdown line breaks
- Uses military time in Central US timezone
- Cleans up temporary files automatically

## Models Used

- **Transcription**: `whisper-large-v3` - Highest accuracy Whisper model
- **Formatting**: `llama-3.3-70b-versatile` - Adds proper Markdown line breaks
- **Slug Generation**: `llama-3.1-8b-instant` - Creates URL-friendly filenames
