// Firm reason: 6 (Wayland/XKB SDK fixture: call the actual production listeners with real SDK objects/keymap descriptors).
#define _GNU_SOURCE
#define ENGINE_WAYLAND_INPUT_V1
#include <stdbool.h>
#include <stdint.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <unistd.h>
#include "vkw.h"
static int wanted;
static int rows;
static void row(int kind,int a,int b,int c){printf("%s[%d,%d,%d,%d]",rows++?",":"",kind,a,b,c);}
bool realm_key(void *s,int key){(void)s;row(90,key,0,1);return true;}
bool realm_key_up(void *s,int key){(void)s;row(90,key,0,0);return true;}
static uint32_t key_cb(void *s,int key,int mods,int down){(void)s;row(1,key,mods,down);return 1;}
static void text_cb(void *s,const char *bytes,size_t n){(void)s;int hash=0;for(size_t i=0;i<n;i++)hash=hash*257+(unsigned char)bytes[i];row(2,(int)n,hash,0);}
static int wants_cb(void *s){(void)s;return wanted;}
static int focus_cb(void *s,int focus){(void)s;row(3,focus,0,0);return 1;}
static void pointer_cb(void *s,float x,float y,int phase){(void)s;row(4,(int)x,(int)y,phase);}
static void wheel_cb(void *s,float dy){(void)s;row(5,(int)(dy*1000),0,0);}
static void hit(int code){vkw_key(NULL,NULL,0,0,(uint32_t)code,WL_KEYBOARD_KEY_STATE_PRESSED);vkw_key(NULL,NULL,0,0,(uint32_t)code,WL_KEYBOARD_KEY_STATE_RELEASED);}
static uint32_t mod(const char *name){xkb_mod_index_t i=xkb_keymap_mod_get_index(vkw_->input.map,name);return i==XKB_MOD_INVALID?0:1u<<i;}
static int map_fd(struct xkb_keymap *map){char *text=xkb_keymap_get_as_string(map,XKB_KEYMAP_FORMAT_TEXT_V1);if(!text)return -1;char name[]="/tmp/aethyr-input-keymap-XXXXXX";int fd=mkstemp(name);unlink(name);size_t n=strlen(text)+1;if(fd<0||write(fd,text,n)!=(ssize_t)n){if(fd>=0)close(fd);free(text);return -1;}lseek(fd,0,SEEK_SET);free(text);return fd;}
int main(int argc,char **argv){
 if(argc!=2)return 2;
 Vkw window={0};vkw_=&window;window.to.realm_key=key_cb;window.to.text=text_cb;window.to.wants_text=wants_cb;window.to.focus=focus_cb;window.to.pointer=pointer_cb;window.to.wheel=wheel_cb;
 struct xkb_context *ctx=xkb_context_new(XKB_CONTEXT_NO_FLAGS);struct xkb_rule_names names={.layout="us,de",.variant="intl,"};struct xkb_keymap *map=ctx?xkb_keymap_new_from_names(ctx,&names,XKB_KEYMAP_COMPILE_NO_FLAGS):NULL;if(!map)return 2;
 char *source=xkb_keymap_get_as_string(map,XKB_KEYMAP_FORMAT_TEXT_V1);if(!source)return 2;size_t size=strlen(source)+1;FILE *record=fopen(argv[1],"wb");if(!record||fwrite(source,1,size,record)!=size||fclose(record))return 2;free(source);
 int fd=map_fd(map);if(fd<0)return 2;vkw_keymap(NULL,NULL,WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1,fd,(uint32_t)size);if(!window.input.state)return 2;
 printf("{\"schema\":\"aethyr.host.wayland-input.v1\",\"rows\":[");
 hit(KEY_A);vkw_mods(NULL,NULL,0,mod(XKB_MOD_NAME_SHIFT),0,0,0);hit(KEY_A);
 vkw_mods(NULL,NULL,0,mod(XKB_MOD_NAME_CTRL)|mod(XKB_MOD_NAME_ALT)|mod(XKB_MOD_NAME_LOGO),0,0,0);hit(KEY_A);
 vkw_mods(NULL,NULL,0,0,0,0,1);hit(KEY_Y);vkw_mods(NULL,NULL,0,0,0,0,0);
 for(int i=0;i<15;i++){const int codes[]={KEY_ESC,KEY_ENTER,KEY_BACKSPACE,KEY_LEFT,KEY_RIGHT,KEY_UP,KEY_DOWN,KEY_HOME,KEY_END,KEY_PAGEUP,KEY_PAGEDOWN,KEY_DELETE,KEY_LEFTSHIFT,KEY_LEFTCTRL,KEY_TAB};hit(codes[i]);}
 wanted=1;hit(KEY_A);hit(KEY_APOSTROPHE);hit(KEY_E);wanted=0;
 // A release keeps the delivered key even if the modifier snapshot changes.
 vkw_mods(NULL,NULL,0,mod(XKB_MOD_NAME_SHIFT),0,0,0);vkw_key(NULL,NULL,0,0,KEY_A,WL_KEYBOARD_KEY_STATE_PRESSED);vkw_mods(NULL,NULL,0,0,0,0,0);vkw_key(NULL,NULL,0,0,KEY_A,WL_KEYBOARD_KEY_STATE_RELEASED);
 vkw_key(NULL,NULL,0,0,KEY_W,WL_KEYBOARD_KEY_STATE_PRESSED);vkw_kb_leave(NULL,NULL,0,NULL);row(6,window.input.delivered[KEY_W],window.shift,0);
 int pair[2];if(socketpair(AF_UNIX,SOCK_STREAM,0,pair))return 2;struct wl_display *display=wl_display_connect_to_fd(pair[0]);if(!display)return 2;struct wl_registry *registry=wl_display_get_registry(display);struct wl_seat *seat=wl_registry_bind(registry,1,&wl_seat_interface,8);struct wl_pointer *pointer=wl_seat_get_pointer(seat);
 vkw_pt_source(NULL,pointer,WL_POINTER_AXIS_SOURCE_FINGER);vkw_pt_axis(NULL,pointer,0,WL_POINTER_AXIS_VERTICAL_SCROLL,wl_fixed_from_double(1.25));vkw_pt_frame(NULL,pointer);
 vkw_pt_axis(NULL,pointer,0,WL_POINTER_AXIS_VERTICAL_SCROLL,wl_fixed_from_double(10));vkw_pt_discrete(NULL,pointer,WL_POINTER_AXIS_VERTICAL_SCROLL,2);vkw_pt_source(NULL,pointer,WL_POINTER_AXIS_SOURCE_WHEEL);vkw_pt_frame(NULL,pointer);
#ifdef WL_POINTER_AXIS_VALUE120_SINCE_VERSION
 vkw_pt_value120(NULL,pointer,WL_POINTER_AXIS_VERTICAL_SCROLL,30);vkw_pt_discrete(NULL,pointer,WL_POINTER_AXIS_VERTICAL_SCROLL,1);vkw_pt_axis(NULL,pointer,0,WL_POINTER_AXIS_VERTICAL_SCROLL,wl_fixed_from_double(2));vkw_pt_source(NULL,pointer,WL_POINTER_AXIS_SOURCE_WHEEL);vkw_pt_frame(NULL,pointer);
#endif
 vkw_pt_axis(NULL,pointer,0,WL_POINTER_AXIS_HORIZONTAL_SCROLL,wl_fixed_from_double(3));vkw_pt_frame(NULL,pointer);
 window.x=20;window.y=30;vkw_pt_button(NULL,pointer,0,0,BTN_LEFT,WL_POINTER_BUTTON_STATE_PRESSED);vkw_pt_leave(NULL,pointer,0,NULL);row(7,window.down,0,0);vkw_pt_motion(NULL,pointer,0,wl_fixed_from_int(-20),wl_fixed_from_int(300));vkw_pt_button(NULL,pointer,0,0,BTN_LEFT,WL_POINTER_BUTTON_STATE_RELEASED);
 wl_pointer_destroy(pointer);wl_seat_destroy(seat);wl_registry_destroy(registry);wl_display_disconnect(display);close(pair[1]);hwi_sdk_close(&window.input);row(8,window.input.context==NULL,window.input.map==NULL,window.input.state==NULL);
 printf("]}\n");xkb_keymap_unref(map);xkb_context_unref(ctx);return 0;
}
