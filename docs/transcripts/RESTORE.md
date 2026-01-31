# Restoring transcripts from git history

Transcripts are consolidated in **`openagents/`** (no duplicates at repo root). If you need to re-run a full restore from git history, from the **repo root** run:

```bash
python3 restore_transcripts.py
```

The script finds every transcript path that ever existed under `docs/transcripts/` in git history, restores each from a commit where it existed, and writes everything under `docs/transcripts/openagents/` (and `openagents/dspy/` for DSPy files). Transcripts include: numbered episodes (001–198), named episodes (194–203, oa-*, ep150/ep167/ep168), dspy talks, README, and one-off files like `dont-build-agents-build-skills.md` and `20250203-1157-ep-157.md`.
