#include "precedence.h"
#include <stdint.h>

static uint8_t malformed[1] = {0};
static uint32_t run(uint8_t *data, uint32_t mode, uint32_t prior) {
    coax_t_ErrorInput input = {0};
    input.wire.data = data;
    input.wire.count = 1;
    input.prior = prior;
    input.mode = mode;
    return error_precedence_probe(input);
}

#ifdef __wasm__
uint32_t probe_bad(uint32_t mode, uint32_t prior) {
    return run((uint8_t *)(uintptr_t)0xfffffff0u, mode, prior);
}
uint32_t probe_wire(uint32_t mode) { return run(malformed, mode, 0); }
#else
#include <sys/mman.h>
#include <unistd.h>
int main(void) {
    long size = sysconf(_SC_PAGESIZE);
    if (size <= 0) return 1;
    void *guard = mmap(0, (size_t)size, PROT_NONE, MAP_PRIVATE | MAP_ANON, -1, 0);
    if (guard == MAP_FAILED) return 2;
    uint32_t codes[] = {1,2,3,4,5,6,7,0xdeadbeef};
    for (uint32_t mode=0; mode<8; mode++) {
        for (unsigned i=0; i<sizeof(codes)/sizeof(codes[0]); i++)
            if (run(guard, mode, codes[i]) != codes[i]) return 10+(int)mode;
        if (run(malformed, mode, 0) != 1) return 30+(int)mode;
    }
    if (munmap(guard, (size_t)size)) return 3;
    return 0;
}
#endif
