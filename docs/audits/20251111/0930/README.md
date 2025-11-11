# Deterministic Heuristics Audit — 2025-11-11 09:30

Scope: Identify all places where deterministic heuristics are used for interpretation, classification, or decision‑making, and propose FM‑based replacements using on‑device Apple Foundation Models when available, with minimal deterministic fallbacks.

Why: The repo mandates an LLM‑First policy. Deterministic rules (e.g., keyword checks, thresholds, regex‑based interpretation) should be replaced by FM‑driven calls for summaries, classifications, intent extraction, and plan generation.

See index.md for the table of contents and links to findings and actions.

