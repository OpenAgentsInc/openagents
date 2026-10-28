#include "host.h"
#include <math.h>
#include <stdlib.h>
#include <string.h>
#include <float.h>
#define SLOTS 16
#define PACKETS 8192
#define OUTPUT 512
typedef struct {
  double *cells; size_t cell_count;
  uint8_t *config; size_t config_size;
  MRDot dots[PACKETS]; size_t dots_count;
  MRLine lines[PACKETS]; size_t lines_count;
  MRBar bar; char label[33]; uint8_t output[OUTPUT]; size_t output_size;
  bool failed;
} Resident;
static Resident *slots[SLOTS];
static uint32_t generations[SLOTS];
static Resident *lookup(int64_t h) {
  if(h<=0) return NULL;
  uint64_t u=(uint64_t)h; unsigned i=(unsigned)(u&255);
  return i<SLOTS && (u>>8)==generations[i] ? slots[i] : NULL;
}
int64_t mr_create(const uint8_t *config,size_t size,size_t cells) {
  if(!config || size==0 || size>65536 || cells==0 || cells>1048576) return 0;
  for(unsigned i=0;i<SLOTS;i++) if(!slots[i]) {
    Resident *r=calloc(1,sizeof(*r)); if(!r) return 0;
    r->cells=calloc(cells,sizeof(double)); r->config=malloc(size);
    if(!r->cells||!r->config){ free(r->cells);free(r->config);free(r);return 0; }
    memcpy(r->config,config,size);r->config_size=size;r->cell_count=cells;
    if(++generations[i]==0) generations[i]=1;
    slots[i]=r;return ((int64_t)generations[i]<<8)|i;
  }
  return 0;
}
void mr_destroy(int64_t h){ Resident*r=lookup(h);if(!r)return;slots[h&255]=NULL;free(r->cells);free(r->config);free(r); }
void mr_clear_frame(int64_t h){Resident*r=lookup(h);if(r){r->dots_count=r->lines_count=0;memset(&r->bar,0,sizeof(r->bar));}}
bool mr_ok(int64_t h){Resident*r=lookup(h);return r&&!r->failed;}
const MRDot *mr_dots(int64_t h){Resident*r=lookup(h);return r?r->dots:NULL;}
const MRLine *mr_lines(int64_t h){Resident*r=lookup(h);return r?r->lines:NULL;}
size_t mr_dot_count(int64_t h){Resident*r=lookup(h);return r?r->dots_count:0;}
size_t mr_line_count(int64_t h){Resident*r=lookup(h);return r?r->lines_count:0;}
const MRBar *mr_bar(int64_t h){Resident*r=lookup(h);return r?&r->bar:NULL;}
const uint8_t *mr_output(int64_t h){Resident*r=lookup(h);return r?r->output:NULL;}
size_t mr_output_size(int64_t h){Resident*r=lookup(h);return r?r->output_size:0;}
double host_load(int64_t h,int64_t slot){Resident*r=lookup(h);if(!r)return 0;if(slot<0||(uint64_t)slot>=r->cell_count){r->failed=true;return 0;}return r->cells[slot];}
void host_store(int64_t h,int64_t slot,double value){Resident*r=lookup(h);if(!r)return;if(slot<0||(uint64_t)slot>=r->cell_count||!isfinite(value)){r->failed=true;return;}r->cells[slot]=value;}
int64_t host_size(int64_t h){Resident*r=lookup(h);return r?(int64_t)r->config_size:0;}
int64_t host_byte(int64_t h,int64_t at){Resident*r=lookup(h);if(!r)return -1;if(at<0||(uint64_t)at>=r->config_size){r->failed=true;return -1;}return r->config[at];}
static bool floats(Resident*r,const double *v,size_t n){for(size_t i=0;i<n;i++)if(!isfinite(v[i])||fabs(v[i])>FLT_MAX){r->failed=true;return false;}return true;}
void host_dot(int64_t h,double x,double y,double size,double light,double soft,double crisp){Resident*r=lookup(h);if(!r)return;double v[]={x,y,size,light,soft,crisp};if(!floats(r,v,6))return;if(r->dots_count==PACKETS){r->failed=true;return;}r->dots[r->dots_count++]=(MRDot){x,y,size,light,soft,crisp,0,0};}
void host_line(int64_t h,double ax,double ay,double bx,double by,double a,double b,double width){Resident*r=lookup(h);if(!r)return;double v[]={ax,ay,bx,by,a,b,width};if(!floats(r,v,7))return;if(r->lines_count==PACKETS){r->failed=true;return;}r->lines[r->lines_count++]=(MRLine){ax,ay,bx,by,a,b,width,0};}
void host_label_byte(int64_t h,int64_t index,int64_t value){
  Resident*r=lookup(h);if(!r)return;
  if(index<0||index>=33||value<0||value>255){r->failed=true;return;}
  r->label[index]=(char)(uint8_t)value;
}
void host_bar(int64_t h,double x,double y,double width,double height,double press,double flash){
  Resident*r=lookup(h);if(!r)return;
  double v[]={x,y,width,height,press,flash};if(!floats(r,v,6))return;
  r->bar=(MRBar){x,y,width,height,press,flash,{0}};
  memcpy(r->bar.label,r->label,sizeof(r->bar.label));r->bar.label[32]=0;
}
void host_output_reset(int64_t h){Resident*r=lookup(h);if(r)r->output_size=0;}
void host_output_byte(int64_t h,int64_t value){Resident*r=lookup(h);if(!r)return;if(value<0||value>255||r->output_size==OUTPUT){r->failed=true;return;}r->output[r->output_size++]=(uint8_t)value;}
