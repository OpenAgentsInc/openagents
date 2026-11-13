// Native-width addresses; counts and all data words retain the browser ABI.
#ifndef EDITOR_METAL_PRODUCER_H
#define EDITOR_METAL_PRODUCER_H
#include <stdint.h>
#define MF_FACE(F) \
 F(uint32_t,host_contract_version,(void)) \
 F(uint32_t,state_size,(void)) F(uint32_t,max_vertices,(void)) F(uint32_t,vertex_floats,(void)) F(uint32_t,uniform_floats,(void)) \
 F(void,state_init,(uintptr_t)) F(void,golden,(uint32_t)) F(uint32_t,frame,(uintptr_t,uintptr_t,uintptr_t,float,float,float,float,uint32_t)) \
 F(uint32_t,key,(uintptr_t,int32_t,int32_t)) F(uint32_t,key_up,(uintptr_t,int32_t,int32_t)) \
 F(uint32_t,pointer,(uintptr_t,float,float,int32_t,int32_t,int32_t)) F(void,wheel,(uintptr_t,float)) F(void,focus,(uintptr_t)) F(void,blur,(uintptr_t)) \
 F(uintptr_t,inbox,(uintptr_t)) F(uint32_t,inbox_cap,(void)) F(void,message,(uintptr_t,int32_t,uint32_t)) \
 F(uintptr_t,asset,(uintptr_t,int32_t,uint32_t)) F(uint32_t,asset_capacity,(uintptr_t,int32_t)) F(void,asset_ready,(uintptr_t,int32_t,uint32_t)) \
 F(uintptr_t,draws,(uintptr_t)) F(uint32_t,draw_count,(uintptr_t)) F(uint32_t,water_draws,(uintptr_t)) F(uint32_t,stores,(void)) \
 F(uintptr_t,store,(uintptr_t,int32_t)) F(uint32_t,store_cap,(int32_t)) F(uint32_t,store_bytes,(uintptr_t,int32_t)) F(uint32_t,store_version,(uintptr_t,int32_t)) \
 F(uintptr_t,particles,(uintptr_t)) F(uint32_t,particle_cap,(void)) F(uint32_t,particle_bytes,(uintptr_t)) F(uint32_t,particle_instances,(uintptr_t)) F(uint32_t,grass_instances,(uintptr_t)) \
 F(int32_t,action,(uintptr_t)) F(uintptr_t,outbox,(uintptr_t)) F(uint32_t,outbox_len,(uintptr_t)) F(uint32_t,outbox_capacity,(uintptr_t)) F(void,action_done,(uintptr_t)) \
 F(uintptr_t,sounds,(uintptr_t)) F(uint32_t,sounds_len,(uintptr_t)) F(uint32_t,sounds_capacity,(uintptr_t)) F(uintptr_t,sound_bank,(uintptr_t)) F(uint32_t,sound_bank_len,(uintptr_t)) F(int32_t,sound_probe,(uintptr_t,int32_t,float,float,float)) \
 F(uint32_t,sound_bank_capacity,(uintptr_t)) F(uintptr_t,sound_bank_storage,(uintptr_t)) F(uint32_t,sound_bank_storage_bytes,(uintptr_t)) \
 F(uintptr_t,media,(uintptr_t)) F(uint32_t,media_len,(uintptr_t)) F(uint32_t,media_capacity,(uintptr_t)) F(int32_t,field_probe,(uintptr_t,int32_t)) F(int32_t,net_probe,(uintptr_t,int32_t)) \
 F(int32_t,wants_text,(uintptr_t)) F(int32_t,frame_admit,(int32_t)) F(uint32_t,pixel_budget,(void))
typedef struct {
 void *image;
#define MF_FN(result,name,args) result (*name) args;
 MF_FACE(MF_FN)
#undef MF_FN
} MFProducer;
#endif
