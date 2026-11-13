/* The Android app's crash reporter (GitHub #13379): see crash-reports.h. The dump's text is Coax
   (crash-dump.coax, compiled as crash-dump.h); this does the effects on a thread of its own: the
   exit records and their tombstones through ActivityManager (JNI), two small files, and the post
   through the app's one HTTPS call (https.c oa_https). */
#include "crash-reports.h"
#include "https.h"
#include "crash-dump.h"
#include <android/log.h>
#include <fcntl.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define SAY(...) __android_log_print(ANDROID_LOG_INFO, "oa-crash", __VA_ARGS__)
#define TOMB_MAX (2 << 20)   /* a tombstone is read whole: 2 MB at most */
#define DUMP_MAX 16000       /* crash-dump.coax ACD_MAX: what the route takes */
#define PENDING 4            /* crashes sent from one launch */
#define EXITS 16             /* exit records looked at */
enum { ASK = 0, YES = 1, NO = 2 };

static struct {
  JavaVM *vm; jobject activity;
  char dir[512], origin[256], exe[64];
  pthread_mutex_t lock; pthread_cond_t wake;
  int asking, answered;                       /* the question is up; the person's word (YES or NO), not yet acted on */
  char dump[PENDING][DUMP_MAX]; int dump_n[PENDING]; int dumps;
  int64_t newest;                             /* when the newest crash read happened, ms */
} C = { .lock = PTHREAD_MUTEX_INITIALIZER, .wake = PTHREAD_COND_INITIALIZER };

/* ---- the two files: the answer, and the newest crash handled */

static int file_read(const char *name, char *out, int cap) {
  char p[640]; snprintf(p, sizeof p, "%s/%s", C.dir, name);
  int fd = open(p, O_RDONLY | O_CLOEXEC); if (fd < 0) return 0;
  int n = (int)read(fd, out, (size_t)cap - 1); close(fd);
  if (n < 0) n = 0;
  out[n] = 0; return n;
}
static bool file_write(const char *name, const char *text) {
  char p[640], t[648]; snprintf(p, sizeof p, "%s/%s", C.dir, name); snprintf(t, sizeof t, "%s.new", p);
  int fd = open(t, O_WRONLY | O_CREAT | O_TRUNC | O_CLOEXEC, 0600); if (fd < 0) return false;
  size_t n = strlen(text); bool ok = write(fd, text, n) == (ssize_t)n;
  ok = close(fd) == 0 && ok;
  if (!ok || rename(t, p) != 0) { unlink(t); return false; }
  return true;
}
static int consent_read(void) { char b[16]; file_read("crash-consent", b, sizeof b); return strncmp(b, "yes", 3) == 0 ? YES : strncmp(b, "no", 2) == 0 ? NO : ASK; }
static int64_t seen_read(void) { char b[32]; file_read("crash-seen", b, sizeof b); return strtoll(b, NULL, 10); }
static void seen_write(int64_t ms) { char b[32]; snprintf(b, sizeof b, "%lld\n", (long long)ms); file_write("crash-seen", b); }

/* ---- JNI pieces */

static bool threw(JNIEnv *e) { if (!(*e)->ExceptionCheck(e)) return false; (*e)->ExceptionClear(e); return true; }
static jclass cls(JNIEnv *e, const char *n) { jclass c = (*e)->FindClass(e, n); return threw(e) ? NULL : c; }
static jmethodID mid(JNIEnv *e, jclass c, const char *n, const char *s) { jmethodID m = c ? (*e)->GetMethodID(e, c, n, s) : NULL; return threw(e) ? NULL : m; }
/* A Java string's bytes into out (NUL-ended, cut to cap): its length. */
static int text_of(JNIEnv *e, jstring s, char *out, int cap) {
  out[0] = 0; if (!s) return 0;
  const char *u = (*e)->GetStringUTFChars(e, s, NULL);
  if (threw(e) || !u) return 0;
  int n = (int)strlen(u); if (n > cap - 1) n = cap - 1;
  memcpy(out, u, (size_t)n); out[n] = 0;
  (*e)->ReleaseStringUTFChars(e, s, u);
  return n;
}
static coax_string S(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }

