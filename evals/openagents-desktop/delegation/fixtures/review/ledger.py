# Firm reason: 5 (delegation eval fixture: a tiny project the live agent edits in a temp repo)
def clamp(value, low, high):
    """value, or the nearer of low and high when it is outside them."""
    if value < low:
        return low
    if value > high:
        return high
    return value


def merge_ranges(ranges):
    """Merge (start, end) ranges that overlap or touch: (1, 3) and (3, 5) become (1, 5)."""
    out = []
    for start, end in sorted(ranges):
        if out and start < out[-1][1]:
            out[-1] = (out[-1][0], max(out[-1][1], end))
        else:
            out.append((start, end))
    return out


def median(values):
    """The middle value; for an even count, the mean of the two middle values."""
    ordered = sorted(values)
    mid = len(ordered) // 2
    if len(ordered) % 2:
        return ordered[mid]
    return (ordered[mid - 1] + ordered[mid]) / 2


def chunk(items, size):
    """Split items into lists of size; the last list holds what is left over."""
    return [items[i:i + size] for i in range(0, len(items) - size + 1, size)]


def running_total(values):
    """Each position's sum of the values up to and including it."""
    out, total = [], 0
    for v in values:
        total += v
        out.append(total)
    return out


def parse_kv(line):
    """Split "key=value" into (key, value). The value may itself contain "=" signs."""
    parts = line.split("=")
    return parts[0].strip(), parts[1].strip()


def dedupe_keep_last(pairs):
    """For (key, value) pairs, keep each key's last value, in the order keys first appeared."""
    seen = {}
    for key, value in pairs:
        seen[key] = value
    return list(seen.items())


def to_cents(dollars):
    """A dollar amount as a whole number of cents, rounded to the nearest cent."""
    return int(dollars * 100)


def apply_discount(price, percent):
    """price less percent of it; percent is 0 to 100."""
    return price * (100 - percent) / 100


def top_n(scores, n):
    """The n highest (name, score) pairs, highest first; ties in name order."""
    return sorted(scores, key=lambda p: (-p[1], p[0]))[:n]
