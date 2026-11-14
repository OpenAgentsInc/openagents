// Firm reason: 6 (C ABI namespace wrapper for the generated native SDK unit).
// The world inventory reuses the existing package grammar. Its transitive
// package-codec exports are private to this unit; the public package codec
// remains NativeWire.c's. No second implementation or wire format is added.
#define cnu_unwrap nw_cnu_unwrap
#define cnu_path nw_cnu_path
#define cnu_number nw_cnu_number
#define cnu_text nw_cnu_text
#define uri_parse nw_uri_parse
#include "Generated/NativeWorld.c"
