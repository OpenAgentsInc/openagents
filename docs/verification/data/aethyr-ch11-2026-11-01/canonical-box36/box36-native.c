#include "Box36.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>

int main(int argc,char **argv){
  assert(argc==2);FILE *out=fopen(argv[1],"wb");assert(out);
  for(uint32_t model=0;model<3;model++)for(uint32_t id=0;id<36;id++){
    coax_t_DeadgearsS q=box36_sample(id,model);
    float packet[10]={q.pos.x,q.pos.y,q.pos.z,q.pos.w,q.world.x,q.world.y,q.world.z,q.normal.x,q.normal.y,q.normal.z};
    assert(fwrite(packet,sizeof(float),10,out)==10);
  }
  assert(!fclose(out));return 0;
}
