# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
def normalize(s):
    """Lowercase, trimmed, inner whitespace collapsed."""
    return ' '.join(s.strip().lower().split())


def username(name):
    """A user name from a display name."""
    return normalize(name).replace(' ', '_')
