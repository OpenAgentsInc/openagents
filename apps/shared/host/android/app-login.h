/* The Android app's login to openagents.com (GitHub #13229; docs/auth/sessions-and-tokens.md). The
   flow and every decision are Coax (apps/shared/app-login-flow.coax and app-login.coax, compiled as
   app-login-unit.coax); this does the effects on a thread of its own:

   - HTTPS through java.net.HttpURLConnection (JNI);
   - the login's record and the phone's Nostr key, each AES-GCM encrypted under one Android
     Keystore key (alias "oa-app-login-store") in the app's own files, mode 0600;
   - the device key that binds the login to this phone (device-key.c, alias "oa-app-login-device"):
     its public half with every login, a proof with every refresh; forgotten when the login ends;
   - the Nostr key "Log in with a key" signs with: made on the phone the first time, kept only
     encrypted, never shown or sent.

   The screen asks with oa_app_login_ask and reads oa_app_login_status. Nothing here logs a token,
   a code, an email or a key. */
#pragma once
#include <jni.h>
#include <stdbool.h>
#include <stdint.h>

/* What the screen asks for: account.coax's AC_* codes. */
enum { OA_LOGIN_EMAIL = 1, OA_LOGIN_CODE = 2, OA_LOGIN_KEY = 3, OA_LOGIN_LOGOUT = 4, OA_LOGIN_BACK = 5, OA_LOGIN_WHO = 6 };

/* Starts the login's thread. dir: the app's files folder (ANativeActivity's internalDataPath);
   origin: "https://openagents.com", or another for development. Reads the kept login and, when
   there is one, asks who it is. */
bool oa_app_login_start(JavaVM *vm, const char *dir, const char *origin);
/* Asks for one thing; ignored while another is under way (the screen shows busy). */
void oa_app_login_ask(int kind, const char *text, int length);
/* What the screen shows: state (0 logged out, 1 code sent, 2 logged in), busy, the server's words
   and who is logged in, into the caller's buffers (NUL-ended). Returns a count that changes
   whenever any of it does. */
uint32_t oa_app_login_status(int *state, bool *busy, char *words, int words_cap, char *who, int who_cap);
