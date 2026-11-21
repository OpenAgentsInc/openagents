# Firm reason: 5 (Tier 2 eval fixture: a tiny Python project the live agent edits in a temp repo)
import re


def slugify(title):
    """A URL slug: lowercase words joined by single hyphens, with no punctuation."""
    words = re.split(r"\s+", title.strip())
    return "-".join(words)
