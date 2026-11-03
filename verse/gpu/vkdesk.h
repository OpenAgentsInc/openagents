// The desk panel (docs/os/2026-11-02-verse-os.md, V3): Linux programs on a headless Coax compositor
// (os/compositor.coax) shown over the realm. vk-realm.c --desk DIR starts DIR/compositor as a child
// on $XDG_RUNTIME_DIR/wayland-verse (WAYLAND_DISPLAY for the programs it launches from DIR/launch),
// asks it for what changed (`frame PATH`, its test channel) up to thirty times a second while the
// panel shows, uploads the picture, and blits it into the panel's rectangle on the swapchain image
// after the realm's frame (vkp's overlay). Keys and the pointer go to it as `keydown`, `button`
// and `move` while it shows. Every decision (open, docked or flat, where the panel goes, who gets
// a key, where a point lands) is os/verse-panel.coax's (verse-panel.h); this does the effects.
// The panel's image and staging buffer are the presenter's, like the swapchain, not the shared
// renderer's (INVARIANTS.md, "A shared-renderer GPU object exists only with a live ledger handle").
// Included once, by vk-realm.c.
#ifndef VKDESK_H
#define VKDESK_H
#include "vkc.h"
#include "vkp.h"
#include "verse-panel.h"
#include <errno.h>
#include <fcntl.h>
#include <spawn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

extern char **environ;
// os/verse-panel.coax's modes and routes
#define PANEL_CLOSED verse_panel_closed()
#define PANEL_FLAT verse_panel_flat()
#define PANEL_TOGGLE verse_panel_toggle()
#define PANEL_TO_DESK verse_panel_to_desk()

typedef struct {
  pid_t pid;
  int to, from;               // the compositor's stdin and stdout
  char line[8192]; size_t line_n;
  int ready, pending;
  double asked_at;
  char frame_path[1100];
  uint32_t dw, dh;            // the desk's size
  VkImage image; VkDeviceMemory mem; VkcBuffer staging;
  int dirty, has_frame, image_ready;
  int64_t mode;               // PANEL_CLOSED, PANEL_DOCKED, PANEL_FLAT
  uint32_t sw, sh;            // the screen it is shown on, for the pointer
} VkDesk;

static VkDesk *vkdesk_;

// Opens the compositor and the panel's image; 0, or -1 (the realm runs on without a desk).
static int vkdesk_open(Vkc *v, VkDesk *d, const char *dir, uint32_t dw, uint32_t dh) {
  memset(d, 0, sizeof *d);
  d->pid = -1; d->dw = dw; d->dh = dh; d->mode = PANEL_CLOSED;
  if (dw < 64 || dw > 4096 || dh < 64 || dh > 4096) { fprintf(stderr, "vk-realm: a desk is 64 to 4096 pixels a side\n"); return -1; }
  const char *rt = getenv("XDG_RUNTIME_DIR");
  if (!rt) { fprintf(stderr, "vk-realm: no XDG_RUNTIME_DIR for the desk\n"); return -1; }
  char program[1100], keymap[1100], launch[1100], socket[1100], wd[16], hd[16];
  snprintf(program, sizeof program, "%s/compositor", dir);
  snprintf(keymap, sizeof keymap, "%s/compositor-us.xkb", dir);
  snprintf(launch, sizeof launch, "%s/launch", dir);
  snprintf(socket, sizeof socket, "%s/wayland-verse", rt);
  snprintf(d->frame_path, sizeof d->frame_path, "%s/verse-desk.ppm", rt);
  snprintf(wd, sizeof wd, "%u", dw); snprintf(hd, sizeof hd, "%u", dh);
  if (access(program, X_OK)) { fprintf(stderr, "vk-realm: no desk compositor at %s\n", program); return -1; }
  int in[2], out[2];
  if (pipe2(in, O_CLOEXEC) || pipe2(out, O_CLOEXEC)) return -1;
  posix_spawn_file_actions_t fa;
  posix_spawn_file_actions_init(&fa);
  posix_spawn_file_actions_adddup2(&fa, in[0], 0);
  posix_spawn_file_actions_adddup2(&fa, out[1], 1);
  char *argv[] = {program, socket, keymap, launch, wd, hd, "--xwayland", NULL};
  int e = posix_spawn(&d->pid, program, &fa, NULL, argv, environ);
  posix_spawn_file_actions_destroy(&fa);
  close(in[0]); close(out[1]);
  if (e) { close(in[1]); close(out[0]); d->pid = -1; fprintf(stderr, "vk-realm: the desk did not start\n"); return -1; }
  d->to = in[1]; d->from = out[0];
  fcntl(d->from, F_SETFL, fcntl(d->from, F_GETFL) | O_NONBLOCK);
  // the panel's picture: BGRA as the swapchain is, filled from the staging buffer by a copy
  VkImageCreateInfo ii = {.sType = VK_STRUCTURE_TYPE_IMAGE_CREATE_INFO, .imageType = VK_IMAGE_TYPE_2D, .format = VK_FORMAT_B8G8R8A8_UNORM,
                          .extent = {dw, dh, 1}, .mipLevels = 1, .arrayLayers = 1, .samples = VK_SAMPLE_COUNT_1_BIT,
                          .tiling = VK_IMAGE_TILING_OPTIMAL, .usage = VK_IMAGE_USAGE_TRANSFER_DST_BIT | VK_IMAGE_USAGE_TRANSFER_SRC_BIT};
  if (vkCreateImage(v->dev, &ii, NULL, &d->image) != VK_SUCCESS) return -1;
  VkMemoryRequirements req;
  vkGetImageMemoryRequirements(v->dev, d->image, &req);
  d->mem = vkc_memory(v, req, VK_MEMORY_PROPERTY_DEVICE_LOCAL_BIT);
  vkBindImageMemory(v->dev, d->image, d->mem, 0);
  d->staging = vkc_buffer(v, (VkDeviceSize)dw * dh * 4);
  vkdesk_ = d;
  return 0;
}

