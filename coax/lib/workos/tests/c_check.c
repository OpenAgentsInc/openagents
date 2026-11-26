/* Native C11 ABI + ASan/UBSan verification harness for coax/lib/workos/ Phase 1 (#14098).
 * Exercises every exported #authority primitive from workos_phase1.h, including
 * the full production 600,000-round PBKDF2-HMAC-SHA256 derivation for wos2. session sealing. */
#include <assert.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "workos_phase1.h"

static coax_string S(const char *lit) {
  return (coax_string){ .data = lit, .count = (int64_t)strlen(lit) };
}

static coax_v_u8 V(uint8_t *buf, int64_t n) {
  return (coax_v_u8){ .data = buf, .count = n };
}

static void hex_to_bytes(const char *hex, uint8_t *out, size_t n) {
  for (size_t i = 0; i < n; i++) {
    unsigned int v = 0;
    for (int j = 0; j < 2; j++) {
      char c = hex[i * 2 + (size_t)j];
      unsigned int d = (c >= '0' && c <= '9') ? (unsigned int)(c - '0')
                     : (c >= 'a' && c <= 'f') ? (unsigned int)(c - 'a' + 10)
                     : (unsigned int)(c - 'A' + 10);
      v = (v << 4) | d;
    }
    out[i] = (uint8_t)v;
  }
}

