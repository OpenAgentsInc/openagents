// Firm reason: 6 (opaque SDK lifetime boundary; no generated types cross Swift).
#ifndef COMPONENTS_GALLERY_EMBEDDED_H
#define COMPONENTS_GALLERY_EMBEDDED_H
void *cg_open(const char *resources);
void *cg_view(void *owner);
void cg_frame(void *owner);
void cg_close(void *owner);
void cg_section(void *owner,int section);
void cg_motion(void *owner,int action);
void cg_scroll(void *owner,float dy);
#endif
