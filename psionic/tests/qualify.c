/* Chapter 17 gate host: runs one case; the Coax verdict is the exit status. */
#include "qualify-gate.h"
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    char *end = NULL;
    long k = strtol(argv[1], &end, 10);
    if (end == argv[1] || *end != 0) return 2;
    return (int)ye_gate(k);
}
