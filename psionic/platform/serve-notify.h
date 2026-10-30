#ifndef PSIONIC_SERVE_NOTIFY_H
#define PSIONIC_SERVE_NOTIFY_H
/* Optional systemd effect, called only after Coax admits readiness and bind.
 * No systemd library/runtime is linked. Returns 0 without NOTIFY_SOCKET. */
int psionic_notify_ready(void);
#endif
