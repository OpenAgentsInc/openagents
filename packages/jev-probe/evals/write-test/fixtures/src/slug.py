"""URL slugs from titles."""

import re


def slugify(title):
    """A lowercase, hyphenated slug with trailing punctuation dropped."""
    text = title.strip().lower()
    text = re.sub(r"[^\w\s-]+$", "", text)
    text = re.sub(r"[\s_]+", "-", text)
    return re.sub(r"-+", "-", text).strip("-")