/* "Android RELEASE (API LEVEL)", and the package's version name and code. */
static void about(JNIEnv *e, char *os, int os_cap, char *version, int version_cap, char *buildno, int buildno_cap) {
  os[0] = version[0] = buildno[0] = 0;
  if ((*e)->PushLocalFrame(e, 32) != 0) { threw(e); return; }
  jclass ver = cls(e, "android/os/Build$VERSION");
  jfieldID release = ver ? (*e)->GetStaticFieldID(e, ver, "RELEASE", "Ljava/lang/String;") : NULL; if (threw(e)) release = NULL;
  jfieldID sdk = ver ? (*e)->GetStaticFieldID(e, ver, "SDK_INT", "I") : NULL; if (threw(e)) sdk = NULL;
  if (release && sdk) {
    char r[48]; text_of(e, (jstring)(*e)->GetStaticObjectField(e, ver, release), r, sizeof r);
    snprintf(os, (size_t)os_cap, "Android %s (API %d)", r, (int)(*e)->GetStaticIntField(e, ver, sdk));
  }
  jclass ctx = cls(e, "android/content/Context"), pm_c = cls(e, "android/content/pm/PackageManager"), pi_c = cls(e, "android/content/pm/PackageInfo");
  jmethodID get_pm = mid(e, ctx, "getPackageManager", "()Landroid/content/pm/PackageManager;"), get_name = mid(e, ctx, "getPackageName", "()Ljava/lang/String;");
  jmethodID get_info = mid(e, pm_c, "getPackageInfo", "(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;"), code = mid(e, pi_c, "getLongVersionCode", "()J");
  jfieldID name = pi_c ? (*e)->GetFieldID(e, pi_c, "versionName", "Ljava/lang/String;") : NULL; if (threw(e)) name = NULL;
  if (get_pm && get_name && get_info && code && name) {
    jobject pm = (*e)->CallObjectMethod(e, C.activity, get_pm); if (threw(e)) pm = NULL;
    jobject pkg = (*e)->CallObjectMethod(e, C.activity, get_name); if (threw(e)) pkg = NULL;
    jobject info = pm && pkg ? (*e)->CallObjectMethod(e, pm, get_info, pkg, (jint)0) : NULL; if (threw(e)) info = NULL;
    if (info) {
      text_of(e, (jstring)(*e)->GetObjectField(e, info, name), version, version_cap);
      jlong c = (*e)->CallLongMethod(e, info, code);
      if (!threw(e)) snprintf(buildno, (size_t)buildno_cap, "%lld", (long long)c);
    }
  }
  (*e)->PopLocalFrame(e, NULL);
}

/* The native crashes since `seen`, newest first: each one's dump into C.dump (PENDING at most), and
   C.newest the time of the newest. Returns false when the records could not be read. */
