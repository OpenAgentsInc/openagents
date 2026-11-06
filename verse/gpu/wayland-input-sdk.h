// Firm reason: 6 (Wayland/XKB SDK: keymap, compose and physical listener effects; Coax normalizes the ABI).
#ifndef AETHYR_WAYLAND_INPUT_SDK_H
#define AETHYR_WAYLAND_INPUT_SDK_H
#include <xkbcommon/xkbcommon.h>
#include <xkbcommon/xkbcommon-compose.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <locale.h>
#include "wayland-input.h"
typedef struct {
 struct xkb_context *context;
 struct xkb_keymap *map;
 struct xkb_state *state;
 struct xkb_compose_table *table;
 struct xkb_compose_state *compose;
 int delivered[KEY_MAX+1];
 unsigned char text_down[KEY_MAX+1],roles[KEY_MAX+1];
} HwiSdk;
static void hwi_sdk_reset(HwiSdk *s){memset(s->roles,0,sizeof s->roles);memset(s->delivered,0,sizeof s->delivered);memset(s->text_down,0,sizeof s->text_down);if(s->compose)xkb_compose_state_reset(s->compose);if(s->state)xkb_state_update_mask(s->state,0,0,0,0,0,0);}
static void hwi_sdk_close(HwiSdk *s){if(s->compose)xkb_compose_state_unref(s->compose);if(s->table)xkb_compose_table_unref(s->table);if(s->state)xkb_state_unref(s->state);if(s->map)xkb_keymap_unref(s->map);if(s->context)xkb_context_unref(s->context);memset(s,0,sizeof *s);}
static int hwi_sdk_keymap(HwiSdk *s,uint32_t format,int fd,uint32_t bytes){
 struct stat st;if(format!=WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1||!hwi_keymap(bytes)||fstat(fd,&st)||st.st_size<bytes){close(fd);return 0;}
 const char *data=mmap(NULL,bytes,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(data==MAP_FAILED)return 0;
 HwiSdk fresh={0};fresh.context=xkb_context_new(XKB_CONTEXT_NO_FLAGS);
 if(fresh.context&&memchr(data,0,bytes)==data+bytes-1)fresh.map=xkb_keymap_new_from_string(fresh.context,data,XKB_KEYMAP_FORMAT_TEXT_V1,XKB_KEYMAP_COMPILE_NO_FLAGS);
 munmap((void*)data,bytes);if(fresh.map)fresh.state=xkb_state_new(fresh.map);
 if(!fresh.state){hwi_sdk_close(&fresh);return 0;}
 const char *locale=getenv("LC_ALL");if(!locale||!*locale)locale=getenv("LC_CTYPE");if(!locale||!*locale)locale=getenv("LANG");if(!locale||!*locale)locale="C.UTF-8";
 fresh.table=xkb_compose_table_new_from_locale(fresh.context,locale,XKB_COMPOSE_COMPILE_NO_FLAGS);if(fresh.table)fresh.compose=xkb_compose_state_new(fresh.table,XKB_COMPOSE_STATE_NO_FLAGS);
 hwi_sdk_close(s);*s=fresh;return 1;
}
static int hwi_sdk_mods(HwiSdk *s){
 if(!s->state)return 0;
 return (int)hwi_mods(xkb_state_mod_name_is_active(s->state,XKB_MOD_NAME_SHIFT,XKB_STATE_MODS_EFFECTIVE)>0,
 xkb_state_mod_name_is_active(s->state,XKB_MOD_NAME_CTRL,XKB_STATE_MODS_EFFECTIVE)>0,
 xkb_state_mod_name_is_active(s->state,XKB_MOD_NAME_ALT,XKB_STATE_MODS_EFFECTIVE)>0,
 xkb_state_mod_name_is_active(s->state,XKB_MOD_NAME_LOGO,XKB_STATE_MODS_EFFECTIVE)>0);
}
static void hwi_sdk_modifiers(HwiSdk *s,uint32_t depressed,uint32_t latched,uint32_t locked,uint32_t group){if(s->state)xkb_state_update_mask(s->state,depressed,latched,locked,0,0,group);}
// No SDK callback calls back into a running producer. These are Wayland
// dispatch effects, and the owning host drains after every handed-off input.
static void hwi_sdk_key(HwiSdk *s,void *user,uint32_t code,int down,int wants,
 uint32_t (*key)(void*,int,int,int),void (*text)(void*,const char*,size_t)){
 if(code>KEY_MAX||!s->state)return;
 xkb_keysym_t sym=xkb_state_key_get_one_sym(s->state,code+8);
 int mods=hwi_sdk_mods(s),role=down?(int)hwi_role(sym):s->roles[code];s->roles[code]=down?role:0;
 int other=0;for(int i=0;i<=KEY_MAX;i++)if(i!=(int)code&&s->roles[i]==role)other=1;
 const char *name=role==1?XKB_MOD_NAME_SHIFT:role==2?XKB_MOD_NAME_CTRL:role==4?XKB_MOD_NAME_ALT:XKB_MOD_NAME_LOGO;
 if(role&&xkb_state_mod_name_is_active(s->state,name,XKB_STATE_MODS_LATCHED|XKB_STATE_MODS_LOCKED)>0)other=1;
 mods=(int)hwi_mod_event(mods,role,down,other);
 if(!down){int value=s->delivered[code];s->delivered[code]=0;if(s->text_down[code]){s->text_down[code]=0;return;}if(value)key(user,value,mods,0);return;}
 int value=(int)hwi_key(sym,xkb_keysym_to_utf32(sym));
 if(wants&&text&&hwi_text(wants,mods,value)){
  char bytes[1025];int n=0;
  if(s->compose){xkb_compose_state_feed(s->compose,sym);enum xkb_compose_status status=xkb_compose_state_get_status(s->compose);
   if(status==XKB_COMPOSE_COMPOSING){s->text_down[code]=1;return;}
   if(status==XKB_COMPOSE_CANCELLED){xkb_compose_state_reset(s->compose);s->text_down[code]=1;return;}
   if(status==XKB_COMPOSE_COMPOSED){n=xkb_compose_state_get_utf8(s->compose,bytes,sizeof bytes);xkb_compose_state_reset(s->compose);}
  }
  if(!n)n=xkb_state_key_get_utf8(s->state,code+8,bytes,sizeof bytes);
  if(hwi_commit(n)){text(user,bytes,(size_t)n);s->text_down[code]=1;}return;
 }
 // Dead/compose symbols have no Unicode scalar and produce no intermediate key.
 if(wants&&s->compose&&sym!=XKB_KEY_NoSymbol&&value==0){xkb_compose_state_feed(s->compose,sym);if(xkb_compose_state_get_status(s->compose)==XKB_COMPOSE_COMPOSING){s->text_down[code]=1;return;}}
 if(value){s->delivered[code]=value;key(user,value,mods,1);}
}
#endif