static void test_pkce_and_state(void) {
  uint8_t ch[43];
  int64_t n = wos_pkce_challenge(S("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"), V(ch, 43));
  assert(n == 43);
  assert(memcmp(ch, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM", 43) == 0);
  assert(wos_b64url_encode_size(32) == 43);
  assert(wos_pkce_verifier_ok(S("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk")));
  assert(!wos_pkce_verifier_ok(S("short")));

  uint8_t ent[32];
  for (int i = 0; i < 32; i++) ent[i] = (uint8_t)(i * 11 + 5);
  uint8_t ver[128], st[43];
  assert(wos_pkce_verifier_from_entropy(V(ent, 32), V(ver, 128)) == 43);
  assert(wos_state_from_entropy(V(ent, 32), V(st, 43)) == 43);
  coax_string s1 = { .data = (const char *)st, .count = 43 };
  uint8_t st2[43];
  memcpy(st2, st, 43);
  coax_string s2 = { .data = (const char *)st2, .count = 43 };
  assert(wos_state_equal(s1, s2));
  st2[0] ^= 1;
  assert(!wos_state_equal(s1, s2));
}

static void test_urls(void) {
  uint8_t out[1024];
  int64_t n = wos_percent_encode(S("a/b?c=1 2~"), V(out, sizeof(out)));
  assert(n == 18 && memcmp(out, "a%2Fb%3Fc%3D1%202~", 18) == 0);
  assert(wos_path_segment_ok(S("user_01ABC")));
  assert(!wos_path_segment_ok(S("%2e%2E")));
  assert(wos_path_ok(S("/user_management/users/user_01ABC")));
  assert(!wos_path_ok(S("/user_management/../secret")));

  n = wos_jwks_url(S(""), S("client_01"), V(out, sizeof(out)));
  assert(n == 41 && memcmp(out, "https://api.workos.com/sso/jwks/client_01", 41) == 0);

  n = wos_logout_url(S(""), S("session_01"), S("https://openagents.com/login"), V(out, sizeof(out)));
  assert(n > 0);
  assert(memcmp(out, "https://api.workos.com/user_management/sessions/logout?session_id=session_01&return_to=https%3A%2F%2Fopenagents.com%2Flogin", (size_t)n) == 0);

  coax_t_WosAuthUrlParams p = {0};
  p.client_id = S("client_01");
  p.redirect_uri = S("openagents://auth/callback");
  p.provider = S("AppleOAuth");
  p.state = S("st_xyz");
  p.code_challenge = S("E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
  p.code_challenge_method = S("S256");
  n = wos_authorize_url(S(""), p, V(out, sizeof(out)));
  assert(n > 0);
  const char *want = "https://api.workos.com/user_management/authorize?client_id=client_01&redirect_uri=openagents%3A%2F%2Fauth%2Fcallback&response_type=code&provider=AppleOAuth&state=st_xyz&code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM&code_challenge_method=S256";
  assert((size_t)n == strlen(want) && memcmp(out, want, (size_t)n) == 0);
}

static void test_webhooks_and_actions(void) {
  const char *hdr = "t=1700000000000, v1=32850522003ebbce01a416e5d5df8ec8354a96e921cac7d0ebbaeef5732bdaea";
  coax_t_WosSigHeader parsed = wos_sig_parse(S(hdr));
  assert(parsed.error == 0 && parsed.timestamp_ms == 1700000000000LL);
  assert(wos_sig_parse(S("t=1700000000000, v1=32850522003ebbce01a416e5d5df8ec8354a96e921cac7d0ebbaeef5732bdaea, v2=deadbeef")).error == 3);

  uint8_t hex[64];
  assert(wos_sig_compute(S("secret"), S("1700000000000"), S("body"), V(hex, 64)));
  assert(memcmp(hex, "32850522003ebbce01a416e5d5df8ec8354a96e921cac7d0ebbaeef5732bdaea", 64) == 0);
  assert(wos_sig_verify(S("secret"), S(hdr), S("body"), 1700000000000LL, 180000) == 0);
  assert(wos_sig_verify(S("secret"), S(hdr), S("body"), 1700000030001LL, 30000) == 5);
  assert(wos_sig_verify(S("wrong"), S(hdr), S("body"), 1700000000000LL, 180000) == 6);

  uint8_t resp[512];
  int64_t rn = wos_action_sign_response(1, false, S("Account frozen"), 1700000000000LL, S("shh"), V(resp, sizeof(resp)));
  assert(rn > 0);
  assert(strstr((const char *)resp, "\"verdict\":\"Deny\"") != NULL);
}

static void test_seal_and_600k_pbkdf2(void) {
  uint8_t key[32];
  assert(wos_seal_hex_key(S("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"), V(key, 32)));
  uint8_t plain[256];
  int64_t n = wos_unseal_raw(V(key, 32), S("wAR4QhPUo3v2n7hGlDGSyMHGYuRzavYm74i9bHLuyaISPeMBWg6ZSbPDsRI3n2S0kSfpi2QaKlq0x0IxzHwdjzkzwxeOh2XmPW8="), V(plain, sizeof(plain)));
  assert(n == 46 && memcmp(plain, "{\"access_token\":\"at_1\",\"refresh_token\":\"rt_1\"}", 46) == 0);

  uint8_t nonce[12] = {1,2,3,4,5,6,7,8,9,10,11,12};
  uint8_t msg[5] = {'h','e','l','l','o'};
  uint8_t sealed[256];
  int64_t sn = wos_seal_raw(V(key, 32), V(nonce, 12), V(msg, 5), V(sealed, sizeof(sealed)));
  assert(sn > 0);
  assert(wos_unseal_raw(V(key, 32), (coax_string){ .data = (const char *)sealed, .count = sn }, V(plain, sizeof(plain))) == 5);
  assert(memcmp(plain, "hello", 5) == 0);

  /* Full production 600,000-round PBKDF2-HMAC-SHA256 check against Python hashlib oracle. */
  uint8_t master[32], want_master[32];
  hex_to_bytes("cb5cdc10a5e76066d5489f29d85eca487661851beb0f22975a50252a654961aa", want_master, 32);
  assert(wos_seal_wos2_master(S("0123456789abcdef0123456789abcdef"), 600000, V(master, 32)));
  assert(memcmp(master, want_master, 32) == 0);

  uint8_t salt[16] = {1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16};
  uint8_t dkey[32];
  assert(wos_seal_wos2_derive(V(master, 32), V(salt, 16), V(dkey, 32)));
  sn = wos_seal_wos2(V(master, 32), V(salt, 16), V(nonce, 12), V(msg, 5), V(sealed, sizeof(sealed)));
  assert(sn > 5 && memcmp(sealed, "wos2.", 5) == 0);
  assert(wos_unseal_wos2(V(master, 32), (coax_string){ .data = (const char *)sealed, .count = sn }, V(plain, sizeof(plain))) == 5);
  assert(memcmp(plain, "hello", 5) == 0);
}

static void test_jwt_rs256(void) {
  const char *jwks = "{\"keys\":[{\"alg\":\"RS256\",\"kty\":\"RSA\",\"use\":\"sig\",\"kid\":\"key_01TEST\",\"n\":\"lOzQ9acbvUPnJraDqkdXc8kvoPPIWk4mwMiO1qYNazGhYvf6WBJI22krMim9uUSFBC5_QxgBtfWxVffX-_xGQKHhsqBnGee1JfQlK32c6NQqoudElOVR5PIwRmkog2qmAJWLD6lnVQ7pNVtZbI8qSF1Euvr_zWLw125WWNa-ws2c0VDuwOQKi2RRWlmwhsiDp3_JLLXHJXqEedBUoNkSqZIMJMbUqEa-SEq-tTS5qbnha13lWn9TEWAH43-o0MGZBAYRBZaS3yxTIyZ38SeO82Dl4OAJA1amC9gMRWcrr6DjuIxHCbDwuTheaADs6RKYps3c4ST_AFumIk79phXvPQ\",\"e\":\"AQAB\"}]}";
  const char *user_jwt = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImtleV8wMVRFU1QifQ.eyJpc3MiOiJodHRwczovL2F1dGgub3BlbmFnZW50cy5jb20iLCJzdWIiOiJ1c2VyXzAxQUJDIiwic2lkIjoic2Vzc2lvbl8wMVhZWiIsIm9yZ19pZCI6Im9yZ18wMSIsInJvbGUiOiJhZG1pbiIsImF1dGhfdGltZSI6MTcwMDAwMDAwMCwiaWF0IjoxNzAwMDAwMDAwLCJuYmYiOjE3MDAwMDAwMDAsImV4cCI6MTcwMDAwMzYwMH0.fOXKkytZ-7hYCo1ULQF7dg4eeRz_KXyJW3X4HoynYEwhmwUIyoTnD_c-V773Y1jz1FixO5UDjQ3B7iOcFrdAWiCWiIbsotiihn7J2_Vt4MEDF-yxZSKSwZrtfYkxuZsMgUHOT-bwKd59BdLqY8x2JtuLfBtiBFvu4a1paPqRHHWitsdt1KKXo131R27TIocZ6iyOpDfaeeS57JJwnb-PtyYqvrK6Nrku_GOxx13aIMEryibvFhx8RTqIwMACB_2lEB5h5koVn2mVyxzgpyKFpfE_zb7sQGAB1Ep7hdSFcKfUOvrW9u_RFEgWiwRuXh95jyTZcPX1VPZN5uv0qvSeEQ";
  const char *widget_jwt = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6ImtleV8wMVRFU1QifQ.eyJzdWIiOiJ1c2VyXzAxQUJDIiwic2lkIjoic2Vzc2lvbl8wMVhZWiIsImF1dGhfdGltZSI6MTcwMDAwMDAwMCwic3ViX3Byb2ZpbGUiOiJ3aWRnZXQiLCJleHAiOjE3MDAwMDM2MDB9.EUA3z8BYZy6FQzE49kGgYjhXaYSNxggIXCFylnnTXB2mxMT07EwobHvYTexBTVCUcZYRCM3bCev9oY1fFXztfpwGPwf49i8QtNdsm40Glpk1rGW3jOYgF3NXhfYSM2wtwNBcRl6Dlio_lYMK6dzNVG-tG1l7JXH_HFdMkNc639mtkxGxvEbQ32gzil-T-0BvsN1RAOMXbMBG6ymg1MtD6koW47D0KpGgUALb5-idWFERHKlguFAR_vcdgDH8QHLG9T7ssHp0jTtxtrDd1f7XuV0qNFu8oWwV7h-BwzIoO-cBcPs50E0H-kzSRoBGDQIXYRdNWcupXs6E6pUk2p__8w";

  coax_t_WosJwtClaims claims = {0};
  coax_v_t_WosJwtClaims cv = { .data = &claims, .count = 1 };
  assert(wos_jwt_verify_session(S(user_jwt), S(jwks), S("https://auth.openagents.com"), 1700000100LL, cv) == 0);
  assert(claims.error == 0 && claims.sub_n == 10 && memcmp(claims.sub.e, "user_01ABC", 10) == 0);
  assert(claims.sid_n == 13 && memcmp(claims.sid.e, "session_01XYZ", 13) == 0);
  assert(wos_jwt_verify_session(S(user_jwt), S(jwks), S(""), 1700003600LL, cv) == 1);
  assert(wos_jwt_verify_session(S(widget_jwt), S(jwks), S(""), 1700000100LL, cv) == 10);
}

static void test_error_and_retry(void) {
  coax_t_WosApiError err = {0};
  coax_v_t_WosApiError ev = { .data = &err, .count = 1 };
  assert(wos_retry_after_parse(S(" 42 ")) == 42);
  wos_parse_api_error(403, S("{\"code\":\"organization_selection_required\",\"message\":\"Pick org\",\"pending_authentication_token\":\"pat_1\"}"), S("12"), S("req_9"), ev);
  assert(err.kind == 1 && err.retry_after_sec == 12);
  assert(err.pending_n == 5 && memcmp(err.pending_auth_token.e, "pat_1", 5) == 0);
  assert(wos_refresh_classify(err) == 3);

  wos_parse_api_error(503, S("unavailable"), S(""), S(""), ev);
  assert(wos_refresh_classify(err) == 1);
  wos_parse_api_error(401, S("{\"error\":\"invalid_grant\"}"), S(""), S(""), ev);
  assert(wos_refresh_classify(err) == 2);

  assert(wos_method_safe(1) && !wos_method_safe(4));
  assert(wos_status_retryable(429) && !wos_status_retryable(400));
  assert(!wos_retry_allowed(4, false, 429, 0, 3));
  assert(wos_retry_allowed(4, true, 429, 0, 3));
  assert(wos_backoff_ms(0, -1, 0, false) == 200);
  assert(wos_backoff_ms(0, 7, 0, true) == 7000);
}

int main(void) {
  test_pkce_and_state();
  test_urls();
  test_webhooks_and_actions();
  test_seal_and_600k_pbkdf2();
  test_jwt_rs256();
  test_error_and_retry();
  puts("workos phase1 c_check: all native ASan/UBSan checks passed (including 600k-round PBKDF2)");
  return 0;
}
