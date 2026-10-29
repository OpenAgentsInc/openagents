# Phrase-gate v2 draft namespace

Reserved proposal namespace: `openagents.access.phrase.v2`. This is not a
registered stable protobuf module, generated API or enabled writer/reader.

The proposed authenticated cryptogram is an exact packed byte format, specified
in [the format review draft](../../../../../docs/protocol/2026-10-28-phrase-gate-v2.md).
If a later protobuf projection carries it, that field carries the **original
cryptogram bytes**. Its protobuf encoding never becomes the MAC input and
decoded fields never substitute for the original header/ciphertext. No key,
password or claimed `verified` bit belongs in a remote projection.

Do not add executable consumer codecs or publish the proposed version until
the format-review task resolves its findings. Current AES-GCM entry bytes and
readers remain unchanged. The future Coax API should transport caller-owned
bytes/status/progress; decoding a pointer supplies no Nostr identity or grant.
