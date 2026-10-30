// The admin's signer, on the admin's own machine: makes a NIP-98 HTTP auth
// event (kind 27235, tags u and method, and payload for a body) signed with a
// Nostr key kept in a file (64 hex digits, chmod 600), for oa-api's admin
// routes (admin.c). The key never leaves this program; it prints only the
// public key or the signed event, base64. BIP-340 Schnorr via libsecp256k1
// for now, as the server verifies it (nostr.c).
//
//   sign --new FILE                     a new key in FILE; prints its public key
//   sign --key FILE --pubkey            the key's public key (hex)
//   sign --key FILE METHOD URL [BODY]   the signed event for METHOD URL (and BODY's file), base64
#include <openssl/evp.h>
#include <openssl/sha.h>
#include <secp256k1.h>
#include <secp256k1_extrakeys.h>
#include <secp256k1_schnorrsig.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static void die(const char *why) { fprintf(stderr, "sign: %s\n", why); exit(1); }
static void random_bytes(unsigned char *out, size_t n) {
  int fd = open("/dev/urandom", O_RDONLY);
  if (fd < 0 || read(fd, out, n) != (ssize_t)n) die("no randomness");
  close(fd);
}
static void hex(const unsigned char *in, size_t n, char *out) { for (size_t i = 0; i < n; i++) snprintf(out + 2 * i, 3, "%02x", in[i]); }
static int hexv(char c) { return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1; }

static void read_key(const char *path, unsigned char key[32]) {
  struct stat st;
  if (stat(path, &st) != 0) die("no key file");
  if (st.st_mode & 077) die("the key file must be chmod 600");
  FILE *f = fopen(path, "r");
  char text[80] = {0};
  if (!f || !fgets(text, sizeof text, f)) die("can't read the key file");
  fclose(f);
  for (int i = 0; i < 32; i++) {
    int a = hexv(text[2 * i]), b = hexv(text[2 * i + 1]);
    if (a < 0 || b < 0) die("the key file must hold 64 hex digits");
    key[i] = (unsigned char)(a * 16 + b);
  }
}

// JSON string escaping for the NIP-01 serialization.
static size_t esc(const char *s, char *out, size_t cap) {
  size_t o = 0;
  for (const unsigned char *p = (const unsigned char *)s; *p; p++) {
    if (o + 8 >= cap) die("too long");
    if (*p == '"' || *p == '\\') { out[o++] = '\\'; out[o++] = (char)*p; }
    else if (*p == '\n') { out[o++] = '\\'; out[o++] = 'n'; }
    else if (*p == '\r') { out[o++] = '\\'; out[o++] = 'r'; }
    else if (*p == '\t') { out[o++] = '\\'; out[o++] = 't'; }
    else if (*p < 0x20) o += (size_t)snprintf(out + o, cap - o, "\\u%04x", *p);
    else out[o++] = (char)*p;
  }
  out[o] = 0;
  return o;
}

int main(int argc, char **argv) {
  secp256k1_context *ctx = secp256k1_context_create(SECP256K1_CONTEXT_NONE);
  unsigned char seed[32]; random_bytes(seed, 32);
  if (!secp256k1_context_randomize(ctx, seed)) die("context");
  unsigned char key[32];
  secp256k1_keypair pair;
  if (argc == 3 && !strcmp(argv[1], "--new")) {
    if (access(argv[2], F_OK) == 0) die("that key file exists already");
    do random_bytes(key, 32); while (!secp256k1_keypair_create(ctx, &pair, key));
    int fd = open(argv[2], O_WRONLY | O_CREAT | O_EXCL, 0600);
    char text[66]; hex(key, 32, text); text[64] = '\n';
    if (fd < 0 || write(fd, text, 65) != 65) die("can't write the key file");
    close(fd);
  } else if (argc >= 4 && !strcmp(argv[1], "--key")) {
    read_key(argv[2], key);
    if (!secp256k1_keypair_create(ctx, &pair, key)) die("not a valid key");
  } else {
    fprintf(stderr, "usage: sign --new FILE | sign --key FILE --pubkey | sign --key FILE METHOD URL [BODY_FILE]\n");
    return 2;
  }
  secp256k1_xonly_pubkey xonly;
  unsigned char pub[32]; char pubhex[65];
  secp256k1_keypair_xonly_pub(ctx, &xonly, NULL, &pair);
  secp256k1_xonly_pubkey_serialize(ctx, pub, &xonly);
  hex(pub, 32, pubhex);
  if (argc == 3 || !strcmp(argv[3], "--pubkey")) { printf("%s\n", pubhex); return 0; }
  if (argc < 5) die("METHOD URL needed");

  // the body's hash, for the payload tag
  char payload[65] = "";
  if (argc >= 6) {
    FILE *f = fopen(argv[5], "rb");
    if (!f) die("can't read the body");
    EVP_MD_CTX *h = EVP_MD_CTX_new();
    EVP_DigestInit_ex(h, EVP_sha256(), NULL);
    unsigned char buf[8192]; size_t n;
    while ((n = fread(buf, 1, sizeof buf, f)) > 0) EVP_DigestUpdate(h, buf, n);
    fclose(f);
    unsigned char d[32]; EVP_DigestFinal_ex(h, d, NULL); EVP_MD_CTX_free(h); hex(d, 32, payload);
  }
  char url[4096], method[32];
  esc(argv[4], url, sizeof url); esc(argv[3], method, sizeof method);
  long long now = (long long)time(NULL);
  if (getenv("SIGN_AGE")) now -= atoll(getenv("SIGN_AGE"));   // tests: an event this many seconds old
  char tags[4400];
  snprintf(tags, sizeof tags, "[[\"u\",\"%s\"],[\"method\",\"%s\"]%s%s%s]", url, method,
           payload[0] ? ",[\"payload\",\"" : "", payload, payload[0] ? "\"]" : "");
  // NIP-01: the id is the hash of [0, pubkey, created_at, kind, tags, content]
  char serial[5000];
  snprintf(serial, sizeof serial, "[0,\"%s\",%lld,27235,%s,\"\"]", pubhex, now, tags);
  unsigned char id[32]; SHA256((const unsigned char *)serial, strlen(serial), id);
  unsigned char sig[64], aux[32]; random_bytes(aux, 32);
  if (!secp256k1_schnorrsig_sign32(ctx, sig, id, &pair, aux)) die("signing failed");
  char idhex[65], sighex[129]; hex(id, 32, idhex); hex(sig, 64, sighex);
  char event[5600];
  int n = snprintf(event, sizeof event, "{\"id\":\"%s\",\"pubkey\":\"%s\",\"created_at\":%lld,\"kind\":27235,\"tags\":%s,\"content\":\"\",\"sig\":\"%s\"}",
                   idhex, pubhex, now, tags, sighex);
  unsigned char b64[8000];
  EVP_EncodeBlock(b64, (const unsigned char *)event, n);
  printf("%s\n", b64);
  memset(key, 0, sizeof key);
  secp256k1_context_destroy(ctx);
  return 0;
}
