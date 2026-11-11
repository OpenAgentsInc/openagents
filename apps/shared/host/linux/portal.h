// Firm reason: 6 (the desktop portal's D-Bus ABI).
// The desktop's file chooser for apps on the shared Linux host: xdg-desktop-portal's
// FileChooser, asked over the session bus (libsystemd's sd-bus), so the dialog is the
// desktop's own. Nothing blocks: portal_pick asks, portal_poll is called every frame and
// hands over what was chosen when the person has answered.
#ifndef UI_PORTAL_H
#define UI_PORTAL_H
typedef struct Portal Portal;
// Each chosen file or folder, as a path. Called from portal_poll.
typedef void (*PortalChosen)(void *user, int request, const char *path);
Portal *portal_open(void);                 // NULL when there is no session bus
// Asks for files (folder 0, several allowed) or one folder (folder 1). `request` comes back with each
// path. 0 when it could not ask, or while an earlier dialog is still open.
int portal_pick(Portal *p, int request, int folder, const char *title, const char *start);
void portal_poll(Portal *p, PortalChosen chosen, void *user);
// Closes a dialog that is still open (the app is quitting).
void portal_cancel(Portal *p);
void portal_close(Portal *p);
#endif
