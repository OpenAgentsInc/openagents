/* crash-reports-check.sh's driver: crash-reports.c and https.c as the app builds them, run in a desktop
   JVM against stand-ins for the Android classes they call (check/stubs). It plays one launch:
     crash-reports-check CLASSPATH DIR ORIGIN RECORDS ANSWER WAIT_SEEN
   RECORDS: the exit records (check/stubs/android/app/ActivityManager.java). ANSWER: send, dont or
   none (what the person presses if asked). WAIT_SEEN: the crash-seen value to wait for, or - to wait
   two seconds. Prints "asked" or "not asked", then "seen VALUE" and "consent VALUE". */
#include "../crash-reports.h"
#include <jni.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static void nap(int ms) { struct timespec t = { ms / 1000, (ms % 1000) * 1000000L }; nanosleep(&t, NULL); }
static void slurp(const char *dir, const char *name, char *out, size_t cap) {
  char p[700]; snprintf(p, sizeof p, "%s/%s", dir, name); out[0] = 0;
  FILE *f = fopen(p, "r"); if (!f) return;
  size_t n = fread(out, 1, cap - 1, f); fclose(f); out[n] = 0;
  while (n > 0 && (out[n - 1] == '\n' || out[n - 1] == ' ')) out[--n] = 0;
}
int main(int argc, char **argv) {
  if (argc != 7) { fprintf(stderr, "usage: crash-reports-check CLASSPATH DIR ORIGIN RECORDS ANSWER WAIT_SEEN\n"); return 2; }
  char cp[1024], records[4096]; snprintf(cp, sizeof cp, "-Djava.class.path=%s", argv[1]); snprintf(records, sizeof records, "-Doa.check.records=%s", argv[4]);
  JavaVMOption options[2] = { { cp, NULL }, { records, NULL } };
  JavaVMInitArgs args = { JNI_VERSION_1_6, 2, options, JNI_FALSE };
  JavaVM *vm = NULL; JNIEnv *e = NULL;
  if (JNI_CreateJavaVM(&vm, (void **)&e, &args) != JNI_OK) { fprintf(stderr, "no JVM\n"); return 2; }
  jclass c = (*e)->FindClass(e, "android/app/NativeActivity");
  jobject activity = c ? (*e)->NewObject(e, c, (*e)->GetMethodID(e, c, "<init>", "()V")) : NULL;
  if (!activity) { fprintf(stderr, "no activity\n"); return 2; }
  if (!oa_crash_start(vm, activity, argv[2], argv[3], "libeverglade.so")) { fprintf(stderr, "the reporter did not start\n"); return 2; }
  if (oa_crash_start(vm, activity, argv[2], argv[3], "libeverglade.so")) { fprintf(stderr, "the reporter started twice\n"); return 2; }
  int asked = 0; char seen[64] = "";
  for (int i = 0; i < 400; i++) {   /* 20 s at most */
    if (!asked && oa_crash_asking()) {
      asked = 1;
      nap(300);   /* a person takes a moment: nothing may leave in it (the script looks at the server after) */
      if (strcmp(argv[5], "send") == 0) oa_crash_answer(true); else if (strcmp(argv[5], "dont") == 0) oa_crash_answer(false);
      else break;
    }
    slurp(argv[2], "crash-seen", seen, sizeof seen);
    if (strcmp(argv[6], "-") == 0 ? i >= 40 : strcmp(seen, argv[6]) == 0) break;
    nap(50);
  }
  nap(200);
  char consent[16]; slurp(argv[2], "crash-seen", seen, sizeof seen); slurp(argv[2], "crash-consent", consent, sizeof consent);
  printf("%s\nseen %s\nconsent %s\n", asked ? "asked" : "not asked", seen[0] ? seen : "-", consent[0] ? consent : "-");
  fflush(stdout);
  _Exit(0);   /* the reporter's thread may still wait for an answer that never comes */
}
