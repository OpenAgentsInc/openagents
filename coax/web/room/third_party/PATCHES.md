# Changes to the vendored sources

Each is small and marked in the source. Keep them when updating (VERSIONS).

1. `nghttp3/lib/nghttp3_stream.c`, `nghttp3_stream_write_settings`: under
   `NGHTTP3_COAX_WEBTRANSPORT` (build.sh defines it), a server that enables
   extended CONNECT and HTTP datagrams also sends the WebTransport settings
   Chrome's QUIC stack reads: `SETTINGS_ENABLE_WEBTRANSPORT` (0x2b603742, draft
   00 to 02) = 1 and `SETTINGS_WEBTRANSPORT_MAX_SESSIONS` (0xc671706a, draft 07)
   = 16. nghttp3 has no WebTransport of its own and no way to add a setting.
2. `ngtcp2/crypto/ngtcp2_crypto_minicrypto.c` is upstream's
   `crypto/picotls/picotls.c` with picotls's minicrypto binding in place of its
   OpenSSL binding (`ptls_openssl_*` to `ptls_minicrypto_*`), so the room links
   no system OpenSSL; and an ECB context is initialised only when it has an
   init (minicrypto's AES-ECB has none; calling it crashed the first Initial).
