/* Firm reason: 2 (independent C11 ABI/header-composition smoke).
   Compile both include orders after merging the native-header emitter fix. */
#ifdef GUARD_REVERSE
#include "guard-guide-eval-wire.h"
#include "guard-guide.h"
#include "guard-guide-eval-wire.h"
#else
#include "guard-guide.h"
#include "guard-guide-eval-wire.h"
#include "guard-guide.h"
#endif
int main(void) { return 0; }
