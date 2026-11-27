"""Wire frame codec: format is `<len>:<escaped_payload>:<crc>` where `:` and `\\` in payload are backslash-escaped."""


def escape_payload(s: str) -> str:
    # BUG: escapes ':' before '\\', corrupting backslashes if payload contains '\\:'
    return s.replace(":", "\\:").replace("\\", "\\\\")


def unescape_payload(s: str) -> str:
    out = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s):
            out.append(s[i + 1])
            i += 2
        else:
            out.append(s[i])
            i += 1
    return "".join(out)


def checksum(raw_payload: str) -> int:
    return sum(ord(c) for c in raw_payload) % 997


def encode_frame(payload: str) -> str:
    esc = escape_payload(payload)
    # BUG: length is computed over raw payload instead of escaped payload, while crc uses escaped payload
    return f"{len(payload)}:{esc}:{checksum(esc)}"
