/* Test-only public API adapter. Never linked into a product. */
#include <stddef.h>
#include <string.h>
#include "secp256k1.h"
#include "secp256k1_ecdh.h"
#include "secp256k1_extrakeys.h"
#include "secp256k1_schnorrsig.h"

void *oa_context(void) { return secp256k1_context_create(SECP256K1_CONTEXT_NONE); }
void oa_destroy(void *ctx) { secp256k1_context_destroy(ctx); }

static int raw_x(unsigned char *out, const unsigned char *x, const unsigned char *y, void *data) {
    (void)y; (void)data; memcpy(out, x, 32); return 1;
}
int oa_ecdh(void *ctx, const unsigned char *secret, const unsigned char *peer_x, unsigned char *out, int inject_default_hash) {
    unsigned char compressed[33]; secp256k1_pubkey peer;
    compressed[0] = 2; memcpy(compressed + 1, peer_x, 32);
    if (!secp256k1_ec_pubkey_parse(ctx, &peer, compressed, sizeof(compressed))) return 0;
    /* Default hashing exists only for the seeded wrong-oracle test. */
    return secp256k1_ecdh(ctx, out, &peer, secret, inject_default_hash ? NULL : raw_x, NULL);
}
int oa_point(void *ctx, const unsigned char *secret, const unsigned char *peer, size_t peer_len, unsigned char *out) {
    secp256k1_pubkey point; size_t n = 33;
    if (peer_len) {
        if (!secp256k1_ec_pubkey_parse(ctx, &point, peer, peer_len)) return 0;
        if (!secp256k1_ec_pubkey_tweak_mul(ctx, &point, secret)) return 0;
    } else if (!secp256k1_ec_pubkey_create(ctx, &point, secret)) return 0;
    return secp256k1_ec_pubkey_serialize(ctx, out, &n, &point, SECP256K1_EC_COMPRESSED) && n == 33;
}
int oa_verify(void *ctx, const unsigned char *msg, size_t n, const unsigned char *pk, const unsigned char *sig) {
    secp256k1_xonly_pubkey pubkey;
    if (!secp256k1_xonly_pubkey_parse(ctx, &pubkey, pk)) return 0;
    return secp256k1_schnorrsig_verify(ctx, sig, msg, n, &pubkey);
}
int oa_sign(void *ctx, const unsigned char *msg, size_t n, const unsigned char *secret, const unsigned char *aux, unsigned char *sig) {
    secp256k1_keypair key;
    secp256k1_schnorrsig_extraparams extra = SECP256K1_SCHNORRSIG_EXTRAPARAMS_INIT;
    extra.ndata = (void*)aux;
    if (!secp256k1_keypair_create(ctx, &key, secret)) return 0;
    return secp256k1_schnorrsig_sign_custom(ctx, sig, msg, n, &key, &extra);
}
