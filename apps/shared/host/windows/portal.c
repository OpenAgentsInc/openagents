// Firm reason: 6 (the Windows file dialog's COM ABI).
// The file chooser on Windows (GitHub #13782): the system's file dialog (IFileOpenDialog) behind
// the Linux portal's contract (../linux/portal.h). The dialog is modal: portal_pick shows it and
// keeps what was chosen, and portal_poll hands that over on the next frame, as Linux's does when
// the portal answers. Paths come back with forward slashes, as the desktop core keeps them.
#define COBJMACROS
#define WIN32_LEAN_AND_MEAN
#include "portal.h"
#include <windows.h>
#include <shobjidl.h>
#include <stdlib.h>
#include <string.h>

#define PORTAL_PATHS 64

struct Portal {
  int request;
  char *paths[PORTAL_PATHS]; int npaths;
  int com;   // COM was started here (and is ended in portal_close)
};

// The dialog's class and interface (shobjidl.h names them; their GUIDs are fixed).
static const CLSID portal_clsid = {0xDC1C5A9C, 0xE88A, 0x4DDE, {0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7}};
static const IID portal_iid = {0xD57C7288, 0xD4AD, 0x4768, {0xBE, 0x02, 0x9D, 0x96, 0x95, 0x32, 0xD9, 0x60}};
static const IID portal_item_iid = {0x43826D1E, 0xE718, 0x42EE, {0xBC, 0x55, 0xA1, 0xE2, 0x61, 0xC3, 0x7B, 0xFE}};

static WCHAR *portal_wide(const char *s) {
  int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
  WCHAR *w = n > 0 ? malloc((size_t)n * sizeof *w) : NULL;
  if (w) MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n);
  return w;
}
static char *portal_utf8(const WCHAR *w) {
  int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
  char *s = n > 0 ? malloc((size_t)n) : NULL;
  if (!s) return NULL;
  WideCharToMultiByte(CP_UTF8, 0, w, -1, s, n, NULL, NULL);
  for (char *c = s; *c; c++) if (*c == '\\') *c = '/';
  return s;
}

Portal *portal_open(void) {
  Portal *p = calloc(1, sizeof *p);
  if (!p) return NULL;
  HRESULT r = CoInitializeEx(NULL, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
  p->com = r == S_OK || r == S_FALSE;
  return p;
}
static void portal_keep(Portal *p, IShellItem *item) {
  WCHAR *name = NULL;
  if (p->npaths < PORTAL_PATHS && SUCCEEDED(IShellItem_GetDisplayName(item, SIGDN_FILESYSPATH, &name)) && name) {
    char *path = portal_utf8(name);
    if (path) p->paths[p->npaths++] = path;
  }
  if (name) CoTaskMemFree(name);
}
int portal_pick(Portal *p, int request, int folder, const char *title, const char *start) {
  if (!p) return 0;
  IFileOpenDialog *dialog = NULL;
  if (FAILED(CoCreateInstance(&portal_clsid, NULL, CLSCTX_INPROC_SERVER, &portal_iid, (void **)&dialog)) || !dialog) return 0;
  FILEOPENDIALOGOPTIONS options = 0;
  IFileOpenDialog_GetOptions(dialog, &options);
  options |= FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | (folder ? FOS_PICKFOLDERS : FOS_ALLOWMULTISELECT | FOS_FILEMUSTEXIST);
  IFileOpenDialog_SetOptions(dialog, options);
  WCHAR *wtitle = title ? portal_wide(title) : NULL;
  if (wtitle) { IFileOpenDialog_SetTitle(dialog, wtitle); free(wtitle); }
  if (start && *start) {
    WCHAR *wstart = portal_wide(start);
    for (WCHAR *c = wstart; c && *c; c++) if (*c == L'/') *c = L'\\';
    IShellItem *where = NULL;
    if (wstart && SUCCEEDED(SHCreateItemFromParsingName(wstart, NULL, &portal_item_iid, (void **)&where)) && where) {
      IFileOpenDialog_SetFolder(dialog, where);
      IShellItem_Release(where);
    }
    free(wstart);
  }
  if (SUCCEEDED(IFileOpenDialog_Show(dialog, GetActiveWindow()))) {
    IShellItemArray *items = NULL;
    if (SUCCEEDED(IFileOpenDialog_GetResults(dialog, &items)) && items) {
      DWORD count = 0;
      IShellItemArray_GetCount(items, &count);
      for (DWORD i = 0; i < count; i++) {
        IShellItem *item = NULL;
        if (SUCCEEDED(IShellItemArray_GetItemAt(items, i, &item)) && item) { portal_keep(p, item); IShellItem_Release(item); }
      }
      IShellItemArray_Release(items);
    }
    p->request = request;
  }
  IFileOpenDialog_Release(dialog);
  return 1;
}
void portal_poll(Portal *p, PortalChosen chosen, void *user) {
  if (!p) return;
  int n = p->npaths;
  p->npaths = 0;
  for (int i = 0; i < n; i++) { if (chosen) chosen(user, p->request, p->paths[i]); free(p->paths[i]); }
}
void portal_cancel(Portal *p) { (void)p; }   // the dialog is closed before portal_pick returns
void portal_close(Portal *p) {
  if (!p) return;
  for (int i = 0; i < p->npaths; i++) free(p->paths[i]);
  if (p->com) CoUninitialize();
  free(p);
}
