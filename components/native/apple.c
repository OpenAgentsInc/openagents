// Firm reason: 6 (Apple GPU view allocation/lifecycle; Coax owns all paint/motion).
#include "apple.h"
#include "NativePresentation.h"
#include "../../apps/shared/host/apple/ui-host-apple.h"
#include <stdlib.h>
typedef struct {UiHost *host;coax_t_ArcadeScene *scene;} Cnv;
static coax_v_t_ArcadeScene state(Cnv *v) {return (coax_v_t_ArcadeScene){v->scene,1};}
static int frame(void *user,float *vertices,float w,float h,float dpr,float dt) {
 Cnv *v=user;return (int)arcade_scene_frame(state(v),(coax_v_f32){vertices,arcade_scene_max_vertices()*28},w,h,dpr,dt);
}
void *cnv_open(const char *shader) {
 Cnv *v=calloc(1,sizeof *v);if (!v) return NULL;
 v->scene=calloc(1,sizeof *v->scene);if (!v->scene) {free(v);return NULL;}
 UiApp app={.title="Coax components",.width=402,.height=874,.max_vertices=(uint32_t)arcade_scene_max_vertices(),.shader=shader,.user=v,.frame=frame,.transparent=1};
 v->host=ui_open(&app,0);if (!v->host) {free(v->scene);free(v);return NULL;}
 uint8_t mono=255,color[4]={255,255,255,255};ui_atlas(v->host,&mono,1,1,color,1,1);return v;
}
void *cnv_view(void *owner) {return owner ? ui_view(((Cnv *)owner)->host):NULL;}
void cnv_begin(void *owner,int reduced) {if (owner) arcade_scene_begin(state(owner),reduced!=0);}
int cnv_add(void *owner,int64_t key,int kind,int role,float x,float y,float w,float h,float cl,float ct,float cr,float cb,uint32_t bg,uint32_t line,uint32_t ink,float alpha,float value,int flags) {
 return owner && arcade_scene_add(state(owner),key,kind,role,x,y,w,h,cl,ct,cr,cb,bg,line,ink,alpha,value,flags);
}
void cnv_frame(void *owner) {if (owner) ui_frame(((Cnv *)owner)->host);}
int cnv_count(void *owner) {return owner ? (int)((Cnv *)owner)->scene->painted:0;}
int cnv_refused(void *owner) {return owner ? (int)((Cnv *)owner)->scene->refused:0;}
void cnv_close(void *owner) {if (owner) {Cnv *v=owner;ui_close(v->host);free(v->scene);free(v);}}
