// Firm reason: 6 (ATK object lifetimes and SDK vtables; Coax supplies every committed semantic row).
#include "access.h"
#include <math.h>
#include <string.h>

typedef struct {AtkObject parent;struct UiAccess *owner;UiElement row;int index,window;} UiAtkNode;
typedef struct {AtkObjectClass parent;} UiAtkNodeClass;
struct UiAccess {UiApp app;UiAtkNode *window;GPtrArray *nodes;GHashTable *ids;};
static void ui_atk_component_init(AtkComponentIface *iface);
static void ui_atk_action_init(AtkActionIface *iface);
static void ui_atk_text_init(AtkTextIface *iface);
G_DEFINE_TYPE_WITH_CODE(UiAtkNode,ui_atk_node,ATK_TYPE_OBJECT,
 G_IMPLEMENT_INTERFACE(ATK_TYPE_COMPONENT,ui_atk_component_init)
 G_IMPLEMENT_INTERFACE(ATK_TYPE_ACTION,ui_atk_action_init)
 G_IMPLEMENT_INTERFACE(ATK_TYPE_TEXT,ui_atk_text_init))

static UiAtkNode *ui_atk_parent_node(UiAtkNode *n) {
 if(!n->owner || n->window)return NULL;
 if(n->row.parent>=0)for(guint i=0;i<n->owner->nodes->len;i++) {UiAtkNode *p=g_ptr_array_index(n->owner->nodes,i);if(p->index==n->row.parent)return p;}
 return n->owner->window;
}
static AtkObject *ui_atk_parent(AtkObject *raw) {UiAtkNode *n=(UiAtkNode *)raw;return ATK_OBJECT(ui_atk_parent_node(n));}
static gint ui_atk_children(AtkObject *raw) {
 UiAtkNode *n=(UiAtkNode *)raw;if(!n->owner)return 0;int count=0;
 for(guint i=0;i<n->owner->nodes->len;i++)if(ui_atk_parent_node(g_ptr_array_index(n->owner->nodes,i))==n)count++;
 return count;
}
static AtkObject *ui_atk_child(AtkObject *raw,gint at) {
 UiAtkNode *n=(UiAtkNode *)raw;if(!n->owner || at<0)return NULL;int index=0;
 for(guint i=0;i<n->owner->nodes->len;i++){UiAtkNode *child=g_ptr_array_index(n->owner->nodes,i);if(ui_atk_parent_node(child)==n && index++==at)return ATK_OBJECT(g_object_ref(child));}
 return NULL;
}
static gint ui_atk_index(AtkObject *raw) {
 UiAtkNode *n=(UiAtkNode *)raw,*parent=ui_atk_parent_node(n);if(!parent)return -1;int index=0;
 for(guint i=0;i<n->owner->nodes->len;i++){UiAtkNode *child=g_ptr_array_index(n->owner->nodes,i);if(child==n)return index;if(ui_atk_parent_node(child)==parent)index++;}return -1;
}
static int ui_atk_pressable(UiAtkNode *n) {return n->owner && !n->window && !(n->row.state&1) && (n->row.role==0 || n->row.role==1 || n->row.role==2 || n->row.role==3 || n->row.role==6 || n->row.role==7 || n->row.role==10);}
static AtkStateSet *ui_atk_states(AtkObject *raw) {
 UiAtkNode *n=(UiAtkNode *)raw;AtkStateSet *s=atk_state_set_new();
 if(!n->owner){atk_state_set_add_state(s,ATK_STATE_DEFUNCT);return s;}
 atk_state_set_add_state(s,ATK_STATE_VISIBLE);atk_state_set_add_state(s,ATK_STATE_SHOWING);
 if(!(n->row.state&1)){atk_state_set_add_state(s,ATK_STATE_ENABLED);atk_state_set_add_state(s,ATK_STATE_SENSITIVE);}
 if(ui_atk_pressable(n))atk_state_set_add_state(s,ATK_STATE_FOCUSABLE);
 if(n->row.state&2)atk_state_set_add_state(s,ATK_STATE_BUSY);
 if(n->row.state&64)atk_state_set_add_state(s,ATK_STATE_FOCUSED);
 if(n->row.state&128){atk_state_set_add_state(s,ATK_STATE_EXPANDABLE);if(n->row.state&8)atk_state_set_add_state(s,ATK_STATE_EXPANDED);}
 if(n->row.state&256 && n->row.state&32)atk_state_set_add_state(s,ATK_STATE_CHECKED);
 if(n->row.state&512 && n->row.state&4)atk_state_set_add_state(s,ATK_STATE_PRESSED);
 if(n->row.state&16)atk_state_set_add_state(s,ATK_STATE_SELECTED);
 return s;
}
static void ui_atk_node_class_init(UiAtkNodeClass *c) {AtkObjectClass *a=ATK_OBJECT_CLASS(c);a->get_parent=ui_atk_parent;a->get_n_children=ui_atk_children;a->ref_child=ui_atk_child;a->get_index_in_parent=ui_atk_index;a->ref_state_set=ui_atk_states;}
static void ui_atk_node_init(UiAtkNode *n) {n->index=-1;}
static const AtkRole ui_atk_roles[14]={ATK_ROLE_PUSH_BUTTON,ATK_ROLE_LIST_ITEM,ATK_ROLE_PAGE_TAB,ATK_ROLE_ENTRY,ATK_ROLE_PANEL,ATK_ROLE_SECTION,ATK_ROLE_CHECK_BOX,ATK_ROLE_SLIDER,ATK_ROLE_STATIC,ATK_ROLE_HEADING,ATK_ROLE_LINK,ATK_ROLE_IMAGE,ATK_ROLE_PROGRESS_BAR,ATK_ROLE_ALERT};
static void ui_atk_extents(AtkComponent *raw,gint *x,gint *y,gint *w,gint *h,AtkCoordType type) {
 (void)type;UiAtkNode *n=(UiAtkNode *)raw;int live=n->owner!=NULL;
 if(x)*x=live?(int)n->row.box[0]:0;
 if(y)*y=live?(int)n->row.box[1]:0;
 if(w)*w=live?(int)n->row.box[2]:0;
 if(h)*h=live?(int)n->row.box[3]:0;
}
static AtkLayer ui_atk_layer(AtkComponent *raw) {(void)raw;return ATK_LAYER_WIDGET;}
static void ui_atk_component_init(AtkComponentIface *i) {i->get_extents=ui_atk_extents;i->get_layer=ui_atk_layer;}
static gint ui_atk_actions(AtkAction *raw) {return ui_atk_pressable((UiAtkNode *)raw)?1:0;}
static const gchar *ui_atk_action_name(AtkAction *raw,gint index) {return index==0 && ui_atk_actions(raw)?"press":NULL;}
static gboolean ui_atk_do(AtkAction *raw,gint index) {
 UiAtkNode *n=(UiAtkNode *)raw;if(index!=0 || !ui_atk_pressable(n) || !n->owner->app.activate)return FALSE;
 UiAccess *a=n->owner;UiApp app=a->app;g_object_ref(n);int ok=app.activate(app.user,n->row.id);
 if(ok && n->owner==a && app.changed)app.changed(app.user);
 g_object_unref(n);return ok!=0;
}
static void ui_atk_action_init(AtkActionIface *i) {i->get_n_actions=ui_atk_actions;i->get_name=ui_atk_action_name;i->do_action=ui_atk_do;}
static const char *ui_atk_value(UiAtkNode *n) {return !n->owner?"":n->row.role==3?n->row.value:n->row.label;}
static gint ui_atk_text_count(AtkText *raw) {return (gint)g_utf8_strlen(ui_atk_value((UiAtkNode *)raw),-1);}
static gchar *ui_atk_text(AtkText *raw,gint start,gint end) {
 const char *text=ui_atk_value((UiAtkNode *)raw);gint count=ui_atk_text_count(raw);if(start<0)start=0;if(end<0 || end>count)end=count;if(start>end)return g_strdup("");return g_utf8_substring(text,start,end);
}
static gunichar ui_atk_character(AtkText *raw,gint at) {const char *text=ui_atk_value((UiAtkNode *)raw);return at<0 || at>=ui_atk_text_count(raw)?0:g_utf8_get_char(g_utf8_offset_to_pointer(text,at));}
static void ui_atk_text_init(AtkTextIface *i) {i->get_text=ui_atk_text;i->get_character_count=ui_atk_text_count;i->get_character_at_offset=ui_atk_character;}
UiAccess *ui_access_create(const UiApp *app) {
 if(!app || !app->elements || !app->element || !app->access_capacity || app->access_capacity>4096)return NULL;
 UiAccess *a=g_new0(UiAccess,1);a->app=*app;a->nodes=g_ptr_array_new();a->ids=g_hash_table_new_full(g_str_hash,g_str_equal,g_free,g_object_unref);
 a->window=g_object_new(ui_atk_node_get_type(),NULL);a->window->owner=a;a->window->window=1;atk_object_set_role(ATK_OBJECT(a->window),ATK_ROLE_FRAME);atk_object_set_name(ATK_OBJECT(a->window),app->title?app->title:"");return a;
}
static int ui_atk_valid(UiElement *r,int index) {
 if(!memchr(r->id,0,sizeof r->id) || !*r->id || !memchr(r->label,0,sizeof r->label) || !memchr(r->value,0,sizeof r->value))return 0;
 if(!g_utf8_validate(r->id,-1,NULL) || !g_utf8_validate(r->label,-1,NULL) || !g_utf8_validate(r->value,-1,NULL) || r->role<0 || r->role>=14 || r->parent>=index || r->parent< -1)return 0;
 for(int i=0;i<4;i++)if(!isfinite(r->box[i]) || fabsf(r->box[i])>1000000)return 0;
 return r->box[2]>=0 && r->box[3]>=0;
}
int ui_access_sync(UiAccess *a) {
 if(!a)return 0;
 int count=a->app.elements(a->app.user);if(count<0 || (uint32_t)count>a->app.access_capacity)return 0;
 UiElement *rows=g_new0(UiElement,count);unsigned char *valid=g_new0(unsigned char,count);GHashTable *seen=g_hash_table_new(g_str_hash,g_str_equal);int ok=1;
 for(int i=0;i<count;i++){if(!a->app.element(a->app.user,i,&rows[i]))continue;if(!ui_atk_valid(&rows[i],i) || g_hash_table_contains(seen,rows[i].id)){ok=0;break;}g_hash_table_add(seen,rows[i].id);valid[i]=rows[i].box[2]>0 && rows[i].box[3]>0;}
 if(!ok){g_hash_table_destroy(seen);g_free(valid);g_free(rows);return 0;}
 GPtrArray *next=g_ptr_array_new();for(int i=0;i<count;i++){if(!valid[i])continue;UiAtkNode *n=g_hash_table_lookup(a->ids,rows[i].id);
  if(!n){n=g_object_new(ui_atk_node_get_type(),NULL);g_hash_table_insert(a->ids,g_strdup(rows[i].id),n);}
  n->owner=a;n->row=rows[i];n->index=i;atk_object_set_role(ATK_OBJECT(n),ui_atk_roles[n->row.role]);atk_object_set_name(ATK_OBJECT(n),n->row.label);atk_object_set_accessible_id(ATK_OBJECT(n),n->row.id);g_ptr_array_add(next,n);
 }
 GHashTableIter it;gpointer key,value;g_hash_table_iter_init(&it,a->ids);while(g_hash_table_iter_next(&it,&key,&value)){UiAtkNode *n=value;if(!g_ptr_array_find(next,n,NULL)){n->owner=NULL;atk_object_notify_state_change(ATK_OBJECT(n),ATK_STATE_DEFUNCT,TRUE);g_hash_table_iter_remove(&it);}}
 g_ptr_array_free(a->nodes,TRUE);a->nodes=next;g_hash_table_destroy(seen);g_free(valid);g_free(rows);return 1;
}
AtkObject *ui_access_window(UiAccess *a) {return a?ATK_OBJECT(a->window):NULL;}
void ui_access_destroy(UiAccess *a) {
 if(!a)return;
 for(guint i=0;i<a->nodes->len;i++){UiAtkNode *n=g_ptr_array_index(a->nodes,i);n->owner=NULL;atk_object_notify_state_change(ATK_OBJECT(n),ATK_STATE_DEFUNCT,TRUE);}
 a->window->owner=NULL;g_hash_table_destroy(a->ids);g_ptr_array_free(a->nodes,TRUE);g_object_unref(a->window);g_free(a);
}
