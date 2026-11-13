/* The Android app's one HTTPS call: see https.h. java.net.HttpURLConnection through JNI: no
   redirects followed, no cache, 15 s to connect and 20 s to read. Nothing here logs a URL, a header
   or a body. */
#include "https.h"
#include <stdbool.h>

static bool threw(JNIEnv *e) { if (!(*e)->ExceptionCheck(e)) return false; (*e)->ExceptionClear(e); return true; }
static jclass cls(JNIEnv *e, const char *n) { jclass c = (*e)->FindClass(e, n); return threw(e) ? NULL : c; }
static jmethodID mid(JNIEnv *e, jclass c, const char *n, const char *s) { jmethodID m = c ? (*e)->GetMethodID(e, c, n, s) : NULL; return threw(e) ? NULL : m; }
static jstring jstr(JNIEnv *e, const char *s) { jstring j = (*e)->NewStringUTF(e, s); return threw(e) ? NULL : j; }
static jbyteArray jbytes(JNIEnv *e, const void *p, int n) {
  jbyteArray a = (*e)->NewByteArray(e, n);
  if (threw(e) || !a) return NULL;
  (*e)->SetByteArrayRegion(e, a, 0, n, (const jbyte *)p);
  return threw(e) ? NULL : a;
}

int oa_https(JNIEnv *e, const char *method, const char *url, const char *const *headers, int nheaders, const char *body, int body_n, uint8_t *answer, int answer_cap, int *answer_n) {
  int status = 0; *answer_n = 0;
  if ((*e)->PushLocalFrame(e, 64) != 0) { threw(e); return 0; }
  jclass url_c = cls(e, "java/net/URL"), con_c = cls(e, "java/net/HttpURLConnection"), out_c = cls(e, "java/io/OutputStream"), in_c = cls(e, "java/io/InputStream");
  jmethodID url_new = mid(e, url_c, "<init>", "(Ljava/lang/String;)V"), open = mid(e, url_c, "openConnection", "()Ljava/net/URLConnection;");
  jmethodID set_method = mid(e, con_c, "setRequestMethod", "(Ljava/lang/String;)V"), set_prop = mid(e, con_c, "setRequestProperty", "(Ljava/lang/String;Ljava/lang/String;)V");
  jmethodID connect_ms = mid(e, con_c, "setConnectTimeout", "(I)V"), read_ms = mid(e, con_c, "setReadTimeout", "(I)V");
  jmethodID follow = mid(e, con_c, "setInstanceFollowRedirects", "(Z)V"), caches = mid(e, con_c, "setUseCaches", "(Z)V"), output = mid(e, con_c, "setDoOutput", "(Z)V");
  jmethodID get_out = mid(e, con_c, "getOutputStream", "()Ljava/io/OutputStream;"), code = mid(e, con_c, "getResponseCode", "()I");
  jmethodID get_in = mid(e, con_c, "getInputStream", "()Ljava/io/InputStream;"), get_err = mid(e, con_c, "getErrorStream", "()Ljava/io/InputStream;");
  jmethodID done = mid(e, con_c, "disconnect", "()V"), write = mid(e, out_c, "write", "([B)V"), close_out = mid(e, out_c, "close", "()V");
  jmethodID read = mid(e, in_c, "read", "([BII)I"), close_in = mid(e, in_c, "close", "()V");
  if (!url_new || !open || !set_method || !set_prop || !connect_ms || !read_ms || !follow || !caches || !output || !get_out || !code || !get_in || !get_err || !done || !write || !close_out || !read || !close_in) goto out;
  jobject u = (*e)->NewObject(e, url_c, url_new, jstr(e, url));
  if (threw(e) || !u) goto out;
  jobject c = (*e)->CallObjectMethod(e, u, open);
  if (threw(e) || !c) goto out;
  (*e)->CallVoidMethod(e, c, set_method, jstr(e, method)); if (threw(e)) goto out;
  (*e)->CallVoidMethod(e, c, connect_ms, (jint)15000); (*e)->CallVoidMethod(e, c, read_ms, (jint)20000);
  (*e)->CallVoidMethod(e, c, follow, JNI_FALSE); (*e)->CallVoidMethod(e, c, caches, JNI_FALSE);
  if (threw(e)) goto out;
  for (int i = 0; i + 1 < nheaders * 2; i += 2) { (*e)->CallVoidMethod(e, c, set_prop, jstr(e, headers[i]), jstr(e, headers[i + 1])); if (threw(e)) goto drop; }
  if (body) {
    (*e)->CallVoidMethod(e, c, output, JNI_TRUE); if (threw(e)) goto drop;
    jobject os = (*e)->CallObjectMethod(e, c, get_out);
    if (threw(e) || !os) goto drop;
    (*e)->CallVoidMethod(e, os, write, jbytes(e, body, body_n)); bool bad = threw(e);
    (*e)->CallVoidMethod(e, os, close_out); bad = threw(e) || bad;
    if (bad) goto drop;
  }
  int got = (*e)->CallIntMethod(e, c, code);
  if (threw(e) || got <= 0) goto drop;
  jobject is = (*e)->CallObjectMethod(e, c, got >= 400 ? get_err : get_in);
  if (threw(e)) is = NULL;
  if (is) {
    jbyteArray buf = (*e)->NewByteArray(e, 8192);
    int n = 0;
    while (buf && n < answer_cap) {
      int r = (*e)->CallIntMethod(e, is, read, buf, (jint)0, (jint)8192);
      if (threw(e) || r < 0) break;
      if (r > answer_cap - n) r = answer_cap - n;
      (*e)->GetByteArrayRegion(e, buf, 0, r, (jbyte *)answer + n);
      n += r;
    }
    (*e)->CallVoidMethod(e, is, close_in); threw(e);
    *answer_n = n;
  }
  status = got;
drop:
  (*e)->CallVoidMethod(e, c, done); threw(e);
out:
  (*e)->PopLocalFrame(e, NULL);
  return status;
}
