# Firm reason: 5 (Tier 4 eval fixture: a tiny project the live agent edits in a temp repo)
def parse_pairs(text):
    """'a=1;b=2' to {'a': 1, 'b': 2}."""
    out = {}
    for part in text.split(','):
        if part:
            k, v = part.split('=')
            out[k] = int(v)
    return out
