#include "host.h"
#include "Home.h"
#include "Player.h"
#include "Grid.h"
#include "Cast.h"
#include "Wallet.h"
#include "Camera.h"
// The compiler's fixed-array typedefs are local to each emitted library.
// Give Haunt's identical 12-float packet alias a distinct C import identity.
#define coax_a12_f32 coax_haunt_a12_f32
#include "Haunt.h"
#undef coax_a12_f32
#include "HauntBuffer.h"
#include "Hud.h"