static void vkdesk_say(VkDesk *d, const char *line) {
  if (d->pid <= 0) return;
  size_t n = strlen(line), done = 0;
  while (done < n) { ssize_t w = write(d->to, line + done, n - done); if (w < 0 && errno == EINTR) continue; if (w <= 0) return; done += (size_t)w; }
}

// The picture the compositor wrote (a P6 PPM, the desk's size), into the staging buffer as BGRA.
static int vkdesk_load(VkDesk *d) {
  FILE *f = fopen(d->frame_path, "rb");
  if (!f) return 0;
  unsigned w = 0, h = 0, max = 0;
  int ok = fscanf(f, "P6 %u %u %u", &w, &h, &max) == 3 && fgetc(f) != EOF && w == d->dw && h == d->dh && max == 255;
  if (ok) {
    uint8_t *px = d->staging.map, row[3 * 4096];
    for (unsigned y = 0; y < h && ok; y++) {
      if (fread(row, 3, w, f) != w) { ok = 0; break; }
      for (unsigned x = 0; x < w; x++) { uint8_t *o = px + ((size_t)y * w + x) * 4; o[0] = row[3 * x + 2]; o[1] = row[3 * x + 1]; o[2] = row[3 * x]; o[3] = 255; }
    }
  }
  fclose(f);
  return ok;
}

// Once a frame, after the fence (the staging buffer is free): the compositor's answers, and the
// next picture asked for while the panel shows.
static void vkdesk_pump(VkDesk *d, double now) {
  if (d->pid <= 0) return;
  char buf[4096];
  ssize_t n;
  while ((n = read(d->from, buf, sizeof buf)) > 0) {
    for (ssize_t i = 0; i < n; i++) {
      if (buf[i] != '\n') { if (d->line_n < sizeof d->line - 1) d->line[d->line_n++] = buf[i]; continue; }
      d->line[d->line_n] = 0; d->line_n = 0;
      if (!strcmp(d->line, "ready")) d->ready = 1;
      else if (!strncmp(d->line, "frame ", 6) || !strcmp(d->line, "shot")) {
        d->pending = 0;
        if (strcmp(d->line, "frame nothing") && !strstr(d->line, "failed") && vkdesk_load(d)) { d->dirty = 1; d->has_frame = 1; }
      }
    }
  }
  if (n == 0) { fprintf(stderr, "vk-realm: the desk compositor ended\n"); close(d->from); close(d->to); d->pid = -1; return; }
  if (d->pending && now - d->asked_at > 2) d->pending = 0;   // an answer lost: ask again
  if (d->ready && d->mode != PANEL_CLOSED && !d->pending && now - d->asked_at >= 1.0 / 30) {
    char cmd[1200];
    // the first picture is all of it; after that, what changed
    snprintf(cmd, sizeof cmd, "%s %s\n", d->has_frame ? "frame" : "shot", d->frame_path);
    vkdesk_say(d, cmd);
    d->pending = 1; d->asked_at = now;
  }
}

static void vkdesk_barrier(VkCommandBuffer cmd, VkImage image, VkImageLayout from, VkImageLayout to, VkAccessFlags a, VkAccessFlags b) {
  VkImageMemoryBarrier m = {.sType = VK_STRUCTURE_TYPE_IMAGE_MEMORY_BARRIER, .srcAccessMask = a, .dstAccessMask = b, .oldLayout = from, .newLayout = to,
                            .srcQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .dstQueueFamilyIndex = VK_QUEUE_FAMILY_IGNORED, .image = image,
                            .subresourceRange = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1}};
  vkCmdPipelineBarrier(cmd, VK_PIPELINE_STAGE_TRANSFER_BIT | VK_PIPELINE_STAGE_HOST_BIT, VK_PIPELINE_STAGE_TRANSFER_BIT, 0, 0, NULL, 0, NULL, 1, &m);
}

