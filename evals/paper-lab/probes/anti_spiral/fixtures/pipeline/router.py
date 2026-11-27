"""Message router built on codec + validator."""
from codec import encode_frame
from validator import decode_and_verify


def roundtrip_batch(messages: list[str]) -> list[str]:
    frames = [encode_frame(m) for m in messages]
    return [decode_and_verify(f) for f in frames]
