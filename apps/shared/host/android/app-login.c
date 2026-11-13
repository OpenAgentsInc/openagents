/* Firm reason: 6 (Android's HTTPS, Keystore and files are Java APIs reached through JNI; the flow and its rules are Coax).
   The Android app's login (app-login.h, GitHub #13229): the effects of apps/shared/app-login-flow.coax
   on a thread of its own. Every Java call is checked; an exception is cleared and the request answers
   as unreachable (status 0) or the store as empty. Nothing here logs a token, code, email or key. */
#include "app-login.h"
#include "https.h"
#include "device-key.h"
#include "app-login-unit.h"

#include <android/log.h>
#include <errno.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/system_properties.h>
#include <time.h>
#include <unistd.h>

#define SAY(...) __android_log_print(ANDROID_LOG_INFO, "oa-login", __VA_ARGS__)
#define STORE_ALIAS "oa-app-login-store"
#define DEVICE_ALIAS "oa-app-login-device"
#define ANSWER_MAX (64 << 10)

static struct {
  JavaVM *vm; JNIEnv *env;
  char dir[512], origin[256];
  pthread_mutex_t lock; pthread_cond_t wake;
  int asked; char text[256]; int text_n;          /* what the screen asked, waiting */
  int state; bool busy; char words[400]; char who[160]; uint32_t gen;
  coax_t_AlFlow *flow;
  uint8_t answer[ANSWER_MAX];
} L = { .lock = PTHREAD_MUTEX_INITIALIZER, .wake = PTHREAD_COND_INITIALIZER };

static coax_v_t_AlFlow F(void) { return (coax_v_t_AlFlow){L.flow, 1}; }
static coax_string S(const char *s, int64_t n) { return (coax_string){s, n}; }
static coax_v_u8 V(void *p, int64_t n) { return (coax_v_u8){(uint8_t *)p, n}; }
static int64_t now_ms(void) { struct timespec t; clock_gettime(CLOCK_REALTIME, &t); return (int64_t)t.tv_sec * 1000 + t.tv_nsec / 1000000; }

/* ---- JNI pieces */

static bool threw(JNIEnv *e) { if (!(*e)->ExceptionCheck(e)) return false; (*e)->ExceptionClear(e); return true; }
static jclass cls(JNIEnv *e, const char *n) { jclass c = (*e)->FindClass(e, n); return threw(e) ? NULL : c; }
static jmethodID mid(JNIEnv *e, jclass c, const char *n, const char *s) { jmethodID m = c ? (*e)->GetMethodID(e, c, n, s) : NULL; return threw(e) ? NULL : m; }
static jmethodID smid(JNIEnv *e, jclass c, const char *n, const char *s) { jmethodID m = c ? (*e)->GetStaticMethodID(e, c, n, s) : NULL; return threw(e) ? NULL : m; }
static jstring jstr(JNIEnv *e, const char *s) { jstring j = (*e)->NewStringUTF(e, s); return threw(e) ? NULL : j; }
static jbyteArray jbytes(JNIEnv *e, const void *p, int n) {
  jbyteArray a = (*e)->NewByteArray(e, n);
  if (threw(e) || !a) return NULL;
  (*e)->SetByteArrayRegion(e, a, 0, n, (const jbyte *)p);
  return threw(e) ? NULL : a;
}

/* The login's exchange: on its thread, the answer in L.answer. */
static int http(const char *method, const char *url, const char *const *headers, int nheaders, const char *body, int body_n, int *answer_n) {
  return oa_https(L.env, method, url, headers, nheaders, body, body_n, L.answer, ANSWER_MAX, answer_n);
}

/* ---- the store: AES-GCM under the Keystore's key, iv || ciphertext, in a 0600 file */

