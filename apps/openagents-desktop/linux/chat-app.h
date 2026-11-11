// Firm reason: 6 (the chat's state as the OpenAgents app's own host reads it; every decision is Coax).
// chat.h's component inside: what linux/main.c, the OpenAgents app, reaches past the component's calls
// for (its menu bar, its --do steps). An app that holds the chat uses chat.h alone.
#ifndef OA_CHAT_APP_H
#define OA_CHAT_APP_H
#ifndef _GNU_SOURCE
#define _GNU_SOURCE   // accept4 and the rest: include this first
#endif
#include <stdarg.h>
#include "zeron.h"
#include "face.h"
#ifdef __APPLE__
#include "ui-host-apple.h"
#include "ct-atlas.h"
#include "dictation-mac.h"
#include <mach-o/dyld.h>
#else
#include "ui-host.h"
#include "portal.h"
#endif
#include "dictation-host.h"
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <time.h>
#include <unistd.h>

#include "chat.h"

#define MAX_CHATS 64

typedef struct { void *state; uint8_t *mono, *color; } ChatAtlas;
struct OaChat {
  UiHost *host;
  void *desk, *settings, *first;
  char resources[PATH_MAX];
  // the atlas: the fonts' bytes, the plan, the rasterizer's working memory, and each chat's pixels
  coax_t_RaFonts fonts;
  coax_t_RaPlan plan;
  coax_t_RaWork *work;
  float sizes[16], icon_sizes[64];
  int64_t weights[16];
  int nfonts, nicons;
  char *svg[64]; size_t svg_n[64];
  ChatAtlas atlases[MAX_CHATS];
#ifdef __APPLE__
  CtAtlas *ct;        // CoreText's fonts and icons, once; a chat's mono pixels are its cache rows only
  uint8_t *full;      // the mono image the GPU holds: the fonts and icons, and the bound chat's cache rows
  int quit;           // Quit was chosen
#endif
  int natlases;
  void *bound;        // the chat whose atlas the GPU holds
  float dpr;
  // settings.json (zeron/settings.coax) and sessions/<id>.jsonl beside it (zeron/store.coax); empty: nothing is saved
  char settings_path[PATH_MAX];
  uint32_t w, h;
  // the room's socket (zeron/room.coax): the listener, and each connection with the bytes of its unfinished line
  char room_path[108];
  int room_fd, room_conns[16], room_n[16];
  char room_buf[16][65536];
#ifndef __APPLE__
  Portal *portal;     // the desktop's file chooser; NULL offscreen and where there is no session bus
#endif
  // dictation: the recorder and the recognizer (NULL for a picture with neither --dictation-fixture nor
  // --dictation-recorder), the request in flight (ticket 0: none) with the chat it began in, and the
  // controller's facts as last said
  Dictation *dictation;
  uint64_t dictation_ticket;
  void *dictation_chat;
  int dictation_phase, dictation_outcome, dictation_fault, dictation_preparing;
  int dictation_key;        // D is down
  int pointer_down;         // the left button is down
  int focused;              // the window has the keyboard
  int offscreen;            // a picture, not a window
  int dictation_fixture;    // --dictation-fixture: the samples come from coax drive's `dictation feed`, on this clock (ms)
  int64_t dictation_clock;
  int setting_changed;      // an option changed settings.json before the window opened: written once it has
};
typedef struct OaChat App;

void *oac_active(App *a);                       // the shown chat
uint8_t *oac_read_file(const char *path, size_t *n);
void oac_attach(App *a, const char *path);
void oac_new_session(App *a, const char *cwd);
void oac_cancel_dictation_for_edit(App *a);
void oac_sync_dictation(App *a);
void oac_set_dictation(App *a, int on);
void oac_copy_selection(App *a);
void oac_pick_folder(App *a);
void oac_save_settings(App *a);
#ifdef __APPLE__
void oac_paste(App *a);
void oac_ime_replace(void *user, int from, int to, const char *bytes, size_t n);
void oac_ime_mark(void *user, int from, int to, const char *bytes, size_t n, int sf, int st);
extern const UiIme oac_ime;   // the input method over the composer
#endif
#endif
