/* The Android app's crash reporter (GitHub #13379; docs/deployment/crash-reports.md).

   Android keeps a record of how each of the app's processes ended (ApplicationExitInfo) and, for a
   native crash, its tombstone. On a launch this reads the records since the last one it handled,
   makes each native crash's dump with Coax (crash-dump.coax: the lines `appdump 1` describes in
   proto/verse/crash-report-v3.json, `app android`), asks the person once, and posts each dump to
   POST /api/playtest/app-crash. The API makes the crash report with the one codec
   (coax/web/room/room-crash.coax).

   Nothing leaves the phone before the person says yes. The first crash raises one question, drawn
   by the app's own screen (apps/everglade-desktop/crash-ask.coax); the answer is kept in the app's
   files (`crash-consent`), and a no also drops the dumps. `crash-seen` holds the time of the newest
   crash handled, so a crash is sent once. A dump the server could not be reached for is tried again
   on the next launch.

   Nothing here logs a dump. */
#pragma once
#include <jni.h>
#include <stdbool.h>

/* Starts the reporter's thread. activity: the NativeActivity (ANativeActivity's clazz); dir: the
   app's files folder (internalDataPath); origin: "https://openagents.com", or another for
   development; exe: the app's own library's file name ("libeverglade.so"). */
bool oa_crash_start(JavaVM *vm, jobject activity, const char *dir, const char *origin, const char *exe);
/* 1 while a crash log waits for the person's word: the screen asks. */
int oa_crash_asking(void);
/* The person's word. It is kept, and never asked for again. */
void oa_crash_answer(bool send);
/* Crashes the app on purpose, for the proof (`adb shell setprop debug.oa.crash 1`): a write through
   a null pointer in a function of this name, so the report's innermost frame resolves to it. */
void oa_crash_force(void);