// vkp's overlay: the new picture uploaded, then the panel blitted into its rectangle (flat: the
// screen cleared first, so nothing of the realm shows round the desk).
static void vkdesk_overlay(VkCommandBuffer cmd, VkImage dst, uint32_t w, uint32_t h, void *user) {
  VkDesk *d = user;
  d->sw = w; d->sh = h;
  if (d->mode == PANEL_CLOSED || !d->has_frame) return;
  if (d->dirty) {
    vkdesk_barrier(cmd, d->image, d->image_ready ? VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL : VK_IMAGE_LAYOUT_UNDEFINED, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL,
                   d->image_ready ? VK_ACCESS_TRANSFER_READ_BIT : 0, VK_ACCESS_TRANSFER_WRITE_BIT);
    VkBufferImageCopy copy = {.imageSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .imageExtent = {d->dw, d->dh, 1}};
    vkCmdCopyBufferToImage(cmd, d->staging.buf, d->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &copy);
    vkdesk_barrier(cmd, d->image, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, VK_ACCESS_TRANSFER_WRITE_BIT, VK_ACCESS_TRANSFER_READ_BIT);
    d->dirty = 0; d->image_ready = 1;
  }
  if (!d->image_ready) return;
  int32_t r[4];
  for (int k = 0; k < 4; k++) r[k] = (int32_t)verse_panel_rect(d->mode, w, h, d->dw, d->dh, k);
  if (r[2] <= r[0] || r[3] <= r[1]) return;
  if (d->mode == PANEL_FLAT) {
    VkClearColorValue black = {.float32 = {0, 0, 0, 1}};
    VkImageSubresourceRange all = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 1, 0, 1};
    vkCmdClearColorImage(cmd, dst, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, &black, 1, &all);
  }
  VkImageBlit blit = {.srcSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .srcOffsets = {{0, 0, 0}, {(int32_t)d->dw, (int32_t)d->dh, 1}},
                      .dstSubresource = {VK_IMAGE_ASPECT_COLOR_BIT, 0, 0, 1}, .dstOffsets = {{r[0], r[1], 0}, {r[2], r[3], 1}}};
  vkCmdBlitImage(cmd, d->image, VK_IMAGE_LAYOUT_TRANSFER_SRC_OPTIMAL, dst, VK_IMAGE_LAYOUT_TRANSFER_DST_OPTIMAL, 1, &blit, VK_FILTER_LINEAR);
}

// A key by its evdev code: 1 when the desk took it (a toggle, or a key while the desk shows).
static int vkdesk_key(int code, int pressed) {
  VkDesk *d = vkdesk_;
  if (!d || d->pid <= 0) return 0;
  int64_t to = verse_panel_key_to(d->mode, code);
  if (to == PANEL_TOGGLE) {
    if (pressed) { d->mode = verse_panel_next(d->mode, code); fprintf(stderr, "vk-realm: the desk is %s\n", d->mode == PANEL_CLOSED ? "closed" : d->mode == PANEL_FLAT ? "flat" : "docked"); }
    return 1;
  }
  if (to != PANEL_TO_DESK) return 0;
  char cmd[64];
  snprintf(cmd, sizeof cmd, "keydown %d %d\n", code, pressed ? 1 : 0);
  vkdesk_say(d, cmd);
  return 1;
}
// The pointer at screen point x, y (the swapchain's pixels): 1 when it is over the panel, and then
// the desk's (moved there; down/up 1/0 for the left button, -1 for a move).
static int vkdesk_pointer(float x, float y, int button) {
  VkDesk *d = vkdesk_;
  if (!d || d->pid <= 0 || d->mode == PANEL_CLOSED || !d->sw) return 0;
  int64_t px = verse_panel_point(d->mode, d->sw, d->sh, d->dw, d->dh, (int64_t)x, (int64_t)y, 0);
  int64_t py = verse_panel_point(d->mode, d->sw, d->sh, d->dw, d->dh, (int64_t)x, (int64_t)y, 1);
  if (px < 0 || py < 0) return d->mode == PANEL_FLAT;   // flat: the whole screen is the desk's
  char cmd[96];
  snprintf(cmd, sizeof cmd, "move %lld %lld\n", (long long)px, (long long)py);
  vkdesk_say(d, cmd);
  if (button >= 0) { snprintf(cmd, sizeof cmd, "button 272 %d\n", button); vkdesk_say(d, cmd); }
  return 1;
}

static void vkdesk_close(Vkc *v, VkDesk *d) {
  if (d->pid > 0) { vkdesk_say(d, "quit\n"); close(d->to); close(d->from); int st; for (int i = 0; i < 20 && waitpid(d->pid, &st, WNOHANG) == 0; i++) usleep(50000); kill(d->pid, SIGTERM); waitpid(d->pid, &st, 0); }
  if (d->image) vkDestroyImage(v->dev, d->image, NULL);
  if (d->mem) vkFreeMemory(v->dev, d->mem, NULL);
  if (d->staging.buf) vkc_free(v, &d->staging);
  vkdesk_ = NULL;
}
#endif