static jobject store_key(JNIEnv *e) {
  jclass ks_c = cls(e, "java/security/KeyStore");
  jmethodID inst = smid(e, ks_c, "getInstance", "(Ljava/lang/String;)Ljava/security/KeyStore;");
  jmethodID load = mid(e, ks_c, "load", "(Ljava/security/KeyStore$LoadStoreParameter;)V");
  jmethodID get = mid(e, ks_c, "getKey", "(Ljava/lang/String;[C)Ljava/security/Key;");
  if (!inst || !load || !get) return NULL;
  jobject ks = (*e)->CallStaticObjectMethod(e, ks_c, inst, jstr(e, "AndroidKeyStore"));
  if (threw(e) || !ks) return NULL;
  (*e)->CallVoidMethod(e, ks, load, (jobject)NULL); if (threw(e)) return NULL;
  jobject key = (*e)->CallObjectMethod(e, ks, get, jstr(e, STORE_ALIAS), (jobject)NULL);
  if (threw(e)) key = NULL;
  if (key) return key;
  /* None yet: an AES-256 key for GCM, made in the Keystore, never exported; no prompt. */
  jclass kg_c = cls(e, "javax/crypto/KeyGenerator"), b_c = cls(e, "android/security/keystore/KeyGenParameterSpec$Builder"), s_c = cls(e, "java/lang/String");
  jmethodID kg_inst = smid(e, kg_c, "getInstance", "(Ljava/lang/String;Ljava/lang/String;)Ljavax/crypto/KeyGenerator;");
  jmethodID kg_init = mid(e, kg_c, "init", "(Ljava/security/spec/AlgorithmParameterSpec;)V"), kg_make = mid(e, kg_c, "generateKey", "()Ljavax/crypto/SecretKey;");
  jmethodID b_new = mid(e, b_c, "<init>", "(Ljava/lang/String;I)V");
  jmethodID b_modes = mid(e, b_c, "setBlockModes", "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_pads = mid(e, b_c, "setEncryptionPaddings", "([Ljava/lang/String;)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_size = mid(e, b_c, "setKeySize", "(I)Landroid/security/keystore/KeyGenParameterSpec$Builder;");
  jmethodID b_build = mid(e, b_c, "build", "()Landroid/security/keystore/KeyGenParameterSpec;");
  if (!kg_inst || !kg_init || !kg_make || !b_new || !b_modes || !b_pads || !b_size || !b_build || !s_c) return NULL;
  jobject kg = (*e)->CallStaticObjectMethod(e, kg_c, kg_inst, jstr(e, "AES"), jstr(e, "AndroidKeyStore"));
  if (threw(e) || !kg) return NULL;
  jobject b = (*e)->NewObject(e, b_c, b_new, jstr(e, STORE_ALIAS), (jint)3);   /* ENCRYPT | DECRYPT */
  if (threw(e) || !b) return NULL;
  jobjectArray gcm = (*e)->NewObjectArray(e, 1, s_c, jstr(e, "GCM")), none = (*e)->NewObjectArray(e, 1, s_c, jstr(e, "NoPadding"));
  if (threw(e)) return NULL;
  (*e)->CallObjectMethod(e, b, b_modes, gcm); (*e)->CallObjectMethod(e, b, b_pads, none); (*e)->CallObjectMethod(e, b, b_size, (jint)256);
  if (threw(e)) return NULL;
  jobject spec = (*e)->CallObjectMethod(e, b, b_build);
  if (threw(e) || !spec) return NULL;
  (*e)->CallVoidMethod(e, kg, kg_init, spec); if (threw(e)) return NULL;
  key = (*e)->CallObjectMethod(e, kg, kg_make);
  return threw(e) ? NULL : key;
}

/* AES-GCM: encrypt plain into out (iv || ciphertext) or decrypt (out gets the plain text). Its length, or -1. */
static int seal(bool encrypt, const uint8_t *in, int n, uint8_t *out, int cap) {
  JNIEnv *e = L.env; int result = -1;
  if ((*e)->PushLocalFrame(e, 32) != 0) { threw(e); return -1; }
  jobject key = store_key(e);
  jclass c_c = cls(e, "javax/crypto/Cipher"), spec_c = cls(e, "javax/crypto/spec/GCMParameterSpec");
  jmethodID inst = smid(e, c_c, "getInstance", "(Ljava/lang/String;)Ljavax/crypto/Cipher;");
  jmethodID init_enc = mid(e, c_c, "init", "(ILjava/security/Key;)V"), init_dec = mid(e, c_c, "init", "(ILjava/security/Key;Ljava/security/spec/AlgorithmParameterSpec;)V");
  jmethodID iv_of = mid(e, c_c, "getIV", "()[B"), final = mid(e, c_c, "doFinal", "([B)[B"), spec_new = mid(e, spec_c, "<init>", "(I[B)V");
  if (!key || !inst || !init_enc || !init_dec || !iv_of || !final || !spec_new) goto out;
  jobject cipher = (*e)->CallStaticObjectMethod(e, c_c, inst, jstr(e, "AES/GCM/NoPadding"));
  if (threw(e) || !cipher) goto out;
  if (encrypt) {
    (*e)->CallVoidMethod(e, cipher, init_enc, (jint)1, key); if (threw(e)) goto out;
    jbyteArray iv = (*e)->CallObjectMethod(e, cipher, iv_of); if (threw(e) || !iv || (*e)->GetArrayLength(e, iv) != 12) goto out;
    jbyteArray ct = (*e)->CallObjectMethod(e, cipher, final, jbytes(e, in, n)); if (threw(e) || !ct) goto out;
    int m = (*e)->GetArrayLength(e, ct);
    if (12 + m > cap) goto out;
    (*e)->GetByteArrayRegion(e, iv, 0, 12, (jbyte *)out); (*e)->GetByteArrayRegion(e, ct, 0, m, (jbyte *)out + 12);
    result = 12 + m;
  } else {
    if (n < 12 + 16) goto out;
    jobject spec = (*e)->NewObject(e, spec_c, spec_new, (jint)128, jbytes(e, in, 12)); if (threw(e) || !spec) goto out;
    (*e)->CallVoidMethod(e, cipher, init_dec, (jint)2, key, spec); if (threw(e)) goto out;
    jbyteArray pt = (*e)->CallObjectMethod(e, cipher, final, jbytes(e, in + 12, n - 12)); if (threw(e) || !pt) goto out;
    int m = (*e)->GetArrayLength(e, pt);
    if (m > cap) goto out;
    (*e)->GetByteArrayRegion(e, pt, 0, m, (jbyte *)out);
    result = m;
  }
out:
  (*e)->PopLocalFrame(e, NULL);
  return result;
}

static void path_of(const char *name, char *out, size_t cap) { snprintf(out, cap, "%s/%s", L.dir, name); }
/* The named file's plain text into out: its length, or -1 when there is none (or it won't open). */
static int load(const char *name, uint8_t *out, int cap) {
  char p[640]; uint8_t sealed[2048]; path_of(name, p, sizeof p);
  int fd = open(p, O_RDONLY | O_CLOEXEC);
  if (fd < 0) return -1;
  ssize_t n = read(fd, sealed, sizeof sealed);
  close(fd);
  return n > 0 ? seal(false, sealed, (int)n, out, cap) : -1;
}
static bool keep(const char *name, const uint8_t *plain, int n) {
  char p[640], tmp[660]; uint8_t sealed[2048]; path_of(name, p, sizeof p); snprintf(tmp, sizeof tmp, "%s.new", p);
  int m = seal(true, plain, n, sealed, sizeof sealed);
  if (m <= 0) return false;
  int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600);
  if (fd < 0) return false;
  bool ok = write(fd, sealed, (size_t)m) == m;
  ok = fsync(fd) == 0 && ok;
  close(fd);
  return ok && rename(tmp, p) == 0;
}
static void drop(const char *name) { char p[640]; path_of(name, p, sizeof p); unlink(p); }

/* ---- the request the flow wrote, sent */

static void hex(const uint8_t *b, int n, char *out) { static const char d[] = "0123456789abcdef"; for (int i = 0; i < n; i++) { out[2 * i] = d[b[i] >> 4]; out[2 * i + 1] = d[b[i] & 15]; } out[2 * n] = 0; }

/* The phone's Nostr key: kept (encrypted), else made now. false when the Keystore gave nothing. */
static bool nostr_key(uint8_t key[32]) {
  if (load("nostr-key", key, 32) == 32) return true;
  uint8_t aux[32] = {0}, nonce[16] = {0}; char out[6000];
  for (int tries = 0; tries < 4; tries++) {
    arc4random_buf(key, 32);
    /* A key secp256k1 refuses (about 2^-128 of them) signs nothing: make another. */
    if (alu_sign(V(key, 32), V(aux, 32), V(nonce, 16), 1, S("https://a/", 10), S("", 0), V(out, sizeof out)) > 0 && keep("nostr-key", key, 32)) return true;
  }
  memset(key, 0, 32);
  return false;
}

static int send_flow(int *answer_n) {
  char path[128], url[512], body[600], auth[6000], name[96], devkey[140], proof[200], access[128], hash[72];
  int pn = (int)alu_path(F(), V(path, sizeof path - 1)); path[pn] = 0;
  int bn = (int)alu_body(F(), V(body, sizeof body - 1)); body[bn] = 0;
  snprintf(url, sizeof url, "%s%s", L.origin, path);
  const char *h[16]; int nh = 0;
  h[nh * 2] = "Accept"; h[nh * 2 + 1] = "application/json"; nh++;
  if (alu_post(F())) { h[nh * 2] = "Content-Type"; h[nh * 2 + 1] = "application/json"; nh++; }
  int64_t auth_kind = alu_auth(F());
  if (auth_kind == 1) {
    int an = (int)alu_access(F(), V(access, sizeof access - 1)); access[an] = 0;
    snprintf(auth, sizeof auth, "Bearer %s", access);
    h[nh * 2] = "Authorization"; h[nh * 2 + 1] = auth; nh++;
  } else if (auth_kind == 2) {
    uint8_t key[32], aux[32], nonce[16];
    if (!nostr_key(key)) return 0;
    arc4random_buf(aux, sizeof aux); arc4random_buf(nonce, sizeof nonce);
    int64_t n = alu_sign(V(key, 32), V(aux, 32), V(nonce, 16), now_ms() / 1000, S(url, (int64_t)strlen(url)), S(body, bn), V(auth, sizeof auth - 1));
    memset(key, 0, sizeof key); memset(aux, 0, sizeof aux);
    if (n <= 0) return 0;
    auth[n] = 0;
    h[nh * 2] = "Authorization"; h[nh * 2 + 1] = auth; nh++;
  }
  if (alu_device(F())) {
    uint8_t pub[OA_DEVICE_KEY_PUBLIC];
    oa_device_store where = oa_device_key_current(L.env, DEVICE_ALIAS, pub);
    if (where != OA_DEVICE_STORE_NONE) {
      char model[PROP_VALUE_MAX] = ""; __system_property_get("ro.product.model", model);
      int nn = (int)alu_device_name(S(model, (int64_t)strlen(model)), V(name, sizeof name - 1)); name[nn] = 0;
      hex(pub, OA_DEVICE_KEY_PUBLIC, devkey);
      h[nh * 2] = "OA-Device-Key"; h[nh * 2 + 1] = devkey; nh++;
      h[nh * 2] = "OA-Device-Platform"; h[nh * 2 + 1] = "android"; nh++;
      h[nh * 2] = "OA-Device-Store"; h[nh * 2 + 1] = oa_device_store_name(where); nh++;
      h[nh * 2] = "OA-Device-Name"; h[nh * 2 + 1] = name; nh++;
    }
  }
  int64_t ms = alu_proof_ms(F());
  if (ms > 0) {
    int hn = (int)alu_proof_hash(F(), V(hash, sizeof hash - 1)); hash[hn] = 0;
    if (oa_device_key_proof(L.env, DEVICE_ALIAS, "POST", url, ms, hash, proof, sizeof proof) > 0) { h[nh * 2] = "OA-Device-Proof"; h[nh * 2 + 1] = proof; nh++; }
  }
  return http(alu_post(F()) ? "POST" : "GET", url, h, nh, alu_post(F()) ? body : NULL, bn, answer_n);
}

/* Runs the flow to its end, then keeps (or deletes) the record and forgets the device key if told. */
static void run(bool started) {
  while (started) {
    int n = 0, status = send_flow(&n);
    char path[96]; int pn = (int)alu_path(F(), V(path, sizeof path - 1)); path[pn] = 0;
    SAY("%s %s: %d", alu_post(F()) ? "POST" : "GET", path, status);   /* the path and status only */
    started = alu_answer(F(), status, S((const char *)L.answer, n), now_ms());
  }
  if (alu_keep(F())) {
    uint8_t rec[600]; int64_t n = alu_record(F(), V(rec, sizeof rec));
    if (n > 0) keep("login", rec, (int)n); else drop("login");
    memset(rec, 0, sizeof rec);
  }
  if (alu_forget_device(F())) oa_device_key_forget(L.env, DEVICE_ALIAS);
}

static void publish(bool busy) {
  char words[400], who[160];
  int wn = (int)alu_words(F(), V(words, sizeof words - 1)); words[wn] = 0;
  int hn = (int)alu_who_text(F(), V(who, sizeof who - 1)); who[hn] = 0;
  pthread_mutex_lock(&L.lock);
  L.state = (int)alu_state(F()); L.busy = busy;
  memcpy(L.words, words, sizeof words); memcpy(L.who, who, sizeof who);
  L.gen++;
  pthread_mutex_unlock(&L.lock);
}

static void *main_loop(void *unused) {
  (void)unused;
  if ((*L.vm)->AttachCurrentThread(L.vm, &L.env, NULL) != JNI_OK) return NULL;
  uint8_t rec[600]; int n = load("login", rec, sizeof rec - 1);
  alu_open(F(), S((const char *)rec, n > 0 ? n : 0));
  memset(rec, 0, sizeof rec);
  publish(true);
  run(alu_who(F(), now_ms()));
  publish(false);
  for (;;) {
    pthread_mutex_lock(&L.lock);
    while (!L.asked) pthread_cond_wait(&L.wake, &L.lock);
    int kind = L.asked; char text[256]; int tn = L.text_n; memcpy(text, L.text, sizeof text);
    L.asked = 0; memset(L.text, 0, sizeof L.text);
    pthread_mutex_unlock(&L.lock);
    publish(true);
    coax_string t = S(text, tn);
    if (kind == OA_LOGIN_EMAIL) run(alu_email(F(), t));
    else if (kind == OA_LOGIN_CODE) run(alu_code(F(), t));
    else if (kind == OA_LOGIN_KEY) run(alu_key(F()));
    else if (kind == OA_LOGIN_LOGOUT) run(alu_logout(F()));
    else if (kind == OA_LOGIN_BACK) alu_back(F());
    else if (kind == OA_LOGIN_WHO) run(alu_who(F(), now_ms()));
    memset(text, 0, sizeof text);
    publish(false);
  }
  return NULL;
}

bool oa_app_login_start(JavaVM *vm, const char *dir, const char *origin) {
  if (L.flow || !vm || !dir || !origin) return false;
  L.flow = calloc(1, (size_t)alu_size());
  if (!L.flow) return false;
  L.vm = vm;
  snprintf(L.dir, sizeof L.dir, "%s", dir); snprintf(L.origin, sizeof L.origin, "%s", origin);
  mkdir(L.dir, 0700);
  L.busy = true;
  pthread_attr_t a; pthread_t t;
  pthread_attr_init(&a);
  /* Signing keeps secp256k1's work on the stack: a deep one. */
  if (pthread_attr_setstacksize(&a, (size_t)16 << 20) != 0 || pthread_create(&t, &a, main_loop, NULL) != 0) { SAY("no thread for the login"); return false; }
  pthread_detach(t);
  return true;
}

void oa_app_login_ask(int kind, const char *text, int length) {
  pthread_mutex_lock(&L.lock);
  if (!L.busy && !L.asked) {
    L.asked = kind;
    L.text_n = length < 0 ? 0 : length > (int)sizeof L.text ? (int)sizeof L.text : length;
    if (L.text_n) memcpy(L.text, text, (size_t)L.text_n);
    L.busy = true; L.gen++;
    pthread_cond_signal(&L.wake);
  }
  pthread_mutex_unlock(&L.lock);
}

uint32_t oa_app_login_status(int *state, bool *busy, char *words, int words_cap, char *who, int who_cap) {
  pthread_mutex_lock(&L.lock);
  *state = L.state; *busy = L.busy;
  snprintf(words, (size_t)words_cap, "%s", L.words); snprintf(who, (size_t)who_cap, "%s", L.who);
  uint32_t g = L.gen;
  pthread_mutex_unlock(&L.lock);
  return g;
}
