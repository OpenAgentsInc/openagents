# Firm reason: 5 (subagent-bench eval fixture/verifier)
def pkcs7_pad(data: bytes, block_size: int = 16) -> bytes:
    if block_size <= 0 or block_size > 255:
        raise ValueError("invalid block size")
    # BUG 1: when len(data) is exact multiple of block_size, returns 0 padding instead of full block!
    rem = len(data) % block_size
    if rem == 0:
        return data
    pad_len = block_size - rem
    return data + bytes([pad_len]) * pad_len


def pkcs7_unpad(data: bytes, block_size: int = 16) -> bytes:
    if not data or len(data) % block_size != 0:
        raise ValueError("invalid padded length")
    pad_len = data[-1]
    if pad_len < 1 or pad_len > block_size:
        raise ValueError("invalid pad byte")
    # BUG 2: doesn't verify all trailing pad_len bytes equal pad_len!
    return data[:-pad_len]
