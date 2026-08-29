#include <objc/message.h>
#include <stddef.h>

typedef struct {
    unsigned long width;
    unsigned long height;
    unsigned long depth;
} OAMtlSize;

void oa_metal_dispatch(void *encoder, const void *sel, unsigned long groups,
                       unsigned long threads) {
    OAMtlSize grid = {groups, 1, 1};
    OAMtlSize tpg = {threads, 1, 1};
    typedef void (*Fn)(void *, const void *, OAMtlSize, OAMtlSize);
    ((Fn)(void *)objc_msgSend)(encoder, sel, grid, tpg);
}
