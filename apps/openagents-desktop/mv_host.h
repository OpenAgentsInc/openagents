// Firm reason: 6 (thin C wrapper over metaverse-viewer.o exports for Swift Bridging Header).
// C-ABI bridge to metaverse-engine/content/desktop-viewer-bridge.coax (GitHub #14041).
#ifndef OA_MV_HOST_H
#define OA_MV_HOST_H

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void *oa_mv_alloc(void);
void oa_mv_free(void *handle);
int oa_mv_load_buf(void *handle, const char *path, const uint8_t *raw, int raw_len, const uint8_t *extra_bin, int extra_len);
int oa_mv_is_ok(void *handle);
int oa_mv_get_format(void *handle);
int oa_mv_is_z_up(void *handle);
int oa_mv_get_vert_count(void *handle);
int oa_mv_get_index_count(void *handle);
int oa_mv_get_node_count(void *handle);
int oa_mv_get_mat_count(void *handle);
int oa_mv_read_verts(void *handle, float *out, int float_cap);
int oa_mv_read_indices(void *handle, uint32_t *out, int idx_cap);
int oa_mv_get_bounds(void *handle, float out6[6]);
int oa_mv_get_node_info(void *handle, int idx, int64_t out_meta5[5], char *out_label, int label_cap);
void oa_mv_set_node_visible(void *handle, int idx, int visible);
void oa_mv_select_node(void *handle, int idx);
int oa_mv_get_mat_info(void *handle, int idx, float out_rgba_mr6[6], int64_t out_tex2[2]);
int oa_mv_ray_pick(void *handle, float ox, float oy, float oz, float dx, float dy, float dz, float out_hit3[3]);
int oa_mv_get_tree_tsv(void *handle, char *out, int cap);
int oa_mv_get_stats(void *handle, char *out, int cap);
int oa_mv_get_err(void *handle, char *out, int cap);

#ifdef __cplusplus
}
#endif

#endif
