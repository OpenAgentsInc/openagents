/* The Mac app's account login (#13229): what Account.swift calls in lab-sign.coax's unit, and what it
   gives main.c. The login itself is apps/shared/host/apple/AppAuth.swift, the iPhone app's own. */
#pragma once
#include <stdbool.h>
#include <stdint.h>

#ifndef COAX_STRING_DEFINED
#define COAX_STRING_DEFINED
typedef struct { const char *data; int64_t count; } coax_string;
#endif
#ifndef COAX_coax_v_u8_DEFINED
#define COAX_coax_v_u8_DEFINED
typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
#endif

/* lab-sign.coax: the CLI key file's format, a new key's check, and a NIP-98 event for a POST. */
int64_t coaxial_account_nip98(coax_v_u8 key, coax_v_u8 aux, int64_t now_s, coax_string url, coax_string payload, coax_v_u8 out);
bool coaxial_account_key_parse(coax_string text, coax_v_u8 key);
int64_t coaxial_account_key_text(coax_v_u8 key, coax_v_u8 out);
bool coaxial_account_profile_ok(coax_string name);
bool coaxial_account_key_ok(coax_v_u8 key);

/* Account.swift, for main.c. changed is called on the main thread whenever the login or its title changes. */
void oa_account_start(void (*changed)(void));
void oa_account_login(void);    /* the Log In sheets: an email code, or this Mac's key */
void oa_account_logout(void);
void oa_account_security(void);   /* the Security sheet: notices, a waiting recovery (Cancel Recovery) and Freeze (#13233) */
int32_t oa_account_logged_in(void);
/* "Log In…", or "Logged in as a•••@example.com": its length, written NUL-ended into out. */
int32_t oa_account_title(char *out, int32_t cap);
/* coax drive's `account state | key | logout | refresh | security`, without a sheet: the answer's length. */
int32_t oa_account_drive(const char *request, char *out, int32_t cap);
/* CrashReports.swift (apps/shared/host/apple/, #13379): listens for the system's crash diagnostics and
   sends an earlier crash's dump once the person has said yes. */
void oa_crash_reports_start(void);
