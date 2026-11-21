# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
import re


def emails(text):
    """Every email address in text, in order."""
    return re.findall(r'[a-z]+@[a-z]+\.com', text)
