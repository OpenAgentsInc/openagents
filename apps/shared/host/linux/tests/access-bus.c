// Firm reason: 6 (a separate AT-SPI SDK client reads the real registered ATK application).
#define main ui_fixture_main
#include "access.c"
#undef main
#include "../access-bus.h"
#include <atspi/atspi.h>
#include <gio/gio.h>
static int client(void) {
 assert(atspi_init()==0);GError *error=NULL;AtspiAccessible *desk=atspi_get_desktop(0);assert(desk);int found=0;
 int apps=atspi_accessible_get_child_count(desk,&error);assert(!error);
 for(int i=0;i<apps;i++){
  AtspiAccessible *app=atspi_accessible_get_child_at_index(desk,i,&error);assert(app && !error);char *name=atspi_accessible_get_name(app,&error);assert(!error);
  if(!strcmp(name,"GPU accessibility fixture")){
   AtspiAccessible *window=atspi_accessible_get_child_at_index(app,0,&error);assert(window && !error && atspi_accessible_get_child_count(window,&error)==14);
   AtspiAccessible *field=atspi_accessible_get_child_at_index(window,3,&error);assert(field && !error && atspi_accessible_get_role(field,&error)==ATSPI_ROLE_ENTRY);
   AtspiText *text=atspi_accessible_get_text_iface(field);char *private=atspi_text_get_text(text,0,-1,&error);assert(!error && private && !*private);g_free(private);g_object_unref(text);g_object_unref(field);
   AtspiAccessible *button=atspi_accessible_get_child_at_index(window,1,&error);AtspiAction *action=atspi_accessible_get_action_iface(button);assert(action && atspi_action_do_action(action,0,&error) && !error);
   g_object_unref(action);g_object_unref(button);g_object_unref(window);found=1;
  }g_free(name);g_object_unref(app);
 }g_object_unref(desk);atspi_exit();assert(found);puts("AT-SPI client: application, window, committed entry, private redaction and real action passed");return 0;
}
typedef struct {GMainLoop *loop;int finished,ok;} Wait;
static void finished(GObject *process,GAsyncResult *result,gpointer raw) {Wait *w=raw;GError *error=NULL;w->ok=g_subprocess_wait_check_finish(G_SUBPROCESS(process),result,&error);g_clear_error(&error);w->finished=1;g_main_loop_quit(w->loop);}
static gboolean timeout(gpointer raw) {Wait *w=raw;g_main_loop_quit(w->loop);return G_SOURCE_REMOVE;}
int main(int argc,char **argv) {
 if(argc==2 && !strcmp(argv[1],"--client"))return client();
 UiApp app={.title="GPU accessibility fixture",.access_capacity=14,.elements=elements,.element=element,.activate=activate,.changed=changed};
 count=14;UiAccess *a=ui_access_create(&app);assert(a && ui_access_sync(a) && ui_access_bus_start(a));
 Wait w={.loop=g_main_loop_new(NULL,FALSE)};GError *error=NULL;GSubprocess *child=g_subprocess_new(G_SUBPROCESS_FLAGS_NONE,&error,argv[0],"--client",NULL);assert(child && !error);
 g_subprocess_wait_check_async(child,NULL,finished,&w);guint deadline=g_timeout_add_seconds(8,timeout,&w);g_main_loop_run(w.loop);
 if(w.finished)g_source_remove(deadline);else g_subprocess_force_exit(child);
 assert(w.finished && w.ok && presses==1 && wakes==1);g_object_unref(child);g_main_loop_unref(w.loop);ui_access_destroy(a);ui_access_bus_stop();return 0;
}
