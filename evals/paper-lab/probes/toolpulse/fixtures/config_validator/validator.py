def parse_window_seconds(spec: str) -> int:
    """Parse duration like '30s', '5m', '2h' into integer seconds."""
    s = spec.strip().lower()
    if not s:
        raise ValueError("empty window spec")
    unit = s[-1]
    num = int(s[:-1])
    if num <= 0:
        raise ValueError("window must be positive")
    if unit == "s":
        return num
    if unit == "m":
        return num * 60
    raise ValueError(f"unsupported unit: {unit}")
