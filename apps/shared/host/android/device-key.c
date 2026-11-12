/* The Android Keystore device key (device-key.h, #13216), through JNI: java.security's KeyStore,
   KeyPairGenerator and Signature, and android.security.keystore's KeyGenParameterSpec and KeyInfo.
   Every Java call is checked; an exception is cleared and the call answers "no key". Nothing here
   logs a key or a signature. */
#include "device-key.h"

#include <android/log.h>
#include <stdio.h>
#include <string.h>

#define LOG(...) __android_log_print(ANDROID_LOG_INFO, "oa-device-key", __VA_ARGS__)

/* True when the last call threw: the exception is cleared. */
static bool threw(JNIEnv *e) {
  if (!(*e)->ExceptionCheck(e)) return false;
  (*e)->ExceptionClear(e);
  return true;
}
static jclass cls(JNIEnv *e, const char *name) { jclass c = (*e)->FindClass(e, name); return threw(e) ? NULL : c; }
static jmethodID mid(JNIEnv *e, jclass c, const char *name, const char *sig) { jmethodID m = c ? (*e)->GetMethodID(e, c, name, sig) : NULL; return threw(e) ? NULL : m; }
static jmethodID smid(JNIEnv *e, jclass c, const char *name, const char *sig) { jmethodID m = c ? (*e)->GetStaticMethodID(e, c, name, sig) : NULL; return threw(e) ? NULL : m; }
static jstring str(JNIEnv *e, const char *s) { jstring j = (*e)->NewStringUTF(e, s); return threw(e) ? NULL : j; }

/* KeyStore.getInstance("AndroidKeyStore"), loaded. */
static jobject keystore(JNIEnv *e) {
  jclass c = cls(e, "java/security/KeyStore");
  jmethodID get = smid(e, c, "getInstance", "(Ljava/lang/String;)Ljava/security/KeyStore;");
  jmethodID load = mid(e, c, "load", "(Ljava/security/KeyStore$LoadStoreParameter;)V");
  if (!get || !load) return NULL;
  jobject ks = (*e)->CallStaticObjectMethod(e, c, get, str(e, "AndroidKeyStore"));
  if (threw(e) || !ks) return NULL;
  (*e)->CallVoidMethod(e, ks, load, (jobject)NULL);
  return threw(e) ? NULL : ks;
}

