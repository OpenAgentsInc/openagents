// Firm reason: 6 (a socket, a thread and the main queue; the operations' answers are the app's Coax).
// coax drive for the iPhone app (The OpenAgents Book, chapter 6; coax/native.md, stage 1; GitHub #13516,
// #13517): apps/shared/host/drive.c on a Unix socket (the simulator) or TCP (a physical iPhone), each
// operation answered on the main thread by the app.
#ifndef PHONE_DRIVE_H
#define PHONE_DRIVE_H
// The app's answer to one operation, on the main thread: its length in out (at most cap bytes), or -1
// for one it does not know.
typedef int (*PhoneDriveAnswer)(const char *op, char *out, int cap);
// One frame of the agent clock (`clock +MS` steps them), on the main thread.
typedef void (*PhoneDriveStep)(float dt);
// Listens on where and answers on a thread of its own, until the driver says quit. where is a socket path
// (a simulator's app shares the Mac's file system; token unused), or tcp:PORT, which needs a token of 32 to
// 128 characters and admits a driver on the phone's own network that sends `attach TOKEN` first.
// 0 if it could not listen.
int phone_drive_start(const char *where, const char *token, PhoneDriveAnswer answer, PhoneDriveStep step);
#endif
