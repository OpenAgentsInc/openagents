/* Firm reason: 1 (scalar browser ABI for the character runtime; admission, playback and the block are Coax). */
#include "character-host.h"
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#define EXPORT(name) __attribute__((export_name(name)))
static coax_t_CharacterHost *state(uint32_t p){return (coax_t_CharacterHost *)(uintptr_t)p;}
static coax_v_t_CharacterHost view(uint32_t p){return (coax_v_t_CharacterHost){state(p),1};}
static coax_t_EchoCatalogScratch catalogScratch;
EXPORT("character_state_size") uint32_t size(void){return sizeof(coax_t_CharacterHost);}
EXPORT("character_state_init") void init(uint32_t p){coax_default_t_CharacterHost(state(p));}
EXPORT("character_digest") uint32_t digest(uint32_t p){return (uint32_t)(uintptr_t)state(p)->digest.e;}
EXPORT("character_pack_admit") uint32_t pack_admit(uint32_t p,uint32_t bytes){return character_pack_gate(view(p),bytes);}
EXPORT("character_catalog_at") uint32_t catalog_at(uint32_t n){return n<=sizeof(catalogScratch.bytes.e)?(uint32_t)(uintptr_t)catalogScratch.bytes.e:0;}
EXPORT("character_catalog_admit") uint32_t catalog_admit(uint32_t p,uint32_t n){return n<=sizeof(catalogScratch.bytes.e)&&character_catalog(view(p),(coax_v_u8){catalogScratch.bytes.e,n});}
EXPORT("character_asset_size") void asset(uint32_t p,int32_t i,uint32_t n){character_asset(view(p),i,n);}
EXPORT("character_block") uint32_t block(uint32_t p){return (uint32_t)(uintptr_t)state(p)->block.e;}
EXPORT("character_block_floats") uint32_t block_floats(void){return sizeof(state(0)->block.e)/sizeof(float);}
EXPORT("character_step") uint32_t step(uint32_t p,float dt){return (uint32_t)character_frame(view(p),dt);}
EXPORT("character_draws") uint32_t draws(uint32_t p){return (uint32_t)(uintptr_t)state(p)->draws.e;}
EXPORT("character_blend") uint32_t blend(int32_t i){return (uint32_t)character_draw_blend(i);}
EXPORT("character_palette") uint32_t palette(uint32_t p){return (uint32_t)(uintptr_t)state(p)->echo.e[0].palette.e;}
EXPORT("character_palette_bytes") uint32_t palette_bytes(void){return sizeof(state(0)->echo.e[0].palette.e);}
EXPORT("character_palette_version") uint32_t palette_version(uint32_t p){return (uint32_t)state(p)->echo.e[0].revision;}
EXPORT("character_probe") int32_t probe(uint32_t p,int32_t f){return (int32_t)character_probe(view(p),f);}
