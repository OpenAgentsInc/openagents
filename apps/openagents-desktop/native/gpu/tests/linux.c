// Firm reason: 6 (the actual Vulkan SDK renders and activates the compiled Coax page).
#include "gallery.h"
#include "../../../../openagents-ios/host.h"
#include "../../../../openagents-ios/core-host.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
// The preview's bounded settling rule: complete the actual animation before
// pressing a row on the page revealed by the closing drawer.
static void render(void *page) {for(int i=0;i<4 || (i<240 && oc_moving(page));i++)assert(oc_shot_frame(page,720,850,0.1f));assert(oc_refused(page)==0 && !oc_moving(page));}
static const char *picture;
static void press(void *page,const char *id) {
 if(!oc_activate(page,id)){
  fprintf(stderr,"Unanswered committed row: %s\n",id);
  for(int i=0;i<oc_rows(page);i++){float box[8];char key[65],label[4097];if(oc_row(page,i,box,key,sizeof key,label,sizeof label))fprintf(stderr,"row %s role %.0f at %.1f %.1f size %.1f %.1f\n",key,box[4],box[0],box[1],box[2],box[3]);}
  (void)oc_shot_write(page,picture);assert(0);
 }render(page);
}
int main(int argc,char **argv) {
 if(argc!=3){fputs("OpenAgentsGPUCheck RESOURCES OUT.png\n",stderr);return 2;}
 picture=argv[2];
 void *page=oc_open_window(argv[1],720,850,1),*core=oa_core_open();assert(page && core);oc_attach(page,core,"");oa_linux_fixture((coax_v_t_AppCore){core,1});oc_step(page);render(page);
 press(page,"nav.menu");press(page,"drawer.settings");press(page,"settings.login");press(page,"login.email");
 char focus[65]={0},value[1024]={0};oc_focus(page,focus,sizeof focus);assert(!strcmp(focus,"login.email"));
 assert(oc_input_text(page,"fixture",7));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture"));
 assert(oc_input_edit(page,"",0,"é",2,0,2));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture"));
 assert(oc_input_edit(page,"@example.test",13,"",0,0,0));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture@example.test"));
 assert(oc_input_paste(page,"x",1));render(page);assert(oc_input_key(page,'z',2,1));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture@example.test"));
 assert(oc_input_key(page,-3,0,1));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture@example.tes"));
 assert(oc_input_key(page,'z',2,1));render(page);oc_field_text(page,value,sizeof value);assert(!strcmp(value,"fixture@example.test"));
 assert(oc_input_key(page,-15,0,1));render(page);oc_focus(page,focus,sizeof focus);assert(!*focus);
 assert(oc_input_key(page,-15,1,1));render(page);oc_focus(page,focus,sizeof focus);assert(!strcmp(focus,"login.email"));
 assert(oc_shot_write(page,argv[2]));oc_blur(page);render(page);oc_focus(page,focus,sizeof focus);assert(!*focus);
 assert(!oc_input_text(page,"stale",5));
 oc_close(page);oa_core_close(core);puts("Vulkan: actual compiled page/core, GPU Settings/login, committed text, provisional IME isolation, undo, Tab/Shift-Tab and stale-field refusal passed; no external compositor/IME ceremony claim");return 0;
}
