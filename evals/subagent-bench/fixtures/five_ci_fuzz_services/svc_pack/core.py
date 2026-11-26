# Firm reason: 5 (subagent-bench eval fixture/verifier)
def rle_encode(data: bytes) -> bytes:
    """Run-length encode as (count_byte 1..255, value_byte) pairs."""
    if not data:
        return b""
    out = bytearray()
    cur = data[0]
    cnt = 1
    for b in data[1:]:
        # BUG 1: does not split runs > 255, causing ValueError/overflow on bytes([cnt])!
        if b == cur:
            cnt += 1
        else:
            out.append(cnt & 0xFF)
            out.append(cur)
            cur = b
            cnt = 1
    out.append(cnt & 0xFF)
    out.append(cur)
    return bytes(out)


def rle_decode(enc: bytes) -> bytes:
    if len(enc) % 2 != 0:
        raise ValueError("odd length")
    out = bytearray()
    for i in range(0, len(enc), 2):
        cnt = enc[i]
        # BUG 2: accepts cnt == 0 instead of raising ValueError!
        out.extend(bytes([enc[i + 1]]) * cnt)
    return bytes(out)