static bool collect(JNIEnv *e, int64_t seen) {
  bool read_ok = false;
  char os[96], version[64], buildno[32]; about(e, os, sizeof os, version, sizeof version, buildno, sizeof buildno);
  uint8_t *tomb = malloc(TOMB_MAX); if (!tomb) return false;
  if ((*e)->PushLocalFrame(e, 32) != 0) { threw(e); free(tomb); return false; }
  jclass ctx = cls(e, "android/content/Context"), am_c = cls(e, "android/app/ActivityManager"), list_c = cls(e, "java/util/List");
  jclass info_c = cls(e, "android/app/ApplicationExitInfo"), in_c = cls(e, "java/io/InputStream");
  jmethodID service = mid(e, ctx, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;");
  jmethodID history = mid(e, am_c, "getHistoricalProcessExitReasons", "(Ljava/lang/String;II)Ljava/util/List;");
  jmethodID size = mid(e, list_c, "size", "()I"), get = mid(e, list_c, "get", "(I)Ljava/lang/Object;");
  jmethodID reason = mid(e, info_c, "getReason", "()I"), when = mid(e, info_c, "getTimestamp", "()J"), status = mid(e, info_c, "getStatus", "()I");
  jmethodID words = mid(e, info_c, "getDescription", "()Ljava/lang/String;"), trace = mid(e, info_c, "getTraceInputStream", "()Ljava/io/InputStream;");
  jmethodID in_read = mid(e, in_c, "read", "([BII)I"), in_close = mid(e, in_c, "close", "()V");
  if (!service || !history || !size || !get || !reason || !when || !status || !words || !trace || !in_read || !in_close) goto out;
  jstring activity_word = (*e)->NewStringUTF(e, "activity"); if (threw(e) || !activity_word) goto out;
  jobject am = (*e)->CallObjectMethod(e, C.activity, service, activity_word); if (threw(e) || !am) goto out;
  /* a null package: every record of this app's own */
  jobject list = (*e)->CallObjectMethod(e, am, history, (jobject)NULL, (jint)0, (jint)EXITS); if (threw(e) || !list) goto out;
  int count = (*e)->CallIntMethod(e, list, size); if (threw(e)) goto out;
  read_ok = true;
  for (int i = 0; i < count; i++) {
    if ((*e)->PushLocalFrame(e, 16) != 0) { threw(e); break; }
    jobject info = (*e)->CallObjectMethod(e, list, get, (jint)i);
    if (!threw(e) && info) {
      int why = (*e)->CallIntMethod(e, info, reason); bool bad = threw(e);
      int64_t t = (int64_t)(*e)->CallLongMethod(e, info, when); bad = threw(e) || bad;
      if (!bad && acd_reports(why) && t > seen) {
        if (t > C.newest) C.newest = t;
        if (C.dumps < PENDING) {
          int st = (*e)->CallIntMethod(e, info, status); if (threw(e)) st = 0;
          char said[256]; jobject d = (*e)->CallObjectMethod(e, info, words); if (threw(e)) d = NULL;
          text_of(e, (jstring)d, said, sizeof said);
          /* the tombstone, read whole; none on a system that keeps none (before Android 12) */
          int n = 0;
          jobject in = (*e)->CallObjectMethod(e, info, trace); if (threw(e)) in = NULL;
          if (in) {
            jbyteArray buf = (*e)->NewByteArray(e, 16384); if (threw(e)) buf = NULL;
            while (buf && n < TOMB_MAX) {
              int r = (*e)->CallIntMethod(e, in, in_read, buf, (jint)0, (jint)16384);
              if (threw(e) || r <= 0) break;
              if (r > TOMB_MAX - n) r = TOMB_MAX - n;
              (*e)->GetByteArrayRegion(e, buf, 0, r, (jbyte *)tomb + n);
              n += r;
            }
            (*e)->CallVoidMethod(e, in, in_close); threw(e);
          }
          int64_t made = acd_dump((coax_v_u8){tomb, n}, t, st, S(said), S(os), S(version), S(buildno), S(C.exe), (coax_v_u8){(uint8_t *)C.dump[C.dumps], DUMP_MAX});
          if (made > 0) { C.dump_n[C.dumps] = (int)made; C.dumps++; }
        }
      }
    }
    (*e)->PopLocalFrame(e, NULL);
  }
out:
  (*e)->PopLocalFrame(e, NULL);
  free(tomb);
  return read_ok;
}

/* Posts every dump. True when the server answered each one (kept or refused): they are done with.
   False when one got no answer or a "try later": the next launch tries again. */
static bool send_all(JNIEnv *e) {
  char url[320]; snprintf(url, sizeof url, "%s/api/playtest/app-crash", C.origin);
  static const char *const headers[] = { "Content-Type", "text/plain; charset=utf-8" };
  bool done = true; uint8_t answer[512];
  for (int i = 0; i < C.dumps; i++) {
    int n = 0;
    int got = oa_https(e, "POST", url, headers, 1, C.dump[i], C.dump_n[i], answer, (int)sizeof answer, &n);
    SAY("crash log %d of %d: status %d", i + 1, C.dumps, got);
    if (got <= 0 || got == 429 || got >= 500) done = false;
  }
  return done;
}

static void *main_loop(void *unused) {
  (void)unused;
  JNIEnv *e = NULL;
  /* (void *): the NDK's jni.h takes a JNIEnv ** here and a JDK's a void **; the check builds with a JDK's */
  if ((*C.vm)->AttachCurrentThread(C.vm, (void *)&e, NULL) != JNI_OK || !e) return NULL;
  int consent = consent_read(); int64_t seen = seen_read();
  bool read_ok = collect(e, seen);
  SAY("%d crash logs since the last launch handled (records %s)", C.dumps, read_ok ? "read" : "not read");
  if (C.dumps > 0) {
    if (consent == ASK) {
      /* the one question: the screen asks (oa_crash_asking), and the answer is kept */
      pthread_mutex_lock(&C.lock);
      C.asking = 1;
      while (C.answered == ASK) pthread_cond_wait(&C.wake, &C.lock);
      consent = C.answered; C.asking = 0;
      pthread_mutex_unlock(&C.lock);
      file_write("crash-consent", consent == YES ? "yes\n" : "no\n");
    }
    /* a no drops them; a yes sends them, and only an answered send is done with */
    if (consent == NO || send_all(e)) seen_write(C.newest);
  } else if (C.newest > seen) seen_write(C.newest);
  memset(C.dump, 0, sizeof C.dump); C.dumps = 0;
  (*C.vm)->DetachCurrentThread(C.vm);
  return NULL;
}

bool oa_crash_start(JavaVM *vm, jobject activity, const char *dir, const char *origin, const char *exe) {
  if (C.vm || !vm || !activity || !dir || !origin || !exe) return false;
  JNIEnv *e = NULL;
  if ((*vm)->GetEnv(vm, (void **)&e, JNI_VERSION_1_6) != JNI_OK || !e) return false;
  jobject held = (*e)->NewGlobalRef(e, activity);
  if (threw(e) || !held) return false;
  C.vm = vm; C.activity = held;
  snprintf(C.dir, sizeof C.dir, "%s", dir); snprintf(C.origin, sizeof C.origin, "%s", origin); snprintf(C.exe, sizeof C.exe, "%s", exe);
  pthread_t t;
  if (pthread_create(&t, NULL, main_loop, NULL) != 0) { (*e)->DeleteGlobalRef(e, held); C.vm = NULL; C.activity = NULL; return false; }
  pthread_detach(t);
  return true;
}
int oa_crash_asking(void) {
  pthread_mutex_lock(&C.lock); int a = C.asking && C.answered == ASK; pthread_mutex_unlock(&C.lock);
  return a;
}
void oa_crash_answer(bool send) {
  pthread_mutex_lock(&C.lock);
  if (C.answered == ASK) { C.answered = send ? YES : NO; pthread_cond_signal(&C.wake); }
  pthread_mutex_unlock(&C.lock);
}
__attribute__((noinline)) void oa_crash_force(void) {
  SAY("crashing on purpose (debug.oa.crash)");
  volatile int *nowhere = NULL;
  *nowhere = 13379;
}
