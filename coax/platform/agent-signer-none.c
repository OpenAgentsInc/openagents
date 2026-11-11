// Firm reason: 6 (weak C symbols: the linker picks a host's own definitions over these; Coax has no way to say that).
// The shared agent engines' host capabilities (coax/lib/agent/engine/): the Lab's signer for our
// inference route and the random bytes its signatures take (microcoder.coax), and the wall clock
// (codex.coax, microcoder.coax). A host that has them defines these names itself
// (apps/openagents-desktop/zeron-face.c, over lab-sign.coax) and the linker takes its definitions.
// A host that does not, a Coax tool such as Coder, links this file (coax/web/tool.sh does): every
// name is here as a weak symbol, so loading the engines' dispatcher
// (coax/lib/agent/engine/engine.coax) asks nothing more of it. There is no signer: the Lab's
// signed route reports "the signature failed" and its other routes go on.
#include <stdbool.h>
#include <stdint.h>
#include <time.h>

__attribute__((weak)) int64_t zr_unix_s(void) { return (int64_t)time(NULL); }
__attribute__((weak)) int64_t zr_lab_unix_s(void) { return (int64_t)time(NULL); }
__attribute__((weak)) int64_t zr_random_u64(void) { return 0; }
__attribute__((weak)) void zr_sign_reset(void) {}
__attribute__((weak)) void zr_sign_key(int64_t i, int64_t b) { (void)i; (void)b; }
__attribute__((weak)) bool zr_sign_tag(int64_t b) { (void)b; return true; }
// The signature's length: 0, there is none.
__attribute__((weak)) int64_t zr_sign_run(int64_t now_s, int64_t a0, int64_t a1, int64_t a2, int64_t a3) { (void)now_s; (void)a0; (void)a1; (void)a2; (void)a3; return 0; }
__attribute__((weak)) int64_t zr_sign_byte(int64_t i) { (void)i; return 0; }
