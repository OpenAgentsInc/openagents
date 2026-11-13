// Firm reason: 2 (independent oracle for Wayland callbacks and SDK socket/pipe effects).
// Build against the actual host headers and generated Coax panel/hands adapters.
#include <assert.h>
#include <stdbool.h>
#include "../vkw.h"
#include "../vkdesk.h"
#include "../vkhands.h"

static int downs[KEY_MAX+1], ups[KEY_MAX+1], snapshots[KEY_MAX+1], realm_ups, pointer_ups;
static int key_sink(int code,int pressed) {
  if (pressed==2) snapshots[code]++;
  else if (pressed) downs[code]++;
  else ups[code]++;
  return 1;
}
bool realm_key(void *state,int key) { (void)state; (void)key; return true; }
bool realm_key_up(void *state,int key) { (void)state; (void)key; realm_ups++; return true; }
static void hand_sink(float x,float y,int phase) { assert(x==320 && y==180); if (phase==2) pointer_ups++; }
static int extra_button_sink(void *user,float x,float y,uint32_t code,int pressed) { (void)user;return vkdesk_button(x,y,code,pressed); }
static void read_pipe(int fd,char *out,size_t size) { ssize_t n=read(fd,out,size-1); assert(n>=0); out[n]=0; }

int main(void) {
  Vkw window={0}; vkw_=&window; window.to.key=key_sink;
  window.forwarded_down[KEY_A]=1; window.keys_down[KEY_W]=17;
  uint32_t held[]={KEY_LEFTSHIFT,KEY_S,KEY_F10,KEY_MAX+1,KEY_S};
  struct wl_array keys={.size=sizeof held,.alloc=sizeof held,.data=held};
  vkw_kb_enter(NULL,NULL,0,NULL,&keys);
  assert(ups[KEY_A]==1 && realm_ups==1 && !window.forwarded_down[KEY_A] && !window.keys_down[KEY_W]);
  assert(snapshots[KEY_S]==1 && snapshots[KEY_F10]==1 && window.forwarded_down[KEY_S] && window.shift==1);
  // A repeated snapshot neither retriggers nor releases a genuinely held key.
  vkw_kb_enter(NULL,NULL,0,NULL,&keys); assert(snapshots[KEY_S]==1 && ups[KEY_S]==0);
  vkw_key(NULL,NULL,0,0,KEY_S,WL_KEYBOARD_KEY_STATE_RELEASED); assert(ups[KEY_S]==1 && !window.forwarded_down[KEY_S]);
  vkw_kb_leave(NULL,NULL,0,NULL); assert(ups[KEY_F10]==1 && ups[KEY_LEFTSHIFT]==1 && window.shift==0);
  vkw_kb_leave(NULL,NULL,0,NULL); assert(ups[KEY_F10]==1);

  int pipefd[2]; assert(pipe(pipefd)==0); assert(fcntl(pipefd[0],F_SETFL,O_NONBLOCK)==0);
  VkDesk desk={.pid=1,.to=pipefd[1],.dw=640,.dh=360,.sw=1280,.sh=720}; vkdesk_=&desk; desk.mode=PANEL_FLAT;
  // The actual Wayland close callback asks the nested desktop to close its focused client.
  vkw_top_close(NULL,NULL); assert(window.closed);
  assert(vkdesk_window_close(&desk)==1); window.closed=0;
  char close_output[256]; read_pipe(pipefd[0],close_output,sizeof close_output); assert(strcmp(close_output,"close\n")==0 && !window.closed);
  desk.mode=verse_panel_next(PANEL_CLOSED,KEY_F9); assert(vkdesk_window_close(&desk)==1); read_pipe(pipefd[0],close_output,sizeof close_output); assert(strcmp(close_output,"close\n")==0);
  desk.mode=PANEL_WORLD; assert(vkdesk_window_close(&desk)==1); read_pipe(pipefd[0],close_output,sizeof close_output); assert(strcmp(close_output,"close\n")==0);
  desk.mode=PANEL_CLOSED; assert(vkdesk_window_close(&desk)==0);
  desk.mode=PANEL_FLAT; desk.pid=-1; assert(vkdesk_window_close(&desk)==0); desk.pid=1;
  assert(read(pipefd[0],close_output,sizeof close_output)<0 && errno==EAGAIN);
  // A held toggle in the focus snapshot must not change the panel mode.
  assert(vkdesk_key(KEY_F10,2)==1 && desk.mode==PANEL_FLAT);
  assert(vkdesk_key(KEY_W,2)==1); char output[256]; read_pipe(pipefd[0],output,sizeof output); assert(strcmp(output,"keydown 17 1\n")==0);
  desk.mode=PANEL_CLOSED; assert(vkdesk_key(KEY_W,0)==1); read_pipe(pipefd[0],output,sizeof output); assert(strcmp(output,"keydown 17 0\n")==0); desk.mode=PANEL_FLAT;
  assert(vkdesk_pointer(640,360,1)==1); read_pipe(pipefd[0],output,sizeof output); assert(strstr(output,"button 272 1\n"));
  // Pointer up belongs to the original recipient, including outside a panel or after closing it.
  assert(vkdesk_pointer(-10,-10,0)==1); read_pipe(pipefd[0],output,sizeof output); assert(strcmp(output,"button 272 0\n")==0);
  assert(vkdesk_pointer(640,360,1)==1); read_pipe(pipefd[0],output,sizeof output);
  desk.mode=PANEL_CLOSED; assert(vkdesk_pointer(640,360,0)==1); read_pipe(pipefd[0],output,sizeof output); assert(strcmp(output,"button 272 0\n")==0);
  assert(vkdesk_pointer(640,360,0)==0); assert(read(pipefd[0],output,sizeof output)<0 && errno==EAGAIN);
  // Real Wayland right/middle callbacks retain their own desktop ownership.
  desk.mode=PANEL_FLAT;window.to.button=extra_button_sink;window.x=640;window.y=360;
  vkw_pt_button(NULL,NULL,0,0,BTN_RIGHT,WL_POINTER_BUTTON_STATE_PRESSED);
  read_pipe(pipefd[0],output,sizeof output);assert(strcmp(output,"move 320 180\nbutton 273 1\n")==0 && desk.extra_down[0] && window.extra_down[0]);
  desk.mode=PANEL_CLOSED;window.x=-20;window.y=-20;
  vkw_pt_button(NULL,NULL,0,0,BTN_RIGHT,WL_POINTER_BUTTON_STATE_RELEASED);
  read_pipe(pipefd[0],output,sizeof output);assert(strcmp(output,"button 273 0\n")==0 && !desk.extra_down[0] && !window.extra_down[0]);
  vkw_pt_button(NULL,NULL,0,0,BTN_RIGHT,WL_POINTER_BUTTON_STATE_RELEASED);assert(read(pipefd[0],output,sizeof output)<0 && errno==EAGAIN);
  desk.mode=PANEL_FLAT;window.x=640;window.y=360;
  vkw_pt_button(NULL,NULL,0,0,BTN_MIDDLE,WL_POINTER_BUTTON_STATE_PRESSED);read_pipe(pipefd[0],output,sizeof output);
  assert(strcmp(output,"move 320 180\nbutton 274 1\n")==0 && desk.extra_down[1]);
  window.x=-20;window.y=-20;vkw_pt_leave(NULL,NULL,0,NULL);read_pipe(pipefd[0],output,sizeof output);
  assert(strcmp(output,"button 274 0\n")==0 && !desk.extra_down[1] && !window.extra_down[1]);
  vkw_pt_leave(NULL,NULL,0,NULL);vkw_pt_button(NULL,NULL,0,0,BTN_SIDE,WL_POINTER_BUTTON_STATE_PRESSED);
  assert(read(pipefd[0],output,sizeof output)<0 && errno==EAGAIN);
  vkw_pt_button(NULL,NULL,0,0,BTN_RIGHT,WL_POINTER_BUTTON_STATE_PRESSED);
  vkw_pt_button(NULL,NULL,0,0,BTN_RIGHT,WL_POINTER_BUTTON_STATE_RELEASED);
  assert(!desk.extra_down[0] && !window.extra_down[0] && read(pipefd[0],output,sizeof output)<0 && errno==EAGAIN);
  close(pipefd[0]); close(pipefd[1]);

  int pair[2]; assert(socketpair(AF_UNIX,SOCK_STREAM,0,pair)==0); close(pair[1]);
  VkHands hands={.fd=pair[0],.x=500000,.y=500000,.run=4,.down=1,.seen=1,.line_n=3};
  vkhands_poll(&hands,0,640,360,hand_sink);
  assert(pointer_ups==1 && hands.fd==-1 && !hands.down && !hands.seen && !hands.run && !hands.line_n);
  vkhands_poll(&hands,0,640,360,hand_sink); assert(pointer_ups==1);
  puts("ok nested desktop owns close; right/middle callbacks route and release once across panel closure and pointer leave; held keys and hand EOF reconcile");
  return 0;
}
