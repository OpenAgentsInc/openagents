// script-store.coax's C (script-store.generated.c: the realm-script rules, the module check, the
// interpreter that loads a module, the manifest and the stored scripts' codec) as one unit of the API
// (#13467): the check a page and a room run, so the server stores only what they will load.
// The engine's code leaves parameters and values unused in places; those warnings stay quiet for this
// unit only (-Werror in test.sh).
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-function"
// A library procedure another unit exports too keeps a name of its own here: the SHA-256 kernel a
// module is named with, which nip98's unit exports too.
#define S_hash script_store_S_hash
#define S_hex script_store_S_hex
#include "script-store.generated.c"