/* Makes the key under alias. strongbox: ask for StrongBox (it throws when the phone has none). */
static bool generate(JNIEnv *e, const char *alias, bool strongbox) {
  jclass gen_c = cls(e, "java/security/KeyPairGenerator");
  jclass b_c = cls(e, "android/security/keystore/KeyGenParameterSpec$Builder");
  jclass ec_c = cls(e, "java/security/spec/ECGenParameterSpec");
  jclass str_c = cls(e, "java/lang/String");
  jmethodID get = smid(e, gen_c, "getInstance", "(Ljava/lang/String;Ljava/lang/String;)Ljava/security/KeyPairGenerator;");
  jmethodID init = mid(e, gen_c, "initialize", "(Ljava/security/spec/AlgorithmParameterSpec;)V");
  jmethodID make = mid(e, gen_c, "generateKeyPair", "()Ljava/security/KeyPair;");
  jmethodID b_new = mid(e, b_c, "<init>", "(Ljava/lang/String;I)V");
  jmethodID b_curve = mid(e, b_c, "setAlgorithmParameterSpec", "(Ljava/security/spec/AlgorithmParameterSpec;)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_digests = mid(e, b_c, "setDigests", "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_strong = mid(e, b_c, "setIsStrongBoxBacked", "(Z)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_build = mid(e, b_c, "build", "()Landroid/security/keystore/KeyGenParameterSpec;");
  jmethodID ec_new = mid(e, ec_c, "<init>", "(Ljava/lang/String;)V");
  if (!get || !init || !make || !b_new || !b_curve || !b_digests || !b_strong || !b_build || !ec_new || !str_c) return false;
  jobject g = (*e)->CallStaticObjectMethod(e, gen_c, get, str(e, "EC"), str(e, "AndroidKeyStore"));
  if (threw(e) || !g) return false;
  /* PURPOSE_SIGN is 4. No setUserAuthenticationRequired: the key signs with no prompt. */
  jobject b = (*e)->NewObject(e, b_c, b_new, str(e, alias), (jint)4);
  if (threw(e) || !b) return false;
  jobject curve = (*e)->NewObject(e, ec_c, ec_new, str(e, "secp256r1"));
  if (threw(e) || !curve) return false;
  (*e)->CallObjectMethod(e, b, b_curve, curve);
  if (threw(e)) return false;
  jobjectArray digests = (*e)->NewObjectArray(e, 1, str_c, str(e, "SHA-256"));
  if (threw(e) || !digests) return false;
  (*e)->CallObjectMethod(e, b, b_digests, digests);
  if (threw(e)) return false;
  (*e)->CallObjectMethod(e, b, b_strong, (jboolean)(strongbox ? JNI_TRUE : JNI_FALSE));
  if (threw(e)) return false;
  jobject spec = (*e)->CallObjectMethod(e, b, b_build);
  if (threw(e) || !spec) return false;
  (*e)->CallVoidMethod(e, g, init, spec);
  if (threw(e)) return false;
  (*e)->CallObjectMethod(e, g, make);   /* StrongBoxUnavailableException lands here on a phone with none */
  return !threw(e);
}

/* The private key's handle under alias, or NULL. */
static jobject private_key(JNIEnv *e, jobject ks, const char *alias) {
  jclass c = cls(e, "java/security/KeyStore");
  jmethodID get = mid(e, c, "getKey", "(Ljava/lang/String;[C)Ljava/security/Key;");
  if (!get) return NULL;
  jobject k = (*e)->CallObjectMethod(e, ks, get, str(e, alias), (jobject)NULL);
  return threw(e) ? NULL : k;
}
/* The public key's handle under alias (from its certificate), or NULL. */
static jobject public_handle(JNIEnv *e, jobject ks, const char *alias) {
  jclass c = cls(e, "java/security/KeyStore");
  jclass cert_c = cls(e, "java/security/cert/Certificate");
  jmethodID get = mid(e, c, "getCertificate", "(Ljava/lang/String;)Ljava/security/cert/Certificate;");
  jmethodID pub = mid(e, cert_c, "getPublicKey", "()Ljava/security/PublicKey;");
  if (!get || !pub) return NULL;
  jobject cert = (*e)->CallObjectMethod(e, ks, get, str(e, alias));
  if (threw(e) || !cert) return NULL;
  jobject k = (*e)->CallObjectMethod(e, cert, pub);
  return threw(e) ? NULL : k;
}
/* 04 || x || y: the last 65 bytes of a P-256 key's X.509 encoding (91 bytes). */
static bool public_bytes(JNIEnv *e, jobject pub, uint8_t out[OA_DEVICE_KEY_PUBLIC]) {
  jclass c = cls(e, "java/security/Key");
  jmethodID enc = mid(e, c, "getEncoded", "()[B");
  if (!enc || !pub) return false;
  jbyteArray a = (jbyteArray)(*e)->CallObjectMethod(e, pub, enc);
  if (threw(e) || !a) return false;
  jsize n = (*e)->GetArrayLength(e, a);
  if (n != 91) return false;
  (*e)->GetByteArrayRegion(e, a, n - OA_DEVICE_KEY_PUBLIC, OA_DEVICE_KEY_PUBLIC, (jbyte *)out);
  return !threw(e) && out[0] == 4;
}
/* Where the Keystore says the key is: KeyInfo.getSecurityLevel on Android 12 and up
   (2 StrongBox, 1 trusted environment, 0 and anything else software), isInsideSecureHardware before. */
static oa_device_store where(JNIEnv *e, jobject priv) {
  jclass key_c = cls(e, "java/security/Key");
  jclass kf_c = cls(e, "java/security/KeyFactory");
  jclass info_c = cls(e, "android/security/keystore/KeyInfo");
  jmethodID alg = mid(e, key_c, "getAlgorithm", "()Ljava/lang/String;");
  jmethodID kf_get = smid(e, kf_c, "getInstance", "(Ljava/lang/String;Ljava/lang/String;)Ljava/security/KeyFactory;");
  jmethodID kf_spec = mid(e, kf_c, "getKeySpec", "(Ljava/security/Key;Ljava/lang/Class;)Ljava/security/spec/KeySpec;");
  if (!alg || !kf_get || !kf_spec || !info_c) return OA_DEVICE_STORE_SOFTWARE;
  jobject name = (*e)->CallObjectMethod(e, priv, alg);
  if (threw(e) || !name) return OA_DEVICE_STORE_SOFTWARE;
  jobject kf = (*e)->CallStaticObjectMethod(e, kf_c, kf_get, name, str(e, "AndroidKeyStore"));
  if (threw(e) || !kf) return OA_DEVICE_STORE_SOFTWARE;
  jobject info = (*e)->CallObjectMethod(e, kf, kf_spec, priv, info_c);
  if (threw(e) || !info) return OA_DEVICE_STORE_SOFTWARE;
  jmethodID level = mid(e, info_c, "getSecurityLevel", "()I");   /* Android 12 */
  if (level) {
    jint l = (*e)->CallIntMethod(e, info, level);
    if (!threw(e)) return l == 2 ? OA_DEVICE_STORE_STRONGBOX : l == 1 ? OA_DEVICE_STORE_TEE : OA_DEVICE_STORE_SOFTWARE;
  }
  jmethodID inside = mid(e, info_c, "isInsideSecureHardware", "()Z");
  if (!inside) return OA_DEVICE_STORE_SOFTWARE;
  jboolean in = (*e)->CallBooleanMethod(e, info, inside);
  return !threw(e) && in ? OA_DEVICE_STORE_TEE : OA_DEVICE_STORE_SOFTWARE;
}

static oa_device_store open_key(JNIEnv *e, const char *alias, uint8_t public_key[OA_DEVICE_KEY_PUBLIC], bool make) {
  if ((*e)->PushLocalFrame(e, 64) != 0) { threw(e); return OA_DEVICE_STORE_NONE; }
  oa_device_store found = OA_DEVICE_STORE_NONE;
  jobject ks = keystore(e);
  jobject priv = ks ? private_key(e, ks, alias) : NULL;
  /* StrongBox first; a phone with none throws, and the key is made in its trusted environment. */
  if (ks && !priv && make && (generate(e, alias, true) || generate(e, alias, false))) priv = private_key(e, ks, alias);
  if (priv && public_bytes(e, public_handle(e, ks, alias), public_key)) found = where(e, priv);
  (*e)->PopLocalFrame(e, NULL);
  return found;
}
oa_device_store oa_device_key_current(JNIEnv *e, const char *alias, uint8_t public_key[OA_DEVICE_KEY_PUBLIC]) { return open_key(e, alias, public_key, true); }
oa_device_store oa_device_key_existing(JNIEnv *e, const char *alias, uint8_t public_key[OA_DEVICE_KEY_PUBLIC]) { return open_key(e, alias, public_key, false); }

/* Signature "SHA256withECDSA": sign with the private key (verify false), or check a signature
   with the public one. Signing: the DER length, or 0. Verifying: 1 when it holds, else 0. */
static size_t ecdsa(JNIEnv *e, const char *alias, const uint8_t *message, size_t n, uint8_t *signature, size_t sig_n, bool verify) {
  if ((*e)->PushLocalFrame(e, 64) != 0) { threw(e); return 0; }
  size_t out = 0;
  jclass c = cls(e, "java/security/Signature");
  jmethodID get = smid(e, c, "getInstance", "(Ljava/lang/String;)Ljava/security/Signature;");
  jmethodID init_sign = mid(e, c, "initSign", "(Ljava/security/PrivateKey;)V");
  jmethodID init_verify = mid(e, c, "initVerify", "(Ljava/security/PublicKey;)V");
  jmethodID update = mid(e, c, "update", "([B)V");
  jmethodID sign = mid(e, c, "sign", "()[B");
  jmethodID check = mid(e, c, "verify", "([B)Z");
  jobject ks = keystore(e);
  jobject key = !ks ? NULL : verify ? public_handle(e, ks, alias) : private_key(e, ks, alias);
  jobject s = get ? (*e)->CallStaticObjectMethod(e, c, get, str(e, "SHA256withECDSA")) : NULL;
  if (threw(e) || !s || !key || !init_sign || !init_verify || !update || !sign || !check) goto done;
  (*e)->CallVoidMethod(e, s, verify ? init_verify : init_sign, key);
  if (threw(e)) goto done;
  jbyteArray m = (*e)->NewByteArray(e, (jsize)n);
  if (threw(e) || !m) goto done;
  (*e)->SetByteArrayRegion(e, m, 0, (jsize)n, (const jbyte *)message);
  (*e)->CallVoidMethod(e, s, update, m);
  if (threw(e)) goto done;
  if (verify) {
    jbyteArray g = (*e)->NewByteArray(e, (jsize)sig_n);
    if (threw(e) || !g) goto done;
    (*e)->SetByteArrayRegion(e, g, 0, (jsize)sig_n, (const jbyte *)signature);
    jboolean ok = (*e)->CallBooleanMethod(e, s, check, g);
    out = !threw(e) && ok ? 1 : 0;
  } else {
    jbyteArray der = (jbyteArray)(*e)->CallObjectMethod(e, s, sign);
    if (threw(e) || !der) goto done;
    jsize len = (*e)->GetArrayLength(e, der);
    if (len < 8 || (size_t)len > sig_n) goto done;
    (*e)->GetByteArrayRegion(e, der, 0, len, (jbyte *)signature);
    out = threw(e) ? 0 : (size_t)len;
  }
done:
  (*e)->PopLocalFrame(e, NULL);
  return out;
}
size_t oa_device_key_sign(JNIEnv *e, const char *alias, const uint8_t *message, size_t n, uint8_t signature[OA_DEVICE_KEY_SIGNATURE_MAX]) {
  return ecdsa(e, alias, message, n, signature, OA_DEVICE_KEY_SIGNATURE_MAX, false);
}

void oa_device_key_forget(JNIEnv *e, const char *alias) {
  if ((*e)->PushLocalFrame(e, 16) != 0) { threw(e); return; }
  jclass c = cls(e, "java/security/KeyStore");
  jmethodID del = mid(e, c, "deleteEntry", "(Ljava/lang/String;)V");
  jobject ks = keystore(e);
  if (ks && del) { (*e)->CallVoidMethod(e, ks, del, str(e, alias)); threw(e); }
  (*e)->PopLocalFrame(e, NULL);
}

const char *oa_device_store_name(oa_device_store store) {
  return store == OA_DEVICE_STORE_STRONGBOX ? "strongbox" : store == OA_DEVICE_STORE_TEE ? "tee" : "software";
}

size_t oa_device_key_proof(JNIEnv *e, const char *alias, const char *method, const char *url, int64_t ms, const char *token_sha256_hex, char *out, size_t cap) {
  char message[700];
  int n = snprintf(message, sizeof message, "OA-Device-Proof-v1\n%s\n%s\n%lld\n%s", method, url, (long long)ms, token_sha256_hex);
  if (n <= 0 || (size_t)n >= sizeof message || strlen(token_sha256_hex) != 64 || ms <= 0) return 0;
  uint8_t der[OA_DEVICE_KEY_SIGNATURE_MAX];
  size_t len = oa_device_key_sign(e, alias, (const uint8_t *)message, (size_t)n, der);
  if (!len) return 0;
  int at = snprintf(out, cap, "v1 %lld ", (long long)ms);
  if (at <= 0 || (size_t)at + 2 * len + 1 > cap) return 0;
  static const char digits[] = "0123456789abcdef";
  for (size_t i = 0; i < len; i++) { out[at++] = digits[der[i] >> 4]; out[at++] = digits[der[i] & 15]; }
  out[at] = 0;
  return (size_t)at;
}

bool oa_device_key_selftest(JNIEnv *e) {
  const char *alias = "oa-device-key-selftest";
  oa_device_key_forget(e, alias);
  uint8_t pub[OA_DEVICE_KEY_PUBLIC], again[OA_DEVICE_KEY_PUBLIC], none[OA_DEVICE_KEY_PUBLIC];
  if (oa_device_key_existing(e, alias, none) != OA_DEVICE_STORE_NONE) { LOG("selftest: a key exists before one was made"); return false; }
  oa_device_store store = oa_device_key_current(e, alias, pub);
  if (store == OA_DEVICE_STORE_NONE) { LOG("selftest: no key could be made"); return false; }
  char proof[200];
  static const char hash[] = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
  size_t pn = oa_device_key_proof(e, alias, "POST", "https://openagents.com/api/auth/refresh", 1800000000000LL, hash, proof, sizeof proof);
  if (!pn || strncmp(proof, "v1 1800000000000 ", 17)) { LOG("selftest: no proof"); return false; }
  static const char message[] = "OA-Device-Proof-v1\nPOST\nhttps://openagents.com/api/auth/refresh\n1800000000000\n9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
  uint8_t der[OA_DEVICE_KEY_SIGNATURE_MAX];
  size_t len = oa_device_key_sign(e, alias, (const uint8_t *)message, sizeof message - 1, der);
  if (!len || ecdsa(e, alias, (const uint8_t *)message, sizeof message - 1, der, len, true) != 1) { LOG("selftest: the signature doesn't verify"); return false; }
  if (ecdsa(e, alias, (const uint8_t *)message, sizeof message - 2, der, len, true) != 0) { LOG("selftest: the signature verifies for another message"); return false; }
  if (oa_device_key_existing(e, alias, again) != store || memcmp(pub, again, sizeof pub)) { LOG("selftest: the key didn't reopen as itself"); return false; }
  oa_device_key_forget(e, alias);
  if (oa_device_key_existing(e, alias, none) != OA_DEVICE_STORE_NONE) { LOG("selftest: forget left the key"); return false; }
  LOG("selftest: ok, key in %s, signature %zu bytes", oa_device_store_name(store), len);
  return true;
}
