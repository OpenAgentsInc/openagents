// Firm reason: 6 (the actual ATK SDK as an independent reader of committed GPU semantics).
#include "../access.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static int count=14,presses,wakes,bad;
static UiAccess *closing;
static int elements(void *p) {(void)p;return count;}
static int element(void *p,int i,UiElement *r) {
 (void)p;memset(r,0,sizeof *r);snprintf(r->id,sizeof r->id,"fixture.%d",i);snprintf(r->label,sizeof r->label,"Row %d 日本",i);
 r->role=i;r->parent=-1;r->box[0]=10;r->box[1]=(float)i*30;r->box[2]=200;r->box[3]=28;
 if(i==0)r->state=1;
 if(i==6)r->state=256|32;
 if(i==3)snprintf(r->label,sizeof r->label,"Private key");
 if(bad && i==1)snprintf(r->id,sizeof r->id,"fixture.0");
 return 1;
}
static int activate(void *p,const char *id) {(void)p;if(!strcmp(id,"fixture.1")){presses++;if(closing){ui_access_destroy(closing);closing=NULL;}return 1;}return 0;}
static void changed(void *p) {(void)p;wakes++;}
int main(void) {
 UiApp app={.title="GPU accessibility fixture",.app_id="com.openagents.fixture",.access_capacity=14,.elements=elements,.element=element,.activate=activate,.changed=changed};
 UiAccess *a=ui_access_create(&app);assert(a && ui_access_sync(a));AtkObject *window=ui_access_window(a);assert(atk_object_get_n_accessible_children(window)==14);
 AtkObject *first=atk_object_ref_accessible_child(window,0),*second=atk_object_ref_accessible_child(window,1),*field=atk_object_ref_accessible_child(window,3),*check=atk_object_ref_accessible_child(window,6);
 assert(atk_object_get_role(first)==ATK_ROLE_PUSH_BUTTON && atk_object_get_role(field)==ATK_ROLE_ENTRY);
 assert(atk_action_get_n_actions(ATK_ACTION(first))==0 && !atk_action_do_action(ATK_ACTION(first),0));
 assert(atk_action_do_action(ATK_ACTION(second),0) && presses==1 && wakes==1);
 AtkStateSet *states=atk_object_ref_state_set(check);assert(atk_state_set_contains_state(states,ATK_STATE_CHECKED));g_object_unref(states);
 char *private=atk_text_get_text(ATK_TEXT(field),0,-1);assert(private && !*private);g_free(private);
 char *text=atk_text_get_text(ATK_TEXT(second),6,-1);assert(!strcmp(text,"日本"));g_free(text);
 int x,y,w,h;atk_component_get_extents(ATK_COMPONENT(second),&x,&y,&w,&h,ATK_XY_WINDOW);assert(x==10 && y==30 && w==200 && h==28);
 bad=1;assert(!ui_access_sync(a) && atk_object_get_n_accessible_children(window)==14);bad=0;
 count=15;assert(!ui_access_sync(a) && atk_object_get_n_accessible_children(window)==14);count=14;
 assert(ui_access_sync(a));AtkObject *same=atk_object_ref_accessible_child(window,1);assert(same==second);g_object_unref(same);
 count=0;assert(ui_access_sync(a) && atk_object_get_n_accessible_children(window)==0 && !atk_action_do_action(ATK_ACTION(second),0));
 states=atk_object_ref_state_set(second);assert(atk_state_set_contains_state(states,ATK_STATE_DEFUNCT));g_object_unref(states);
 ui_access_destroy(a);assert(!atk_action_do_action(ATK_ACTION(field),0));
 g_object_unref(first);g_object_unref(second);g_object_unref(field);g_object_unref(check);
 count=14;closing=ui_access_create(&app);assert(closing && ui_access_sync(closing));second=atk_object_ref_accessible_child(ui_access_window(closing),1);
 int before=wakes;assert(atk_action_do_action(ATK_ACTION(second),0) && !closing && wakes==before);g_object_unref(second);
 puts("ATK: committed roles, states, Unicode text, private value, geometry, real action, stable objects, atomic refusal and retirement passed");return 0;
}
