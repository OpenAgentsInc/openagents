/* Chapter 169's harness takes no caller spans that could overlap, so this
 * adapter observes nothing; it exists so the shared native build has one. */
#include "compiler-harness.h"
typedef int psionic_harness_adapter;
