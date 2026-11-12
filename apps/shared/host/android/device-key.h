/* The device key that ties an app's login to this Android phone (docs/auth/sessions-and-tokens.md
   "Native: refresh tokens bound to a device key", #13216): a P-256 key in the Android Keystore
   that can't be exported, in StrongBox when the phone has it, else in the phone's trusted
   environment. It is made silently the first time it is asked for (no lock screen prompt: the
   key asks for no user authentication), and one key lasts one login.

   The server's side is coax/web/api/auth-device.c; the proof's bytes are
   coax/web/api/auth-device-crypto.coax's. The Keystore has no C interface, so device-key.c calls
   java.security and android.security.keystore through JNI. Every call takes the thread's JNIEnv. */
#pragma once
#include <jni.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

enum { OA_DEVICE_KEY_PUBLIC = 65, OA_DEVICE_KEY_SIGNATURE_MAX = 72 };
/* Where the key lives, as the Keystore reports it: what the app sends in OA-Device-Store. */
typedef enum { OA_DEVICE_STORE_NONE = 0, OA_DEVICE_STORE_STRONGBOX = 1, OA_DEVICE_STORE_TEE = 2, OA_DEVICE_STORE_SOFTWARE = 3 } oa_device_store;

/* This login's key under alias: the stored one, or a new one made now. public_key gets
   04 || x || y. Where it lives, or OA_DEVICE_STORE_NONE when the Keystore gave no key. */
oa_device_store oa_device_key_current(JNIEnv *env, const char *alias, uint8_t public_key[OA_DEVICE_KEY_PUBLIC]);
/* The stored key only, never a new one (a refresh signs with the login's own key). */
oa_device_store oa_device_key_existing(JNIEnv *env, const char *alias, uint8_t public_key[OA_DEVICE_KEY_PUBLIC]);
/* ES256 over message (SHA-256 inside), DER, into signature: its length, or 0. */
size_t oa_device_key_sign(JNIEnv *env, const char *alias, const uint8_t *message, size_t n, uint8_t signature[OA_DEVICE_KEY_SIGNATURE_MAX]);
/* Forgets the key (logout, or the server ended the login): the next login makes a new one. */
void oa_device_key_forget(JNIEnv *env, const char *alias);
/* "strongbox", "tee" or "software": the OA-Device-Store header's value. */
const char *oa_device_store_name(oa_device_store store);
/* The OA-Device-Proof header's value for a request, "v1 <ms> <signature hex>", into out
   (at least 170 bytes): its length, or 0. token_sha256_hex: SHA-256 of the refresh token, 64 hex. */
size_t oa_device_key_proof(JNIEnv *env, const char *alias, const char *method, const char *url, int64_t ms, const char *token_sha256_hex, char *out, size_t cap);
/* Makes a key, signs with it, checks the signature with the platform's own verifier, reopens it,
   forgets it. true when all of it held; what it found goes to logcat under "oa-device-key". */
bool oa_device_key_selftest(JNIEnv *env);
