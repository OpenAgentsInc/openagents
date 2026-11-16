// Firm reason: 6 (the AT-SPI SDK bridge; no D-Bus protocol or application state is implemented here).
#include "access-bus.h"
#include <atk-bridge.h>
#include <string.h>
typedef struct {AtkObject parent;GPtrArray *windows;} UiAtkRoot;
typedef struct {AtkObjectClass parent;} UiAtkRootClass;
G_DEFINE_TYPE(UiAtkRoot,ui_atk_root,ATK_TYPE_OBJECT)
static UiAtkRoot *root;
static AtkUtilClass *utility;
static int started;
static gint ui_root_count(AtkObject *o) {return (gint)((UiAtkRoot *)o)->windows->len;}
static AtkObject *ui_root_child(AtkObject *o,gint i) {UiAtkRoot *r=(UiAtkRoot *)o;return i<0 || (guint)i>=r->windows->len?NULL:g_object_ref(g_ptr_array_index(r->windows,(guint)i));}
static void ui_root_finalize(GObject *o) {g_ptr_array_free(((UiAtkRoot *)o)->windows,TRUE);G_OBJECT_CLASS(ui_atk_root_parent_class)->finalize(o);}
static void ui_atk_root_class_init(UiAtkRootClass *c) {AtkObjectClass *a=ATK_OBJECT_CLASS(c);a->get_n_children=ui_root_count;a->ref_child=ui_root_child;G_OBJECT_CLASS(c)->finalize=ui_root_finalize;}
static void ui_atk_root_init(UiAtkRoot *r) {r->windows=g_ptr_array_new_with_free_func(g_object_unref);atk_object_set_role(ATK_OBJECT(r),ATK_ROLE_APPLICATION);}
static AtkObject *ui_root(void) {return root?ATK_OBJECT(root):NULL;}
static const gchar *ui_toolkit(void) {return "Coax Native";}
static const gchar *ui_version(void) {return "1";}
static void ui_bus_retired(AtkObject *window,const gchar *state,gboolean on,gpointer raw) {
 UiAtkRoot *r=raw;if(!on || strcmp(state,"defunct"))return;
 guint i;if(g_ptr_array_find(r->windows,window,&i)){atk_object_set_parent(window,NULL);g_signal_emit_by_name(r,"children-changed::remove",i,window,NULL);g_ptr_array_remove_index(r->windows,i);}
}
int ui_access_bus_start(UiAccess *access) {
 AtkObject *window=ui_access_window(access);if(!window)return 0;
 if(!root){
  // Refuse a second toolkit's application root rather than replacing it.
  if(atk_get_root())return 0;
  root=g_object_new(ui_atk_root_get_type(),NULL);atk_object_set_name(ATK_OBJECT(root),atk_object_get_name(window));
  utility=g_type_class_ref(ATK_TYPE_UTIL);utility->get_root=ui_root;utility->get_toolkit_name=ui_toolkit;utility->get_toolkit_version=ui_version;
 }
 if(root->windows->len>=16)return 0;
 if(!g_ptr_array_find(root->windows,window,NULL)){
  guint index=root->windows->len;g_ptr_array_add(root->windows,g_object_ref(window));atk_object_set_parent(window,ATK_OBJECT(root));
  g_signal_connect(window,"state-change",G_CALLBACK(ui_bus_retired),root);g_signal_emit_by_name(root,"children-changed::add",index,window,NULL);
 }
 if(!started){if(atk_bridge_adaptor_init(NULL,NULL)!=0)return 0;started=1;}
 return 1;
}
void ui_access_bus_pump(void) {for(int i=0;i<64 && g_main_context_pending(NULL);i++)g_main_context_iteration(NULL,FALSE);}
void ui_access_bus_stop(void) {
 if(started){atk_bridge_adaptor_cleanup();started=0;}
 if(root){for(guint i=0;i<root->windows->len;i++){AtkObject *w=g_ptr_array_index(root->windows,i);g_signal_handlers_disconnect_by_func(w,ui_bus_retired,root);atk_object_set_parent(w,NULL);}g_object_unref(root);root=NULL;}
 if(utility){utility->get_root=NULL;utility->get_toolkit_name=NULL;utility->get_toolkit_version=NULL;g_type_class_unref(utility);utility=NULL;}
}
