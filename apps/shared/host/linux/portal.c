// Firm reason: 6 (the desktop portal's D-Bus ABI).
#define _GNU_SOURCE
#include "portal.h"
#include <systemd/sd-bus.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define PORTAL_PATHS 64

struct Portal {
  sd_bus *bus;
  sd_bus_slot *slot;         // the open dialog's Response
  char handle[256];          // its request object
  int request, serial;
  char *paths[PORTAL_PATHS]; int npaths;   // what the Response named, until portal_poll hands it over
  int answered;
};

static int hex(char c) { return c >= '0' && c <= '9' ? c - '0' : c >= 'a' && c <= 'f' ? c - 'a' + 10 : c >= 'A' && c <= 'F' ? c - 'A' + 10 : -1; }
// file:///a%20b -> /a b; NULL for anything that is not a local file.
static char *uri_path(const char *uri) {
  if (strncmp(uri, "file://", 7)) return NULL;
  const char *s = uri + 7;
  if (*s != '/') { s = strchr(s, '/'); if (!s) return NULL; }   // a host name: skipped
  char *out = malloc(strlen(s) + 1), *o = out;
  if (!out) return NULL;
  for (; *s; s++) {
    if (*s == '%' && hex(s[1]) >= 0 && hex(s[2]) >= 0) { *o++ = (char)(hex(s[1]) * 16 + hex(s[2])); s += 2; }
    else *o++ = *s;
  }
  *o = 0;
  return out;
}
static int on_response(sd_bus_message *m, void *user, sd_bus_error *error) {
  Portal *p = user; (void)error;
  uint32_t code = 2;
  p->answered = 1;
  if (sd_bus_message_read(m, "u", &code) < 0 || code != 0) return 0;   // cancelled, or the portal ended it
  if (sd_bus_message_enter_container(m, 'a', "{sv}") < 0) return 0;
  while (sd_bus_message_enter_container(m, 'e', "sv") > 0) {
    const char *key = NULL;
    if (sd_bus_message_read(m, "s", &key) < 0) break;
    if (key && !strcmp(key, "uris") && sd_bus_message_enter_container(m, 'v', "as") >= 0) {
      if (sd_bus_message_enter_container(m, 'a', "s") >= 0) {
        const char *uri;
        while (sd_bus_message_read(m, "s", &uri) > 0) {
          char *path = uri_path(uri);
          if (path && p->npaths < PORTAL_PATHS) p->paths[p->npaths++] = path; else free(path);
        }
        sd_bus_message_exit_container(m);
      }
      sd_bus_message_exit_container(m);
    } else sd_bus_message_skip(m, "v");
    sd_bus_message_exit_container(m);
  }
  return 0;
}

Portal *portal_open(void) {
  Portal *p = calloc(1, sizeof *p);
  if (!p) return NULL;
  if (sd_bus_open_user(&p->bus) < 0) { free(p); return NULL; }
  return p;
}
int portal_pick(Portal *p, int request, int folder, const char *title, const char *start) {
  if (!p || p->slot) return 0;
  // The request's object is known before the call, from this connection's name and a token, so the
  // answer cannot be missed (the portal's documented way).
  const char *unique = NULL;
  if (sd_bus_get_unique_name(p->bus, &unique) < 0 || !unique || *unique != ':') return 0;
  char sender[128]; snprintf(sender, sizeof sender, "%s", unique + 1);
  for (char *c = sender; *c; c++) if (*c == '.') *c = '_';
  char token[64]; snprintf(token, sizeof token, "openagents%d_%d", (int)getpid(), ++p->serial);
  snprintf(p->handle, sizeof p->handle, "/org/freedesktop/portal/desktop/request/%s/%s", sender, token);
  if (sd_bus_match_signal(p->bus, &p->slot, "org.freedesktop.portal.Desktop", p->handle, "org.freedesktop.portal.Request", "Response", on_response, p) < 0) { p->slot = NULL; return 0; }
  sd_bus_message *m = NULL, *reply = NULL;
  sd_bus_error error = SD_BUS_ERROR_NULL;
  int ok = sd_bus_message_new_method_call(p->bus, &m, "org.freedesktop.portal.Desktop", "/org/freedesktop/portal/desktop", "org.freedesktop.portal.FileChooser", "OpenFile") >= 0
    && sd_bus_message_append(m, "ss", "", title ? title : "") >= 0
    && sd_bus_message_open_container(m, 'a', "{sv}") >= 0
    && sd_bus_message_append(m, "{sv}", "handle_token", "s", token) >= 0
    && sd_bus_message_append(m, "{sv}", "multiple", "b", !folder) >= 0
    && sd_bus_message_append(m, "{sv}", "directory", "b", folder) >= 0;
  if (ok && start && *start) {   // where the dialog opens: the folder's bytes with their ending zero, as the portal takes it
    ok = sd_bus_message_open_container(m, 'e', "sv") >= 0 && sd_bus_message_append(m, "s", "current_folder") >= 0
      && sd_bus_message_open_container(m, 'v', "ay") >= 0 && sd_bus_message_append_array(m, 'y', start, strlen(start) + 1) >= 0
      && sd_bus_message_close_container(m) >= 0 && sd_bus_message_close_container(m) >= 0;
  }
  ok = ok && sd_bus_message_close_container(m) >= 0 && sd_bus_call(p->bus, m, 5000000, &error, &reply) >= 0;
  if (!ok) fprintf(stderr, "portal: the desktop has no file chooser (%s)\n", error.message ? error.message : "xdg-desktop-portal did not answer");
  sd_bus_error_free(&error); sd_bus_message_unref(m); sd_bus_message_unref(reply);
  if (!ok) { sd_bus_slot_unref(p->slot); p->slot = NULL; return 0; }
  p->request = request; p->answered = 0;
  return 1;
}
void portal_poll(Portal *p, PortalChosen chosen, void *user) {
  if (!p) return;
  while (sd_bus_process(p->bus, NULL) > 0) {}
  if (!p->answered) return;
  p->answered = 0;
  sd_bus_slot_unref(p->slot); p->slot = NULL;
  int n = p->npaths; p->npaths = 0;
  for (int i = 0; i < n; i++) { if (chosen) chosen(user, p->request, p->paths[i]); free(p->paths[i]); }
}
void portal_cancel(Portal *p) {
  if (!p || !p->slot) return;
  sd_bus_call_method(p->bus, "org.freedesktop.portal.Desktop", p->handle, "org.freedesktop.portal.Request", "Close", NULL, NULL, "");
  sd_bus_slot_unref(p->slot); p->slot = NULL;
}
void portal_close(Portal *p) {
  if (!p) return;
  portal_cancel(p);
  for (int i = 0; i < p->npaths; i++) free(p->paths[i]);
  sd_bus_flush_close_unref(p->bus);
  free(p);
}
