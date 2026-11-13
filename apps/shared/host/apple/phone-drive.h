// Firm reason: 6 (a Unix socket, a thread and the main queue; the operations' answers are the app's Coax).
// coax drive for the iPhone app (The OpenAgents Book, chapter 6; coax/native.md, stage 1; GitHub #13516):
// apps/shared/host/drive.c on a socket path, each operation answered on the main thread by the app.
#ifndef PHONE_DRIVE_H
#define PHONE_DRIVE_H
// The app's answer to one operation, on the main thread: its length in out (at most cap bytes), or -1
// for one it does not know.
typedef int (*PhoneDriveAnswer)(const char *op, char *out, int cap);
// One frame of the agent clock (`clock +MS` steps them), on the main thread.
typedef void (*PhoneDriveStep)(float dt);
// Listens on path (a simulator's app shares the Mac's file system) and answers on a thread of its own,
// until the driver says quit. 0 if the socket could not be made.
int phone_drive_start(const char *path, PhoneDriveAnswer answer, PhoneDriveStep step);
#endif
