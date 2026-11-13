/* The Android app's one HTTPS call (GitHub #13229, #13379): the login's requests (app-login.c) and
   the crash reporter's post (crash-reports.c) both go out through it. */
#pragma once
#include <jni.h>
#include <stdint.h>

/* One HTTPS exchange through java.net.HttpURLConnection on the calling thread's env (it must be
   attached): the status, 0 when nothing answered, and the answer's bytes in answer. headers: nheaders
   pairs of name and value. body NULL sends none. */
int oa_https(JNIEnv *e, const char *method, const char *url, const char *const *headers, int nheaders, const char *body, int body_n, uint8_t *answer, int answer_cap, int *answer_n);
