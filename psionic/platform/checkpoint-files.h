#pragma once
#include <stdint.h>
/* File effects only. Coax decides every byte, name and admission. */
int64_t psionic_ck_publish(const char *dir, const char *name, const uint8_t *bytes, int64_t n);
int64_t psionic_ck_load(const char *dir, const char *name, uint8_t *buf, int64_t cap);
