// Firm reason: 6 (the chat's process effects, files and atlas; every decision is Coax).
// The OpenAgents chat as a component any app on the shared host can hold (GitHub #13156): the
// Zeron-fork chat screen (zeron/screen.coax, zeron/desk.coax) with its sessions, settings and
// session files, each chat's glyph atlas, the room's socket, attachments and dictation. The
// OpenAgents app (linux/main.c) is this component filling its window; Everglade's Workshop
// (apps/everglade-desktop) holds it in a panel beside the scene.
//
// The component owns the shared host's component pass and atlas (ui_atlas): an app that holds it
// draws its own pictures in its layer (ui-host-apple.h ui_layer), not with the components.
// It draws from the window's top left, w by h pixels, so a holder that gives it less than the
// window keeps it on the left and sends it the input that lands there.
//
// Order: oa_chat_new before ui_open (the first events come while the window opens), then
// oa_chat_start with the host; every other call is from the window's thread.
#ifndef OA_CHAT_H
#define OA_CHAT_H
#include <stddef.h>
#include <stdint.h>

typedef struct OaChat OaChat;
typedef struct UiHost UiHost;

typedef struct {
  const char *resources;     // fonts/ and icons/ (components/zeron/), beside the program or in the bundle
  const char *settings;      // settings.json, with sessions/ beside it; NULL or "": nothing is kept
  const char *cwd;           // the folder a first session works in; NULL: $HOME
  const char *skin;          // NULL, "zeron", "everglade" or "verse"
  float dpr;                 // pixels a point to start at (the host's scale replaces it in a window)
  int offscreen;             // a picture, not a window
  int demo;                  // the demo transcript, nothing kept
  int still_clock;           // a still clock, for pictures
  int room;                  // listen on the room's socket (the program must answer --mcp-room)
  // dictation: "on" or "off" kept in settings.json, NULL as kept; none at all unless dictation is set
  const char *dictation_setting;
  int dictation;             // run dictation: a window's recorder, or for a picture a fixture or a recorder
  int dictation_fixture;     // a picture's fixture clock and samples (no recorder)
  const char *dictation_recorder, *dictation_stand_in;
} OaChatOptions;

OaChat *oa_chat_new(const OaChatOptions *options);   // NULL: no memory, or no skin of that name
int oa_chat_start(OaChat *c, UiHost *host);          // the atlas, the sessions as kept and dictation; 0 failed
uint32_t oa_chat_max_vertices(void);                 // what a frame may hold (28 floats each)
int oa_chat_frame(OaChat *c, float *vertices, float w, float h, float dpr, float dt);
void oa_chat_pointer(OaChat *c, float x, float y, int phase, int clicks, int mods);
void oa_chat_wheel(OaChat *c, float dy);
int oa_chat_key(OaChat *c, int key, int mods, int down);
void oa_chat_text(OaChat *c, const char *bytes, size_t n);
int oa_chat_wants_text(OaChat *c);
void oa_chat_file(OaChat *c, int request, const char *path);   // a file chosen, dropped or pasted (UiApp.file)
int oa_chat_live(OaChat *c);                         // the shown chat's turn is running
void oa_chat_close(OaChat *c);                       // the sessions written, dictation and the room closed
// coax drive (apps/shared/host/drive.c, UiApp.drive): one of the chat's operations, its answer in out (at most
// cap bytes): the answer's length, or -1 for an operation it does not know. Targets are the element table's ids
// (`tree`):   tree | tap ID [CLICKS] | shift-tap ID | hover ID | drag ID DX,DY | type ID TEXT | type ID key CHORD |
//   type - TEXT | key CHORD | wheel DY | send TEXT | newchat | fixture open|closed|table | sidebar | room | closetab |
//   folder PATH | attach PATH | mode|model|effort|access NAME | plan on|off | open model|access | answer allow|deny |
//   stop | scroll PX | dictation on|off|down|up|mousedown|mouseup|cancel|feed FILE | state status|last|desk|draft|dictation
int oa_chat_drive(OaChat *c, const char *op, char *out, int cap);
// 1 while something is coming: a turn in any session, dictation's model loading, a request's words (UiApp.busy).
int oa_chat_busy(OaChat *c);
// --mcp-room SOCKET AGENT: the room's MCP server, which Codex starts as this program again.
int oa_chat_mcp_room(const char *path, int agent);
#endif
