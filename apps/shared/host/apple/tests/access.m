// Firm reason: 6 (AppKit/Metal SDK qualification of the committed-row accessibility bridge).
#import <AppKit/AppKit.h>
#include "../ui-host-apple.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static int presses,legacy_reads,count=14;
static int rows(void *user) {(void)user;return count;}
static int row(void *user,int i,UiElement *out) {
 (void)user;memset(out,0,sizeof *out);out->role=i;out->parent=-1;
 snprintf(out->id,sizeof out->id,"fixture.%d",i);snprintf(out->label,sizeof out->label,"Row %d",i);
 out->box[1]=(float)i*30;out->box[2]=200;out->box[3]=28;
 if(i==0)out->state=1; // disabled; never pressable
 if(i==6)out->state=256|32; // checked checkbox
 if(i==3)snprintf(out->label,sizeof out->label,"Private key"); // no private value
 return 1;
}
static int press(void *user,const char *id) {(void)user;if(!strcmp(id,"fixture.1")){presses++;return 1;}return 0;}
static int legacy(void *user,const char *op,char *out,int cap) {(void)user;(void)op;(void)out;(void)cap;legacy_reads++;return -1;}
int main(int argc,char **argv) {
 @autoreleasepool {
  assert(argc==2);UiApp app={.title="Accessibility qualification",.width=400,.height=600,.max_vertices=6,.shader=argv[1],.access_capacity=14,.elements=rows,.element=row,.activate=press,.drive=legacy};
  UiHost *h=ui_open(&app,1);assert(h);char out[32768]={0};
  int n=ui_drive(h,"ax",out,sizeof out);assert(n>0 && strstr(out,"fixture.9") && strstr(out,"fixture.8") && strstr(out,"fixture.6 AXCheckBox") && strstr(out,"fixture.12 AXProgressIndicator") && strstr(out,"fixture.11 AXImage"));
  assert(strstr(out,"fixture.0 AXButton \"Row 0\" disabled") && strstr(out,"fixture.3 AXTextField \"Private key\"") && !strstr(out,"fixture.4 "));
  assert(ui_drive(h,"ax-press fixture.0",out,sizeof out)>0 && strstr(out,"not pressed") && presses==0);
  assert(ui_drive(h,"ax-press fixture.1",out,sizeof out)>0 && strstr(out,"pressed fixture.1") && presses==1 && legacy_reads==0);
  assert(ui_drive(h,"ax-key tab",out,sizeof out)>0 && strstr(out,"focus fixture.1"));
  assert(ui_drive(h,"ax-key space",out,sizeof out)>0 && presses==2 && legacy_reads==0);
  [NSApplication sharedApplication];[NSApp setActivationPolicy:NSApplicationActivationPolicyProhibited];
  // The active-editor adapter publishes its role explicitly even in a hidden
  // window. The ordinary committed-row oracle stays offscreen.
  app.hidden=1;UiHost *with_editor=ui_open(&app,0);assert(with_editor);
  NSTextField *editor=[[NSTextField alloc]initWithFrame:NSMakeRect(0,0,200,28)];
  editor.accessibilityIdentifier=@"fixture.3";editor.accessibilityLabel=@"Active email";
  editor.accessibilityElement=YES;editor.accessibilityRole=NSAccessibilityTextFieldRole;
  [(__bridge NSView *)ui_view(with_editor) addSubview:editor];
  ui_access_editor(with_editor,"fixture.3",(__bridge void *)editor);
  int shown=ui_drive(with_editor,"ax",out,sizeof out);
  assert(shown>0 && strstr(out,"fixture.3 AXTextField") && strstr(out,"\"Active email\""));
  char *first=strstr(out,"fixture.3 ");assert(first && !strstr(first+1,"fixture.3 "));
  ui_access_editor(with_editor,"",NULL);
  assert(ui_drive(with_editor,"ax",out,sizeof out)>0 && strstr(out,"fixture.3 AXTextField \"Private key\""));
  ui_close(with_editor);(void)editor.accessibilityFrame;
  count=15;assert(ui_drive(h,"ax",out,sizeof out)>0 && strstr(out,"fixture.9")); // overflow retains prior rows
  count=0;assert(ui_drive(h,"ax",out,sizeof out)>0 && strstr(out,"no elements"));
  ui_close(h);(void)editor.accessibilityFrame;
  puts("AppKit: all committed roles, private field metadata, disabled/real press, capacity refusal, active-editor replacement and retirement passed; no legacy tree read");
 }
 return 0;
}
