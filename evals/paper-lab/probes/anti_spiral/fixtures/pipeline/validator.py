# Firm reason: 5 (Paper-Lab eval fixture: a tiny Python project the live agent edits in a temp repo)
"""Frame validator: parses `<len>:<escaped_payload>:<crc>` without splitting on escaped `\\:`."""
from codec import checksum, unescape_payload


def split_frame(frame: str) -> tuple[int, str, int]:
    first_colon = frame.find(":")
    if first_colon <= 0:
        raise ValueError("missing length")
    declared_len = int(frame[:first_colon])
    rest = frame[first_colon + 1:]
    # BUG: splits on last ':' unconditionally, failing if payload ends with escaped '\:' and ignores declared_len
    last_colon = rest.rfind(":")
    if last_colon < 0:
        raise ValueError("missing checksum")
    esc = rest[:last_colon]
    crc = int(rest[last_colon + 1:])
    if len(esc) != declared_len:
        raise ValueError(f"length mismatch: {len(esc)} != {declared_len}")
    return declared_len, esc, crc


def decode_and_verify(frame: str) -> str:
    _, esc, crc = split_frame(frame)
    raw = unescape_payload(esc)
    if checksum(raw) != crc:
        raise ValueError(f"bad checksum: expected {checksum(raw)}, got {crc}")
    return raw
