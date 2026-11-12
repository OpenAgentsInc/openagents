// The seat ticket's C writer against the one Coax writer (#13248): player.c's v2/v3/v4 snprintf and
// HMAC (copied below as it stood at 66d705a84f, coax/web/api/player.c:515-566, with its b64url and
// ticket_name) and seat_ticket_hmac_write (aethyr/session/seat-ticket.coax, through ticket-sign-unit.c)
// must give the same bytes for proto/verse/seat-ticket-hmac-v1.json's values and for a seeded sweep of
// player.c's inputs. Run once before player.c's writer went; it goes with it.
//   coax/bin/coax c coax/web/api/ticket-sign.coax --run-steps 250000000 --library --header W/ticket-sign.generated.h > W/ticket-sign.generated.c
//   cc -std=c11 -O1 -D_DEFAULT_SOURCE -IW -Icoax/web/api coax/web/api/tests/ticket-hmac-diff.c -lcrypto -o W/diff && W/diff
#include "../ticket-sign-unit.c"
#include <stdio.h>
#include <stdlib.h>
#include <openssl/evp.h>
#include <openssl/hmac.h>

// ---- player.c's writer, as it was
static void b64url(char *out, size_t cap, const unsigned char *b, size_t n) {
  static const char a[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
  size_t o = 0;
  for (size_t i = 0; i < n && o + 5 < cap; i += 3) {
    unsigned v = (unsigned)b[i] << 16 | (i + 1 < n ? (unsigned)b[i + 1] << 8 : 0) | (i + 2 < n ? b[i + 2] : 0);
    out[o++] = a[v >> 18 & 63]; out[o++] = a[v >> 12 & 63];
    if (i + 1 < n) out[o++] = a[v >> 6 & 63];
    if (i + 2 < n) out[o++] = a[v & 63];
  }
  out[o] = 0;
}
static size_t ticket_name(char *out, size_t cap, const char *in) {
  size_t m = 0;
  for (const unsigned char *p = (const unsigned char *)(in ? in : ""); *p && m + 1 < cap && m < 24; p++) {
    unsigned char ch = *p;
    bool ok = (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z') || (ch >= '0' && ch <= '9') || (ch == ' ' && m > 0) || ch == '-' || ch == '_';
    if (ok) out[m++] = (char)ch;
  }
  while (m > 0 && out[m - 1] == ' ') m--;
  out[m] = 0;
  return m;
}
// resume: player.c's resume == 1 (its checkpoint_owner the owner); og: OA_ROOM_OWNER_GEN=1's generation
// (NULL none). The ticket into ticket (4097), its length; 0 where player.c answered an error.
static size_t c_writer(const char *secret, const char *pid, int acct, const char *name, size_t nn, const char *room, int shard,
                       long long expires, int hp, int xp, int wanted, int resume, long long checkpoint_owner, const char *og,
                       const char *saved, char *ticket) {
  char name64[48];
  if (nn) b64url(name64, sizeof name64, (const unsigned char *)name, nn); else strcpy(name64, "-");
  char text[256];
  char hps[32]; if (hp >= 0 && hp < 100000) snprintf(hps, sizeof hps, "%d", hp); else strcpy(hps, "-");
  if (xp > 0) snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "x%d", xp);
  if (wanted >= 1 && wanted <= 5) { if (xp <= 0) snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "x0"); snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "w%d", wanted); }
  if (resume==1) { snprintf(hps+strlen(hps),sizeof hps-strlen(hps),"g%lld",checkpoint_owner); }
  else if (og) snprintf(hps + strlen(hps), sizeof hps - strlen(hps), "g%s", og);
  char resumed[4097];
  int tn = shard > 0 ? snprintf(text, sizeof text, "v3.%s.%d.%s.%s.%d.%lld.%s", pid, acct, name64, room, shard, expires, hps)
                     : snprintf(text, sizeof text, "v2.%s.%d.%s.%s.%lld.%s", pid, acct, name64, room, expires, hps);
  if (tn <= 0 || (size_t)tn + 66 >= sizeof text) return 0;
  unsigned char mac[EVP_MAX_MD_SIZE]; unsigned int ml = 0;
  if (!HMAC(EVP_sha256(), secret, (int)strlen(secret), (const unsigned char *)text, (size_t)tn, mac, &ml) || ml != 32) return 0;
  text[tn++] = '.';
  for (unsigned i = 0; i < 32; i++) tn += snprintf(text + tn, sizeof text - (size_t)tn, "%02x", mac[i]);
  if (resume==1) {
    coax_string prefix=resume_ticket_prefix((coax_string){text,tn},(coax_string){saved,(int64_t)strlen(saved)},(coax_v_u8){(uint8_t *)resumed,4031});
    if (!prefix.count || !HMAC(EVP_sha256(),secret,(int)strlen(secret),(const unsigned char *)resumed,(size_t)prefix.count,mac,&ml) || ml!=32) return 0;
    int pos=(int)prefix.count;resumed[pos++]='.';for(unsigned i=0;i<32;i++)pos+=snprintf(resumed+pos,sizeof resumed-(size_t)pos,"%02x",mac[i]);
    memcpy(ticket, resumed, (size_t)pos); ticket[pos] = 0; return (size_t)pos;
  }
  memcpy(ticket, text, (size_t)tn); ticket[tn] = 0; return (size_t)tn;
}

