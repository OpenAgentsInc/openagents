/* Existing native host effects and byte-preserving Grid-face observation.
   Schedule, acceptance, replay and output decisions live in Coax. */
#include "realm-host.h"
#include "face.h"
#include <stdint.h>
#include <string.h>

extern void page(uint32_t,uint32_t);
extern int64_t grid_probe(int64_t),grid_mark(int64_t);
extern float grid_pose(int64_t);
extern uint32_t grid_replay(void);
static RealmHost journey_host;
static float journey_width=960,journey_height=540;
static uint32_t journey_vertices;

int64_t gj_native_open(void) {
  rh_open(&journey_host,0);
  page(journey_host.state,1);
  const char facts[]="url=http://127.0.0.1:8817/realms/the-grid?from=home\nnow=1793577600000\nclip=1\nrealm=the-grid\nslot=the-grid\n";
  rh_send(&journey_host,RH_PAGE_KIND,facts,sizeof facts-1);
  return 0;
}
int64_t gj_native_frame(float dt) {
  /* Call the raw production export: never silently cap a count in rh_frame. */
  uint32_t vertices=coaxial_frame(journey_host.state,journey_host.vertices,journey_host.uniforms,journey_width,journey_height,1,dt,0);
  journey_vertices=vertices;
  rh_after(&journey_host,0);
  return vertices;
}
void gj_native_key(int64_t key,int64_t down) {
  if(down)coaxial_key(journey_host.state,(int32_t)key,0);
  else coaxial_key_up(journey_host.state,(int32_t)key,0);
  rh_after(&journey_host,0);
}
int64_t gj_native_probe(int64_t field) {return grid_probe(field);}
float gj_native_pose(int64_t field) {return grid_pose(field);}
int64_t gj_native_mark(int64_t step) {return grid_mark(step);}
int64_t gj_native_replay(void) {return grid_replay();}
void gj_native_viewport(float width,float height) {journey_width=width;journey_height=height;}
int64_t gj_native_info(int64_t field) {
  switch(field) {
    case 0:return journey_host.max_vertices;case 1:return journey_host.floats;
    case 2:return journey_host.uniform_floats;case 3:return journey_vertices;
    case 4:return coaxial_draw_count(journey_host.state);
    case 5:return 0; /* The Grid has no index asset or index-array export. */
    default:return -1;
  }
}
int64_t gj_native_bytes(int64_t kind) {
  switch(kind) {
    case 0:return (int64_t)journey_host.uniform_floats*4;
    case 1:return (int64_t)journey_vertices*journey_host.floats*4;
    case 2:return (int64_t)coaxial_draw_count(journey_host.state)*16;
    case 3:return 0;
    default:return -1;
  }
}
int64_t gj_native_byte(int64_t kind,int64_t offset) {
  int64_t bytes=gj_native_bytes(kind);
  if(offset<0||offset>=bytes)return -1;
  uint32_t address=0;uint64_t capacity=0;
  if(kind==0){address=journey_host.uniforms;capacity=(uint64_t)journey_host.uniform_floats*4;}
  if(kind==1){address=journey_host.vertices;capacity=(uint64_t)journey_host.max_vertices*journey_host.floats*4;}
  if(kind==2){address=coaxial_draws(journey_host.state);capacity=coaxial_draw_capacity(journey_host.state);}
  if(!address||(uint64_t)bytes>capacity)return -1;
  return ((const uint8_t *)rh_ptr(address))[offset];
}
void gj_native_close(void) {rh_close(&journey_host);}
