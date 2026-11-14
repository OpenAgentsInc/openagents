// The host core's face (core-face.coax) as C loads Coax libraries whose #export_c procedures
// other units of the same app export too: SHA-256's (native-audio, the OpenAgents chat) and
// coax/lib/crypto/bytes.coax's, through byte-storage.coax (the chat loads it through sha256.coax).
// No host calls these through the face, so this unit keeps its own names for them and the other
// unit owns the C names (#13588). Compile engine-core-face.c with -include this file. A library
// export added to the face later that another unit has too fails the app's link (coax/remote/check.sh
// desktop-units); list it here. The face's exports:
//   nm -g --defined-only engine-core-face.o | awk '{print $3}' | sed 's/^_//'
#define S_hash engine_core_S_hash
#define S_hex engine_core_S_hex
#define cb_ct_equal engine_core_cb_ct_equal
#define cb_ct_select32 engine_core_cb_ct_select32
#define cb_ct_select64 engine_core_cb_ct_select64
#define cb_load_be32 engine_core_cb_load_be32
#define cb_load_be64 engine_core_cb_load_be64
#define cb_load_le32 engine_core_cb_load_le32
#define cb_load_le64 engine_core_cb_load_le64
#define cb_mask engine_core_cb_mask
#define cb_span engine_core_cb_span
#define cb_store_be32 engine_core_cb_store_be32
#define cb_store_be64 engine_core_cb_store_be64
#define cb_store_le32 engine_core_cb_store_le32
#define cb_store_le64 engine_core_cb_store_le64
