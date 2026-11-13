// Vulkan effects and bounded observations. Coax owns specs, planning, byte
// admission, identity checks and lease lifetimes. Included by the native face.
typedef struct {int index,part,planning;int64_t generation,epoch;coax_t_AssetId source;coax_t_BackingGrant grant;} SwBackingCall;
typedef struct {VkDeviceMemory memory;coax_t_HostInboxLease lease;} SwBackingMemory;
typedef struct {coax_t_NativeBackingSpec spec;coax_t_BackingNeed need;} SwBackingPlan;
static coax_t_NativeBacking vk_backing;
static SwBackingMemory vk_backing_memories[64];
static SwBackingCall *vk_backing_call;
static struct {
  int64_t events[4096][20],event_n,violations,last_status;
  int64_t devices[8][18],device_n;
  SwBackingPlan plans[8][135];
  bool pressure;
} vk_backing_observer;
static coax_v_t_NativeBacking sw_bv(void){return(coax_v_t_NativeBacking){&vk_backing,1};}
static void sw_b_event(int tag,int index,int64_t generation,int64_t epoch,int part,coax_t_HostInboxLease lease,int status){
  if(vk_backing_observer.event_n>=4096){vk_backing_observer.violations++;return;}
  int64_t *e=vk_backing_observer.events[vk_backing_observer.event_n++];coax_t_NativeBackingSpec spec={0};coax_t_BackingNeed need={0};
  if(index>=0&&index<135){spec=vk_backing.specs.e[index];need=vk_backing.needs.e[index];}
  const int64_t words[20]={tag,state.tick,index,index>=0?spec.row:-1,generation,epoch,part,index>=0?spec.category:-1,spec.logical,need.bytes,need.alignment,need.types,need.memory_type,lease.slot,lease.serial,lease.epoch,status,vk_backing.budget.e[0].queue.e[0].bytes,vk_backing.actual,vk_backing.budget.e[0].queue.e[0].count};
  memcpy(e,words,sizeof words);
}
static bool sw_b_release(coax_t_HostInboxLease lease){
  if(!nb_live(sw_bv(),lease))return false;
  coax_t_NativeBackingOwner owner=vk_backing.owners.e[lease.slot];
  if(!nb_release(sw_bv(),lease)){vk_backing_observer.violations++;return false;}
  sw_b_event(3,(int)owner.index,owner.generation,owner.epoch,(int)owner.part,lease,0);return true;
}
int sw_vk_backing_memory_allowed(uint64_t bytes,uint32_t type){
  if(!vk_backing_call||bytes>INT64_MAX||!nb_can_allocate(sw_bv(),vk_backing_call->grant.lease,(int64_t)bytes,type)){vk_backing_observer.violations++;return 0;}return 1;
}
void sw_vk_backing_memory_created(uintptr_t memory,uint64_t bytes,uint32_t type){
  SwBackingCall *c=vk_backing_call;
  if(!c||c->grant.lease.slot<0||c->grant.lease.slot>=64||vk_backing_memories[c->grant.lease.slot].memory||!nb_allocated(sw_bv(),c->grant.lease,(int64_t)bytes,type)){vk_backing_observer.violations++;return;}
  vk_backing_memories[c->grant.lease.slot]=(SwBackingMemory){(VkDeviceMemory)memory,c->grant.lease};
  if(vk_backing.actual>vk_memory_peak)vk_memory_peak=vk_backing.actual;
  sw_b_event(1,c->index,c->generation,c->epoch,c->part,c->grant.lease,0);
}
static coax_t_HostInboxLease sw_b_memory_lease(VkDeviceMemory memory){
  for(int i=0;i<64;i++)if(vk_backing_memories[i].memory==memory&&memory)return vk_backing_memories[i].lease;
  coax_t_HostInboxLease none={.slot=-1};return none;
}
void sw_vk_backing_memory_freed(uintptr_t memory){
  coax_t_HostInboxLease lease=sw_b_memory_lease((VkDeviceMemory)memory);
  if(lease.slot<0||!nb_freed(sw_bv(),lease)){vk_backing_observer.violations++;return;}
  coax_t_NativeBackingOwner o=vk_backing.owners.e[lease.slot];
  memset(&vk_backing_memories[lease.slot],0,sizeof vk_backing_memories[lease.slot]);
  sw_b_event(2,(int)o.index,o.generation,o.epoch,(int)o.part,lease,0);
}
static VkResult sw_b_gate(void *owner,const VksMemoryInfo *info){
  SwBackingCall *c=owner;
  coax_t_BackingNeed need={.bytes=info->requirements.size>INT64_MAX?-1:(int64_t)info->requirements.size,.alignment=info->requirements.alignment>INT64_MAX?-1:(int64_t)info->requirements.alignment,.types=info->requirements.memoryTypeBits,.memory_type=info->memory_type};
  if(c->planning)return nb_plan(sw_bv(),c->index,need)?VK_NOT_READY:VK_ERROR_INITIALIZATION_FAILED;
  c->grant=nb_hold(sw_bv(),c->index,c->generation,c->epoch,c->part,c->source,need);vk_backing_observer.last_status=c->grant.status;
  sw_b_event(c->grant.status==COAX_BackingStatus_ok?0:4,c->index,c->generation,c->epoch,c->part,c->grant.lease,c->grant.status);
  return c->grant.status==COAX_BackingStatus_ok?VK_SUCCESS:VK_ERROR_OUT_OF_POOL_MEMORY;
}
static VkResult sw_b_create(int index,int64_t generation,int64_t epoch,int part,coax_t_AssetId source,int planning,VkcBuffer *buffer,VksImage *image){
  if(vk_backing_call||index<0||index>=135||!vk_backing.specs.e[index].valid)return VK_ERROR_INITIALIZATION_FAILED;
  coax_t_NativeBackingSpec spec=vk_backing.specs.e[index];SwBackingCall call={.index=index,.generation=generation,.epoch=epoch,.part=part,.source=source,.planning=planning};
  call.grant.status=COAX_BackingStatus_invalid;call.grant.lease.slot=-1;vk_backing_call=&call;
  VkResult result;
  if(spec.kind==0)result=vks_buffer_try_admitted(&vk,buffer,(VkDeviceSize)spec.logical,sw_b_gate,&call,NULL);
  else{VkFormat format=spec.kind==1?VK_FORMAT_R8G8B8A8_UNORM:spec.kind==2?VK_FORMAT_R16G16B16A16_SFLOAT:VK_FORMAT_D32_SFLOAT;
    result=vks_image_try_admitted(&vk,image,(uint32_t)spec.width,(uint32_t)spec.height,1,0,format,spec.part==4?VKS_TARGET:VKS_TEXTURE,(uint32_t)spec.levels,sw_b_gate,&call,NULL);}
  vk_backing_call=NULL;
  if(result!=VK_SUCCESS&&call.grant.status==COAX_BackingStatus_ok&&!sw_b_release(call.grant.lease))return VK_ERROR_INITIALIZATION_FAILED;
  return result;
}
static void sw_b_buffer_free(VkcBuffer *buffer){
  if(!buffer->mem)return;coax_t_HostInboxLease lease=sw_b_memory_lease(buffer->mem);vkc_free(&vk,buffer);memset(buffer,0,sizeof *buffer);
  if(!sw_b_release(lease))vk_backing_observer.violations++;
}
static void sw_b_image_free(VksImage *image){
  if(!image->mem)return;coax_t_HostInboxLease lease=sw_b_memory_lease(image->mem);vks_image_free(&vk,image);memset(image,0,sizeof *image);
  if(!sw_b_release(lease))vk_backing_observer.violations++;
}
int32_t sw_vk_backing_pressure(void){if(vk_opened||vk_backing_observer.device_n)return 0;vk_backing_observer.pressure=true;return 1;}
int32_t sw_vk_backing_clone(void){
  if(!vk_opened||!vk_objects[0].live||!vk_objects[0].texture)return VK_ERROR_INITIALIZATION_FAILED;
  VksImage image={0};SwPhysical *p=&objects[0];VkResult result=sw_b_create(1,p->key.generation,p->key.epoch,3,p->key.content,0,NULL,&image);
  if(result==VK_SUCCESS)sw_b_image_free(&image);return result;
}
static bool sw_b_open(void){
  if(vk_backing_observer.device_n>=8)return false;
  coax_a5_s64 words={.e={vk.props.vendorID,vk.props.deviceID,vk.props.apiVersion,vk.props.driverVersion,vk.props.limits.maxMemoryAllocationCount}};
  if(!nb_begin(sw_bv(),manifest,words,vk_backing_observer.pressure))return false;
  for(int i=0;i<135;i++)if(vk_backing.specs.e[i].valid){VkcBuffer buffer={0};VksImage image={0};coax_t_NativeBackingSpec spec=vk_backing.specs.e[i];
    if(sw_b_create(i,0,0,(int)spec.part,spec.source,1,&buffer,&image)!=VK_NOT_READY||buffer.mem||image.mem)return false;}
  if(!nb_ready(sw_bv()))return false;
  int device=(int)vk_backing_observer.device_n++;int64_t *d=vk_backing_observer.devices[device];d[0]=vk_backing.budget.e[0].queue.e[0].epoch;
  for(int i=0;i<5;i++)d[1+i]=words.e[i];d[6]=vk_backing.pressure;d[7]=vk_backing.budget.e[0].total_cap;d[8]=vk_backing.budget.e[0].queue.e[0].capacity;
  for(int i=0;i<8;i++)d[9+i]=vk_backing.budget.e[0].caps.e[i];
  for(int i=0;i<135;i++)if(vk_backing.specs.e[i].valid){vk_backing_observer.plans[device][i]=(SwBackingPlan){vk_backing.specs.e[i],vk_backing.needs.e[i]};d[17]++;}
  sw_b_event(5,-1,0,0,-1,(coax_t_HostInboxLease){.slot=-1,.epoch=d[0]},0);return true;
}
static bool sw_b_close(void){
  int64_t epoch=vk_backing.budget.e[0].queue.e[0].epoch;if(!nb_close(sw_bv()))return false;
  sw_b_event(6,-1,0,0,-1,(coax_t_HostInboxLease){.slot=-1,.epoch=epoch},0);return true;
}
int64_t sw_vk_backing_info(int32_t field){switch(field){case 0:return vk_backing.phase;case 1:return vk_backing.budget.e[0].queue.e[0].bytes;case 2:return vk_backing.actual;case 3:return vk_backing.budget.e[0].queue.e[0].count;case 4:return vk_backing.budget.e[0].peak;case 5:return vk_backing.actual_peak;case 6:return vk_backing.budget.e[0].refused;case 7:return vk_backing_observer.violations;case 8:return vk_backing.budget.e[0].queue.e[0].epoch;case 9:return sizeof vk_backing;case 10:return sizeof vk_backing_observer;case 11:return vk_backing_observer.event_n;case 12:return vk_backing_observer.device_n;default:return -1;}}
int64_t sw_vk_backing_event(int32_t index,int32_t field){return index>=0&&index<vk_backing_observer.event_n&&field>=0&&field<20?vk_backing_observer.events[index][field]:-1;}
int64_t sw_vk_backing_device(int32_t index,int32_t field){return index>=0&&index<vk_backing_observer.device_n&&field>=0&&field<18?vk_backing_observer.devices[index][field]:-1;}
int64_t sw_vk_backing_plan(int32_t device,int32_t index,int32_t field){
  if(device<0||device>=vk_backing_observer.device_n||index<0||index>=135)return -1;SwBackingPlan *p=&vk_backing_observer.plans[device][index];if(!p->spec.valid)return -1;
  const int64_t words[13]={index,p->spec.row,p->spec.part,p->spec.category,p->spec.kind,p->spec.logical,p->spec.width,p->spec.height,p->spec.levels,p->need.bytes,p->need.alignment,p->need.types,p->need.memory_type};return field>=0&&field<13?words[field]:-1;
}
static size_t sw_b_statics(void){return sizeof vk_backing+sizeof vk_backing_memories+sizeof vk_backing_call+sizeof vk_backing_observer;}
