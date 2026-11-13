// Test-only initialized storage. Two otherwise identical engines write distinct
// words into imported memory, making an obsolete instantiation observable.
#include <stdint.h>
#ifndef RELOAD_MARKER
#define RELOAD_MARKER 0x13579bdfu
#endif
static volatile uint32_t reload_marker = RELOAD_MARKER;
__attribute__((export_name("coaxial_reload_marker")))
uint32_t coaxial_reload_marker(void) { return reload_marker; }
