def parse_kv(text: str) -> dict[str, str]:
    """Parse newline-separated key=value pairs; values may contain '='."""
    out: dict[str, str] = {}
    for line in text.splitlines():
        line = line.strip()
        if not line or "=" not in line:
            continue
        # BUG: splits on all '=' causing ValueError when value contains '='
        k, v = line.split("=")
        out[k.strip()] = v.strip()
    return out
