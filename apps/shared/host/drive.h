// Firm reason: 6 (a Unix socket and the frame clock; the operations' answers are the app's Coax).
// coax drive on the shared hosts (docs/research/exact/04-apps-audit-2026-11-10.md, recommendations 1
// and 3; GitHub #13156): the host's half of the driver, the same on Linux (linux/ui-host.c) and the
// Mac (apple/ui-host.m), which both include drive.c. Internal to the hosts: an app sees ui-host.h.
#ifndef UI_DRIVE_H
#define UI_DRIVE_H
#include <stdint.h>

#define UI_DRIVE_LINE 8192
typedef struct UiDrive {
  int agent;          // the agent clock: frames get no time but what `clock` gives them
  int real_time;      // `clock +MS` also waits MS of real time (a recorder that runs on the wall clock)
  float giving;       // the time the frame being stepped gets, under the agent clock
  double clock_ms;    // the agent clock's time since it started
  int listen_fd, conn_fd;   // the socket, and the driver connected to it (-1 none)
  char line[UI_DRIVE_LINE]; int line_n;
  int quit;           // `quit` was asked
} UiDrive;
#endif
