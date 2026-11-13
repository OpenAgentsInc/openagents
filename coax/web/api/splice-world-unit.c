// metaverse/splice/world-api.coax's C (splice-world.generated.c: Splice's game, its edit layer, its
// walk replay and the world document's codec) as one unit of the API (#13412): the code the page
// at /worlds/splice runs, so the server checks an edit exactly as the page does.
// The game's code leaves parameters and values unused in places; those warnings stay quiet for
// this unit only (-Werror in test.sh).
#pragma GCC diagnostic ignored "-Wunused-parameter"
#pragma GCC diagnostic ignored "-Wunused-variable"
#pragma GCC diagnostic ignored "-Wunused-but-set-variable"
#pragma GCC diagnostic ignored "-Wunused-function"
// A library procedure another unit exports too keeps a name of its own here.
#define NC_count splice_world_NC_count
#include "splice-world.generated.c"
