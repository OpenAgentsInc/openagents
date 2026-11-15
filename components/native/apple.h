// Firm reason: 6 (opaque native SDK boundary for the shared component scene).
#ifndef COAX_COMPONENT_NATIVE_APPLE_H
#define COAX_COMPONENT_NATIVE_APPLE_H
#include <stdint.h>
void *cnv_open(const char *shader);
void *cnv_view(void *owner);
void cnv_begin(void *owner,int reduced);
int cnv_add(void *owner,int64_t key,int kind,int role,float x,float y,float w,float h,float cl,float ct,float cr,float cb,uint32_t bg,uint32_t line,uint32_t ink,float alpha,float value,int flags);
void cnv_frame(void *owner);
int cnv_count(void *owner);
int cnv_refused(void *owner);
void cnv_close(void *owner);
#endif
