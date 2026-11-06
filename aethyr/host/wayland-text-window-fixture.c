// Firm reason: 6 (real XKB compose state and production Wayland window callbacks, without opening a display or GPU).
#define _GNU_SOURCE
#define AETHYR_WAYLAND_INPUT_V1
#define AETHYR_WAYLAND_TEXT_V1
#include <stdbool.h>
#include <stdint.h>
#include "vkw.h"
static int wanted=1;static uint64_t owner=1;static int rows;
static void row(int kind,int a,int b,int c){printf("%s[%d,%d,%d,%d]",rows++?",":"",kind,a,b,c);}
bool realm_key(void*s,int key){(void)s;(void)key;return false;}
bool realm_key_up(void*s,int key){(void)s;(void)key;return false;}
static uint32_t key_cb(void*s,int key,int mods,int down){(void)s;(void)key;(void)mods;(void)down;return 0;}
static int wants_cb(void*s){(void)s;return wanted;}
static uint64_t owner_cb(void*s){(void)s;return owner;}
static void feed(Vkw*w){xkb_compose_state_feed(w->input.compose,XKB_KEY_dead_acute);if(xkb_compose_state_get_status(w->input.compose)!=XKB_COMPOSE_COMPOSING)exit(3);}
int main(void){
 Vkw w={0};vkw_=&w;w.to.realm_key=key_cb;w.to.wants_text=wants_cb;w.to.text_owner=owner_cb;
 hwt_open_sdk(&w.text_sdk,(HwtTo){.wants=wants_cb,.owner=owner_cb,.cancel_compose=vkw_compose_cancel,.cancel_user=&w},NULL,NULL);hwt_focus(&w.text_sdk,1);
 w.input.context=xkb_context_new(XKB_CONTEXT_NO_FLAGS);w.input.table=xkb_compose_table_new_from_locale(w.input.context,"C.UTF-8",XKB_COMPOSE_COMPILE_NO_FLAGS);w.input.compose=xkb_compose_state_new(w.input.table,XKB_COMPOSE_STATE_NO_FLAGS);if(!w.input.compose)return 2;
 printf("{\"schema\":\"aethyr.host.wayland-text-window.v1\",\"rows\":[");
 feed(&w);vkw_pt_button(NULL,NULL,1,0,BTN_LEFT,WL_POINTER_BUTTON_STATE_PRESSED);row(1,xkb_compose_state_get_status(w.input.compose),w.text_sdk.focused,w.text_sdk.epoch==4);vkw_pt_button(NULL,NULL,1,0,BTN_LEFT,WL_POINTER_BUTTON_STATE_RELEASED);
 feed(&w);vkw_key(NULL,NULL,2,0,KEY_ENTER,WL_KEYBOARD_KEY_STATE_PRESSED);row(2,xkb_compose_state_get_status(w.input.compose),w.text_sdk.focused,w.text_sdk.epoch==5);
 feed(&w);owner++;vkw_text_sync(&w,0);row(3,xkb_compose_state_get_status(w.input.compose),w.text_sdk.focused,w.text_sdk.epoch==6);
 feed(&w);vkw_text_invalidate(&w);row(4,xkb_compose_state_get_status(w.input.compose),w.text_sdk.focused,w.text_sdk.epoch==7);
 feed(&w);vkw_kb_leave(NULL,NULL,3,NULL);row(5,xkb_compose_state_get_status(w.input.compose),w.text_sdk.focused,w.keyboard_focused);
 hwt_close_sdk(&w.text_sdk);hwi_sdk_close(&w.input);row(6,w.input.compose==NULL,w.text_sdk.paste==NULL,w.text_sdk.input==NULL);printf("]}\n");return 0;
}
