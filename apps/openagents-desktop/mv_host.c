// Firm reason: 6 (thin C wrapper mapping Swift-friendly pointers to Coax slice ABI in metaverse-viewer.o).
#include "mv_host.h"
#include "metaverse-viewer.h"
#include <stdlib.h>
#include <string.h>

static coax_v_t_MvHostState mv_view(void *h) {
  return (coax_v_t_MvHostState){(coax_t_MvHostState *)h, h ? 1 : 0};
}

static coax_string mv_str(const char *s) {
  if (!s) return (coax_string){NULL, 0};
  return (coax_string){(const uint8_t *)s, (int64_t)strlen(s)};
}

void *oa_mv_alloc(void) {
  int64_t sz = oa_mv_state_size();
  if (sz <= 0) return NULL;
  void *p = calloc(1, (size_t)sz);
  if (p) oa_mv_init(mv_view(p));
  return p;
}

void oa_mv_free(void *handle) {
  if (handle) free(handle);
}

int oa_mv_load_buf(void *handle, const char *path, const uint8_t *raw, int raw_len, const uint8_t *extra_bin, int extra_len) {
  if (!handle || !raw || raw_len <= 0) return 0;
  coax_v_u8 rv = {(uint8_t *)raw, (int64_t)raw_len};
  coax_v_u8 ev = {(uint8_t *)extra_bin, (extra_bin && extra_len > 0) ? (int64_t)extra_len : 0};
  return oa_mv_load(mv_view(handle), mv_str(path), rv, ev) ? 1 : 0;
}

int oa_mv_is_ok(void *handle) { return handle && oa_mv_ok(mv_view(handle)) ? 1 : 0; }
int oa_mv_get_format(void *handle) { return handle ? (int)oa_mv_format(mv_view(handle)) : 0; }
int oa_mv_is_z_up(void *handle) { return handle && oa_mv_z_up(mv_view(handle)) ? 1 : 0; }
int oa_mv_get_vert_count(void *handle) { return handle ? (int)oa_mv_vert_count(mv_view(handle)) : 0; }
int oa_mv_get_index_count(void *handle) { return handle ? (int)oa_mv_index_count(mv_view(handle)) : 0; }
int oa_mv_get_node_count(void *handle) { return handle ? (int)oa_mv_node_count(mv_view(handle)) : 0; }
int oa_mv_get_mat_count(void *handle) { return handle ? (int)oa_mv_mat_count(mv_view(handle)) : 0; }

int oa_mv_read_verts(void *handle, float *out, int float_cap) {
  if (!handle || !out || float_cap <= 0) return 0;
  return (int)oa_mv_copy_verts(mv_view(handle), (coax_v_f32){out, (int64_t)float_cap});
}

int oa_mv_read_indices(void *handle, uint32_t *out, int idx_cap) {
  if (!handle || !out || idx_cap <= 0) return 0;
  return (int)oa_mv_copy_indices(mv_view(handle), (coax_v_u32){out, (int64_t)idx_cap});
}

int oa_mv_get_bounds(void *handle, float out6[6]) {
  if (!handle || !out6) return 0;
  return oa_mv_bounds(mv_view(handle), (coax_v_f32){out6, 6}) ? 1 : 0;
}

int oa_mv_get_node_info(void *handle, int idx, int64_t out_meta5[5], char *out_label, int label_cap) {
  if (!handle || !out_meta5 || !out_label || label_cap < 2) return 0;
  int n = (int)oa_mv_node_info(mv_view(handle), (int64_t)idx, (coax_v_s64){out_meta5, 5}, (coax_v_u8){(uint8_t *)out_label, (int64_t)(label_cap - 1)});
  if (n < 0) n = 0;
  if (n >= label_cap) n = label_cap - 1;
  out_label[n] = '\0';
  return n;
}

void oa_mv_set_node_visible(void *handle, int idx, int visible) {
  if (handle) oa_mv_node_set_visible(mv_view(handle), (int64_t)idx, visible != 0);
}

void oa_mv_select_node(void *handle, int idx) {
  if (handle) oa_mv_node_select(mv_view(handle), (int64_t)idx);
}

int oa_mv_get_mat_info(void *handle, int idx, float out_rgba_mr6[6], int64_t out_tex2[2]) {
  if (!handle || !out_rgba_mr6 || !out_tex2) return 0;
  return oa_mv_mat_info(mv_view(handle), (int64_t)idx, (coax_v_f32){out_rgba_mr6, 6}, (coax_v_s64){out_tex2, 2}) ? 1 : 0;
}

int oa_mv_ray_pick(void *handle, float ox, float oy, float oz, float dx, float dy, float dz, float out_hit3[3]) {
  if (!handle || !out_hit3) return -1;
  return (int)oa_mv_pick(mv_view(handle), ox, oy, oz, dx, dy, dz, (coax_v_f32){out_hit3, 3});
}

int oa_mv_get_tree_tsv(void *handle, char *out, int cap) {
  if (!handle || !out || cap < 2) return 0;
  int n = (int)oa_mv_tree_tsv(mv_view(handle), (coax_v_u8){(uint8_t *)out, (int64_t)(cap - 1)});
  if (n < 0) n = 0;
  if (n >= cap) n = cap - 1;
  out[n] = '\0';
  return n;
}

int oa_mv_get_stats(void *handle, char *out, int cap) {
  if (!handle || !out || cap < 2) return 0;
  int n = (int)oa_mv_stats(mv_view(handle), (coax_v_u8){(uint8_t *)out, (int64_t)(cap - 1)});
  if (n < 0) n = 0;
  if (n >= cap) n = cap - 1;
  out[n] = '\0';
  return n;
}

int oa_mv_get_err(void *handle, char *out, int cap) {
  if (!handle || !out || cap < 2) return 0;
  int n = (int)oa_mv_err(mv_view(handle), (coax_v_u8){(uint8_t *)out, (int64_t)(cap - 1)});
  if (n < 0) n = 0;
  if (n >= cap) n = cap - 1;
  out[n] = '\0';
  return n;
}
