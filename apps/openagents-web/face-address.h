// Face addresses follow the actual host ABI. Counts and wire/GPU words stay u32.
// Browser and the existing low-address Linux host retain their published ABI.
#ifndef COAXIAL_FACE_ADDRESS_H
#define COAXIAL_FACE_ADDRESS_H
#include <stdint.h>
#if defined(COAXIAL_NATIVE_ADDRESS)
typedef uintptr_t cx_face_addr;
#else
typedef uint32_t cx_face_addr;
#endif
#endif