// ---- the comparison
static unsigned long long seed = 13248;
static unsigned long long rnd(void) { seed ^= seed << 13; seed ^= seed >> 7; seed ^= seed << 17; return seed; }
static long long pick(long long lo, long long hi) { return lo + (long long)(rnd() % (unsigned long long)(hi - lo + 1)); }
static void code(char *out, int most) {
  static const char a[] = "abcdefghijklmnopqrstuvwxyz0123456789-";
  int n = (int)pick(1, most);
  for (int i = 0; i < n; i++) out[i] = a[pick(0, 36)];
  out[n] = 0;
}
static const char *SAVED =
  "{\"v\":2,\"owner\":7,\"process\":101,\"rev\":3,\"at\":1762190000000,\"engine\":1,\"rules\":1,\"content\":1,\"hp\":0,\"max\":20,\"xp\":350,\"wanted\":2,\"x_mm\":-1234,\"y_mm\":200,\"z_mm\":-9876,\"yaw_mrad\":1234,\"pid\":\"p-a\",\"room\":\"pair-a\",\"dead\":true,\"state\":[1,0,4,50,2,6,0,2,0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0,0,0,0]}";
static int same = 0, differ = 0, both_refused = 0;
static void compare(const char *why, const char *key, const char *pid, int acct, const char *raw_name, const char *room, int shard,
                    long long expires, int hp, int xp, int wanted, int resume, long long owner, bool og, const char *saved) {
  char name[32]; size_t nn = ticket_name(name, sizeof name, raw_name);
  char ogs[24]; snprintf(ogs, sizeof ogs, "%lld", owner);
  char a[4097], b[4097];
  size_t an = c_writer(key, pid, acct, name, nn, room, shard, expires, hp, xp, wanted, resume, owner, !resume && og && owner > 0 ? ogs : NULL, saved, a);
  size_t bn = stk_hmac_write_c(key, pid, acct, name, room, shard, expires, hp, xp, wanted, resume || (og && owner > 0) ? owner : 0, resume, saved, b, sizeof b);
  if (an == 0 && bn == 0) { both_refused++; return; }
  if (an == bn && !memcmp(a, b, an)) { same++; return; }
  differ++;
  if (differ <= 10) printf("DIFFER %s\n  c:    %s\n  coax: %s\n", why, an ? a : "(refused)", bn ? b : "(refused)");
}

int main(void) {
  const char *fk = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
  // the fixtures' values (proto/verse/seat-ticket-hmac-v1.json)
  compare("fixture v2", fk, "p-ffeeddccbbaa9988", 0, "", "dawn", 0, 1762190600000LL, -1, 0, 0, 0, 0, false, "-");
  compare("fixture v3", fk, "p-0123456789abcdef", 1, "Ash", "everglade-2", 3, 1762190600000LL, 14, 320, 3, 0, 12, true, "-");
  compare("fixture v4", fk, "p-a", 0, "Ash", "pair-a", 1, 1762190060000LL, 7, 350, 2, 1, 7, false, SAVED);
  int fixtures = same;
  // a seeded sweep over player.c's inputs: xp as player.c clamps it (0 to 9999999), owners of 1 or more
  static const long long hps[] = {-1, 0, 1, 99999, 100000, 100001, -2};
  static const long long xps[] = {0, 1, 9, 10, 9999999};
  static const long long ex[] = {1, 9, 10, 999999999999999LL, 1762190600000LL};
  static const long long ow[] = {0, 1, 9, 10, 9999999999LL};
  char key[130], pid[48], room[32], raw[40];
  for (int i = 0; i < 200000; i++) {
    int kn = (int)pick(32, 128);
    for (int k = 0; k < kn; k++) key[k] = (char)pick(33, 126);
    key[kn] = 0;
    if (pick(0, 1)) snprintf(pid, sizeof pid, "p-%016llx", rnd()); else code(pid, 40);
    code(room, 24);
    int rn = (int)pick(0, 30);
    for (int k = 0; k < rn; k++) raw[k] = pick(0, 3) ? (char)pick(32, 126) : ' ';
    raw[rn] = 0;
    int shard = pick(0, 3) ? (int)pick(0, 99) : (int)pick(0, 1) * 99;
    long long expires = pick(0, 1) ? ex[pick(0, 4)] : pick(1, 999999999999999LL);
    int hp = pick(0, 1) ? (int)hps[pick(0, 6)] : (int)pick(-5, 120000);
    int xp = pick(0, 1) ? (int)xps[pick(0, 4)] : (int)pick(0, 9999999);
    int wanted = (int)pick(-1, 6);
    int resume = pick(0, 3) == 0;
    long long owner = pick(0, 1) ? ow[pick(0, 4)] : pick(1, 9999999999LL);
    if (resume && owner < 1) owner = 1;
    compare("sweep", key, pid, (int)pick(0, 2), raw, room, shard, expires, hp, xp, wanted, resume, owner, pick(0, 1), resume && pick(0, 1) ? SAVED : "-");
  }
  printf("ticket-hmac-diff: fixtures %d of 3 the same; sweep %d the same, %d refused by both, %d differ\n", fixtures, same - fixtures, both_refused, differ);
  return fixtures == 3 && differ == 0 ? 0 : 1;
}
