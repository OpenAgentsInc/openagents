// Firm reason: 6 (GTK/Wayland native controls, clipboard and UI-thread scheduling).
// Coax supplies the component projection, application reducer and all services.
#include <gtk/gtk.h>
#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

typedef struct {const char *data;int64_t count;} Text;
typedef struct {void *data;int64_t count;} Span;
typedef struct {unsigned char *data;int64_t count;} Bytes;
extern int64_t nd_size(void),ls_size(void),nd_frame(Span),nd_kind(Span,int64_t),nd_text(Span,int64_t,Bytes),nd_action(Span),ls_status(Span);
extern Text nd_id(Span,int64_t),nd_value(Span,int64_t),nd_body(Span);
extern _Bool nd_enabled(Span,int64_t);
extern void nd_input(Span,Text,Text,_Bool),nd_done(Span),nd_reply(Span,int64_t,int64_t,Text,_Bool);
extern int64_t ls_call(Span,int64_t,Text,Bytes);
static Span state,services;
static GtkWidget *box;
static GHashTable *controls;
static int rendering,working;
static GtkApplication *application;
static int inspect_ui,inspection_passed,inspection_stage,inspection_ticks;
typedef struct {char *id;GtkWidget *widget;int kind;unsigned seen;} Control;
typedef struct {int op,status;char *body,*result;int64_t n;} Job;
static unsigned frame;
static Text text(const char *s) {return (Text){s,(int64_t)strlen(s)};}
static char *owned(Text t) {return g_strndup(t.data,(gsize)t.count);}
static void pump(void);
static gboolean inspect(gpointer unused) {
  (void)unused;if(++inspection_ticks>200) {fputs("Native Linux control inspection timed out\n",stderr);g_application_quit(G_APPLICATION(application));return G_SOURCE_REMOVE;}
  if(working)return G_SOURCE_CONTINUE;
  if(inspection_stage==0) {
    Control *entry=g_hash_table_lookup(controls,"login.email"),*local=g_hash_table_lookup(controls,"environments.local");
    if(!entry || !local)return G_SOURCE_CONTINUE;
    gtk_editable_set_text(GTK_EDITABLE(entry->widget),"fixture@example.test");
    if(strcmp(gtk_editable_get_text(GTK_EDITABLE(entry->widget)),"fixture@example.test"))return G_SOURCE_CONTINUE;
    inspection_stage=1;g_signal_emit_by_name(local->widget,"clicked");
  }else if(inspection_stage==1) {
    Control *create=g_hash_table_lookup(controls,"env.new");if(!create)return G_SOURCE_CONTINUE;inspection_stage=2;g_signal_emit_by_name(create->widget,"clicked");
  }else {
    Control *composer=g_hash_table_lookup(controls,"env.composer"),*send=g_hash_table_lookup(controls,"env.composer.send");if(!composer || !send || !GTK_IS_TEXT_VIEW(composer->widget) || gtk_widget_get_width(composer->widget)<1)return G_SOURCE_CONTINUE;
    gtk_text_buffer_set_text(gtk_text_view_get_buffer(GTK_TEXT_VIEW(composer->widget)),"An unsent native draft",-1);
    if(!gtk_widget_get_sensitive(send->widget))return G_SOURCE_CONTINUE;
    inspection_passed=1;puts("Coax Native Linux: GTK controls, local service catalog and multiline draft passed; no turn sent");g_application_quit(G_APPLICATION(application));return G_SOURCE_REMOVE;
  }return G_SOURCE_CONTINUE;
}
static void pressed(GtkButton *button,gpointer raw) { (void)button;Control *c=raw;nd_input(state,text(c->id),text(""),1);pump();}
static void changed(GtkEditable *field,gpointer raw) {if(rendering)return;Control *c=raw;nd_input(state,text(c->id),text(gtk_editable_get_text(field)),0);pump();}
static void buffer_changed(GtkTextBuffer *buffer,gpointer raw) {
  if(rendering)return;Control *c=raw;GtkTextIter a,b;gtk_text_buffer_get_bounds(buffer,&a,&b);char *value=gtk_text_buffer_get_text(buffer,&a,&b,0);nd_input(state,text(c->id),text(value),0);g_free(value);pump();
}
static void render(void) {
  int64_t n=nd_frame(state);frame++;rendering=1;
  for(int64_t i=0;i<n;i++) {
    int kind=(int)nd_kind(state,i);if(!kind)continue;
    char *id=owned(nd_id(state,i));unsigned char label[32769];int64_t length=nd_text(state,i,(Bytes){label,32768});if(length<0)length=0;label[length]=0;
    Control *c=g_hash_table_lookup(controls,id);
    if(!c) {
      c=g_new0(Control,1);c->id=g_strdup(id);c->kind=kind;
      if(kind==1) {c->widget=gtk_button_new();g_signal_connect(c->widget,"clicked",G_CALLBACK(pressed),c);}
      else if(kind==2) {c->widget=gtk_entry_new();g_signal_connect(c->widget,"changed",G_CALLBACK(changed),c);}
      else if(kind==4) {c->widget=gtk_text_view_new();gtk_text_view_set_wrap_mode(GTK_TEXT_VIEW(c->widget),GTK_WRAP_WORD_CHAR);gtk_widget_set_size_request(c->widget,-1,100);g_signal_connect(gtk_text_view_get_buffer(GTK_TEXT_VIEW(c->widget)),"changed",G_CALLBACK(buffer_changed),c);}
      else {c->widget=gtk_label_new("");gtk_label_set_wrap(GTK_LABEL(c->widget),1);gtk_label_set_wrap_mode(GTK_LABEL(c->widget),PANGO_WRAP_WORD_CHAR);gtk_label_set_xalign(GTK_LABEL(c->widget),0);gtk_label_set_selectable(GTK_LABEL(c->widget),1);}
      gtk_widget_set_hexpand(c->widget,1);gtk_widget_set_name(c->widget,id);g_hash_table_insert(controls,c->id,c);gtk_box_append(GTK_BOX(box),c->widget);
    }
    c->seen=frame;gtk_widget_set_sensitive(c->widget,nd_enabled(state,i));
    gtk_accessible_update_property(GTK_ACCESSIBLE(c->widget),GTK_ACCESSIBLE_PROPERTY_LABEL,(const char *)label,-1);
    if(kind==1)gtk_button_set_label(GTK_BUTTON(c->widget),(char *)label);
    else if(kind==2) {char *value=owned(nd_value(state,i));if(strcmp(value,gtk_editable_get_text(GTK_EDITABLE(c->widget))))gtk_editable_set_text(GTK_EDITABLE(c->widget),value);gtk_entry_set_placeholder_text(GTK_ENTRY(c->widget),(char *)label);g_free(value);}
    else if(kind==4) {
      char *value=owned(nd_value(state,i));GtkTextBuffer *buffer=gtk_text_view_get_buffer(GTK_TEXT_VIEW(c->widget));GtkTextIter a,b;gtk_text_buffer_get_bounds(buffer,&a,&b);char *old=gtk_text_buffer_get_text(buffer,&a,&b,0);
      if(strcmp(old,value))gtk_text_buffer_set_text(buffer,value,-1);g_free(old);g_free(value);
    }else gtk_label_set_text(GTK_LABEL(c->widget),(char *)label);
    g_free(id);
  }
  GHashTableIter it;gpointer key,value;g_hash_table_iter_init(&it,controls);
  while(g_hash_table_iter_next(&it,&key,&value)) {Control *c=value;if(c->seen!=frame) {gtk_box_remove(GTK_BOX(box),c->widget);g_hash_table_iter_remove(&it);g_free(c->id);g_free(c);}}
  rendering=0;
}
static gboolean completed(gpointer raw) {
  Job *j=raw;working=0;nd_reply(state,j->op,j->status,j->n<0 ? text("{\"error\":\"Service unavailable. Check the connection and login.\"}"):(Text){j->result,j->n},j->n<0);g_free(j->body);g_free(j->result);g_free(j);pump();return G_SOURCE_REMOVE;
}
static void *worker(void *raw) {Job *j=raw;j->result=g_malloc0(1048577);j->n=ls_call(services,j->op,text(j->body),(Bytes){(unsigned char *)j->result,1048576});j->status=(int)ls_status(services);g_idle_add(completed,j);return NULL;}
static gboolean timer(gpointer raw) {(void)raw;working=0;nd_reply(state,12,0,text("{\"ready\":true}"),0);pump();return G_SOURCE_REMOVE;}
static void pump(void) {
  render();if(working)return;int op=(int)nd_action(state);if(!op)return;char *body=owned(nd_body(state));nd_done(state);
  if(op==12) {working=1;g_timeout_add(1000,timer,NULL);g_free(body);return;}
  Job *j=g_new0(Job,1);j->op=op;j->body=body;working=1;pthread_t thread;pthread_attr_t attr;pthread_attr_init(&attr);pthread_attr_setstacksize(&attr,134217728);
  if(pthread_create(&thread,&attr,worker,j)!=0) {j->n=-1;completed(j);}else pthread_detach(thread);pthread_attr_destroy(&attr);
}
static void activate(GtkApplication *app,gpointer unused) {
  (void)unused;GtkWidget *window=gtk_application_window_new(app);gtk_window_set_title(GTK_WINDOW(window),"OpenAgents");gtk_window_set_default_size(GTK_WINDOW(window),680,850);
  GtkWidget *scroll=gtk_scrolled_window_new();box=gtk_box_new(GTK_ORIENTATION_VERTICAL,14);gtk_widget_set_margin_start(box,24);gtk_widget_set_margin_end(box,24);gtk_widget_set_margin_top(box,24);gtk_widget_set_margin_bottom(box,24);gtk_scrolled_window_set_child(GTK_SCROLLED_WINDOW(scroll),box);gtk_window_set_child(GTK_WINDOW(window),scroll);pump();gtk_window_present(GTK_WINDOW(window));
  if(inspect_ui)g_timeout_add(50,inspect,NULL);
}
int main(int argc,char **argv) {
  state=(Span){calloc(1,(size_t)nd_size()),1};services=(Span){calloc(1,(size_t)ls_size()),1};controls=g_hash_table_new(g_str_hash,g_str_equal);
  if(argc==2 && !strcmp(argv[1],"--inspect-program")) {int64_t n=nd_frame(state);nd_done(state);nd_reply(state,1,0,text("{\"loggedIn\":false,\"localEnvironment\":true}"),0);n=nd_frame(state);int found=0;for(int64_t i=0;i<n;i++){char *id=owned(nd_id(state,i));if(!strcmp(id,"environments.local"))found=1;g_free(id);}puts(found?"Coax Native Linux: generated program and local access semantics passed":"Native program inspection failed");return found?0:2;}
  if(argc==2 && !strcmp(argv[1],"--inspect-ui")) {inspect_ui=1;argc=1;}
  GtkApplication *app=gtk_application_new(inspect_ui ? "com.openagents.desktop.qualifier":"com.openagents.desktop",G_APPLICATION_DEFAULT_FLAGS);application=app;g_signal_connect(app,"activate",G_CALLBACK(activate),NULL);int status=g_application_run(G_APPLICATION(app),argc,argv);g_object_unref(app);return inspect_ui ? (inspection_passed?0:2):status;
}
