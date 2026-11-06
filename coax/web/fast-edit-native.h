// Development host SDK boundary. Call poll only between complete room ticks.
#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
bool coax_fast_start(const char *directory);
void coax_fast_poll(void);
void coax_fast_close(void);
int coax_fast_status(char *out, size_t capacity);
float coax_dev_dispatch(float value);
