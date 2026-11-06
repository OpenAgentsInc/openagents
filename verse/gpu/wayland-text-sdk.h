// Firm reason: 6 (Wayland text-input/data-device SDK and owned nonblocking descriptors; Coax admits text, bounds, deadlines and owners).
#ifndef AETHYR_WAYLAND_TEXT_SDK_H
#define AETHYR_WAYLAND_TEXT_SDK_H
#include <fcntl.h>
#include <errno.h>
#include <time.h>
#include <signal.h>
#include <pthread.h>
#include "wayland-text.h"
#include "text-input-unstable-v3-client-protocol.h"
typedef struct {
 void *user;
 void (*text)(void*,const char*,size_t);
 int (*wants)(void*);
 int (*rect)(void*,float[4]);
 uint64_t (*owner)(void*);
 void (*error)(void*,const char*);
 void (*cancel_compose)(void*);void *cancel_user;
} HwtTo;
typedef struct HwtSdk HwtSdk;
typedef struct {HwtSdk *host;struct wl_data_offer *offer;int mime;} HwtOffer;
typedef struct {HwtSdk *host;struct wl_data_source *source;char *bytes;size_t length;int refs,current;} HwtSource;
typedef struct {int fd;size_t sent;int64_t start;HwtSource *source;} HwtSend;
struct HwtSdk {
 HwtTo to;struct wl_surface *surface;struct wl_seat *seat;
 struct zwp_text_input_manager_v3 *manager;struct zwp_text_input_v3 *input;
 struct wl_data_device_manager *data_manager;struct wl_data_device *device;
 HwtOffer offers[4];HwtOffer *selection;HwtSource sources[4];HwtSend sends[4];
 uint32_t serial,commits,since;uint64_t epoch,host_owner,paste_epoch,paste_owner;
 int focused,entered,wants,enabled,bad,preedit,pending_preedit;float rectangle[4];
 char commit[4097];size_t commit_n;
 int paste_fd;char *paste;size_t paste_n;int64_t paste_start;
};
static int64_t hwt_now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
static void hwt_error(HwtSdk*s,const char*m){if(s->to.error)s->to.error(s->to.user,m);}
static int hwt_wants(HwtSdk*s){return s->to.wants&&s->to.wants(s->to.user);}
static uint64_t hwt_host_owner(HwtSdk*s){return s->to.owner?s->to.owner(s->to.user):1;}
static void hwt_paste_close(HwtSdk*s){if(s->paste_fd>=0)close(s->paste_fd);s->paste_fd=-1;free(s->paste);s->paste=NULL;s->paste_n=0;}
static int hwt_commit_state(HwtSdk*s){if(s->commits==INT32_MAX){hwt_error(s,"Text input unavailable");return 0;}zwp_text_input_v3_commit(s->input);s->commits++;return 1;}
static void hwt_clear_pending(HwtSdk*s){s->bad=0;s->commit_n=0;s->pending_preedit=0;}
// Invalidate before changing the protocol state: old field replies and held pipes
// cannot publish even if their physical completion arrives in the next dispatch.
static void hwt_boundary_sdk(HwtSdk*s){
 s->epoch=hwt_epoch(s->epoch);hwt_paste_close(s);hwt_clear_pending(s);s->preedit=0;
 if(s->to.cancel_compose)s->to.cancel_compose(s->to.cancel_user);
 if(s->enabled){zwp_text_input_v3_disable(s->input);hwt_commit_state(s);s->enabled=0;}
}
static int hwt_sync(HwtSdk*s){
 int wants=hwt_wants(s);uint64_t owner=hwt_host_owner(s);float r[4]={0};int rect=s->to.rect&&s->to.rect(s->to.user,r);
 int changed=wants!=s->wants||owner!=s->host_owner||memcmp(r,s->rectangle,sizeof r);
 if(changed)hwt_boundary_sdk(s);s->wants=wants;s->host_owner=owner;memcpy(s->rectangle,r,sizeof r);
 if(s->input&&s->entered&&s->focused&&wants&&owner&&s->epoch&&!s->enabled){
  zwp_text_input_v3_enable(s->input);zwp_text_input_v3_set_content_type(s->input,0,ZWP_TEXT_INPUT_V3_CONTENT_PURPOSE_NORMAL);
  if(rect&&hwt_rectangle(r[0],r[1],r[2],r[3]))zwp_text_input_v3_set_cursor_rectangle(s->input,(int)r[0],(int)r[1],(int)r[2],(int)r[3]);
  if(hwt_commit_state(s)){s->since=s->commits;s->enabled=1;}
 }
 return changed;
}
static void hwt_focus(HwtSdk*s,int focused){if(s->focused!=focused){hwt_boundary_sdk(s);s->focused=focused;}hwt_sync(s);}
static void hwt_enter(void*d,struct zwp_text_input_v3*i,struct wl_surface*surface){(void)i;HwtSdk*s=d;if(surface==s->surface){s->entered=1;hwt_sync(s);}}
static void hwt_leave(void*d,struct zwp_text_input_v3*i,struct wl_surface*surface){(void)i;(void)surface;HwtSdk*s=d;hwt_boundary_sdk(s);s->entered=0;}
static void hwt_preedit(void*d,struct zwp_text_input_v3*i,const char*text,int32_t begin,int32_t end){(void)i;(void)begin;(void)end;HwtSdk*s=d;size_t n=text?strnlen(text,4097):0;if(!hwt_text((coax_string){text?text:"",(int64_t)n},false))s->bad=1;else s->pending_preedit=n!=0;}
static void hwt_string(void*d,struct zwp_text_input_v3*i,const char*text){(void)i;HwtSdk*s=d;size_t n=text?strnlen(text,4097):0;if(!hwt_text((coax_string){text?text:"",(int64_t)n},false)){s->bad=1;return;}if(n)memcpy(s->commit,text,n);s->commit_n=n;}
static void hwt_delete(void*d,struct zwp_text_input_v3*i,uint32_t before,uint32_t after){(void)i;HwtSdk*s=d;if(before||after)s->bad=1;}
static void hwt_done(void*d,struct zwp_text_input_v3*i,uint32_t serial){
 (void)i;HwtSdk*s=d;hwt_sync(s);
 if(s->enabled&&hwt_serial(s->commits,s->since,serial)&&hwt_owner(s->epoch,s->epoch,s->focused,s->wants)){
  if(s->bad)hwt_error(s,"Text input unavailable");else{if(s->commit_n&&s->to.text)s->to.text(s->to.user,s->commit,s->commit_n);s->preedit=s->pending_preedit;}
 }
 hwt_clear_pending(s);
}
static const struct zwp_text_input_v3_listener hwt_input_listener={.enter=hwt_enter,.leave=hwt_leave,.preedit_string=hwt_preedit,.commit_string=hwt_string,.delete_surrounding_text=hwt_delete,.done=hwt_done};
static void hwt_offer_mime(void*d,struct wl_data_offer*o,const char*mime){(void)o;HwtOffer*q=d;if(!strcmp(mime,"text/plain;charset=utf-8"))q->mime=3;else if(!strcmp(mime,"UTF8_STRING")&&q->mime<2)q->mime=2;else if(!strcmp(mime,"text/plain")&&!q->mime)q->mime=1;}
static void hwt_offer_actions(void*d,struct wl_data_offer*o,uint32_t actions){(void)d;(void)o;(void)actions;}
static void hwt_offer_action(void*d,struct wl_data_offer*o,uint32_t action){(void)d;(void)o;(void)action;}
static const struct wl_data_offer_listener hwt_offer_listener={.offer=hwt_offer_mime,.source_actions=hwt_offer_actions,.action=hwt_offer_action};
static void hwt_offer_close(HwtOffer*q){if(q->offer)wl_data_offer_destroy(q->offer);q->offer=NULL;q->mime=0;}
static void hwt_data_offer(void*d,struct wl_data_device*device,struct wl_data_offer*offer){(void)device;HwtSdk*s=d;for(int n=0;n<4;n++)if(!s->offers[n].offer){HwtOffer*q=&s->offers[n];q->host=s;q->offer=offer;wl_data_offer_add_listener(offer,&hwt_offer_listener,q);return;}wl_data_offer_destroy(offer);hwt_error(s,"Clipboard unavailable");}
static void hwt_data_enter(void*d,struct wl_data_device*v,uint32_t serial,struct wl_surface*surface,wl_fixed_t x,wl_fixed_t y,struct wl_data_offer*offer){(void)v;(void)serial;(void)surface;(void)x;(void)y;HwtSdk*s=d;for(int n=0;n<4;n++)if(s->offers[n].offer==offer && &s->offers[n]!=s->selection)hwt_offer_close(&s->offers[n]);}
static void hwt_data_leave(void*d,struct wl_data_device*v){(void)d;(void)v;}
static void hwt_data_motion(void*d,struct wl_data_device*v,uint32_t time,wl_fixed_t x,wl_fixed_t y){(void)d;(void)v;(void)time;(void)x;(void)y;}
static void hwt_data_drop(void*d,struct wl_data_device*v){(void)d;(void)v;}
static void hwt_selection(void*d,struct wl_data_device*v,struct wl_data_offer*offer){(void)v;HwtSdk*s=d;HwtOffer*next=NULL;for(int n=0;n<4;n++)if(s->offers[n].offer==offer&&offer)next=&s->offers[n];if(s->selection&&s->selection!=next)hwt_offer_close(s->selection);s->selection=next;}
static const struct wl_data_device_listener hwt_device_listener={.data_offer=hwt_data_offer,.enter=hwt_data_enter,.leave=hwt_data_leave,.motion=hwt_data_motion,.drop=hwt_data_drop,.selection=hwt_selection};
static void hwt_source_release(HwtSource*q){if(q->current||q->refs)return;if(q->source)wl_data_source_destroy(q->source);free(q->bytes);memset(q,0,sizeof *q);}
static void hwt_send_close(HwtSend*j){if(j->source){close(j->fd);HwtSource*q=j->source;j->source=NULL;q->refs--;hwt_source_release(q);}}
static void hwt_source_target(void*d,struct wl_data_source*source,const char*mime){(void)d;(void)source;(void)mime;}
static void hwt_source_send(void*d,struct wl_data_source*source,const char*mime,int32_t fd){
 (void)source;HwtSource*q=d;HwtSdk*s=q->host;if(strcmp(mime,"text/plain;charset=utf-8")&&strcmp(mime,"text/plain")&&strcmp(mime,"UTF8_STRING")){close(fd);return;}
 for(int n=0;n<4;n++)if(!s->sends[n].source){int flags=fcntl(fd,F_GETFL);if(flags<0||fcntl(fd,F_SETFL,flags|O_NONBLOCK)<0){close(fd);hwt_error(s,"Clipboard unavailable");return;}q->refs++;s->sends[n]=(HwtSend){.fd=fd,.start=hwt_now(),.source=q};return;}
 close(fd);hwt_error(s,"Clipboard unavailable");
}
static void hwt_source_cancel(void*d,struct wl_data_source*source){(void)source;HwtSource*q=d;q->current=0;hwt_source_release(q);}
static const struct wl_data_source_listener hwt_source_listener={.target=hwt_source_target,.send=hwt_source_send,.cancelled=hwt_source_cancel};
static void hwt_open_sdk(HwtSdk*s,HwtTo to,struct wl_surface*surface,struct wl_seat*seat){
 s->to=to;s->surface=surface;s->seat=seat;s->epoch=1;s->paste_fd=-1;
 if(s->manager&&seat){s->input=zwp_text_input_manager_v3_get_text_input(s->manager,seat);zwp_text_input_v3_add_listener(s->input,&hwt_input_listener,s);}
 if(s->data_manager&&seat){s->device=wl_data_device_manager_get_data_device(s->data_manager,seat);wl_data_device_add_listener(s->device,&hwt_device_listener,s);}
}
static int hwt_copy(HwtSdk*s,const char*bytes,size_t length){
 if(!s->device||!s->serial||!s->focused||!hwt_count((int64_t)length,true)||!hwt_text((coax_string){bytes,(int64_t)length},true))return 0;
 int cells=0;size_t total=0;HwtSource*q=NULL;for(int n=0;n<4;n++){if(s->sources[n].bytes){cells++;total+=s->sources[n].length;}else q=&s->sources[n];}
 if(!q||!hwt_lease(cells,(int64_t)total,(int64_t)length))return 0;
 char*copy=malloc(length?length:1);if(!copy)return 0;if(length)memcpy(copy,bytes,length);
 struct wl_data_source*source=wl_data_device_manager_create_data_source(s->data_manager);if(!source){free(copy);return 0;}
 *q=(HwtSource){.host=s,.source=source,.bytes=copy,.length=length,.current=1};wl_data_source_add_listener(source,&hwt_source_listener,q);
 wl_data_source_offer(source,"text/plain;charset=utf-8");wl_data_source_offer(source,"text/plain");wl_data_source_offer(source,"UTF8_STRING");wl_data_device_set_selection(s->device,source,s->serial);return 1;
}
static int hwt_paste_start(HwtSdk*s){
 hwt_sync(s);if(!s->device||!s->selection||!s->selection->mime||!hwt_owner(s->epoch,s->epoch,s->focused,s->wants))return 0;
 hwt_boundary_sdk(s);hwt_sync(s);
 hwt_paste_close(s);int pair[2];if(pipe(pair))return 0;int flags=fcntl(pair[0],F_GETFL);if(flags<0||fcntl(pair[0],F_SETFL,flags|O_NONBLOCK)<0||fcntl(pair[0],F_SETFD,FD_CLOEXEC)<0||fcntl(pair[1],F_SETFD,FD_CLOEXEC)<0){close(pair[0]);close(pair[1]);return 0;}
 s->paste=malloc(2097152);if(!s->paste){close(pair[0]);close(pair[1]);return 0;}s->paste_fd=pair[0];s->paste_epoch=s->epoch;s->paste_owner=s->host_owner;s->paste_start=hwt_now();
 const char*m=s->selection->mime==3?"text/plain;charset=utf-8":s->selection->mime==2?"UTF8_STRING":"text/plain";wl_data_offer_receive(s->selection->offer,m,pair[1]);close(pair[1]);return 1;
}
static void hwt_poll(HwtSdk*s){
 hwt_sync(s);int64_t now=hwt_now();
 if(s->paste_fd>=0){
  if(!hwt_owner(s->paste_epoch,s->epoch,s->focused,s->wants)||s->paste_owner!=s->host_owner){hwt_paste_close(s);}
  else if(!hwt_time(s->paste_start,now)){hwt_paste_close(s);hwt_error(s,"Clipboard unavailable");}
  else{char bytes[32768];ssize_t n=read(s->paste_fd,bytes,sizeof bytes);if(n>0){if(!hwt_chunk((int64_t)s->paste_n,n)){hwt_paste_close(s);hwt_error(s,"Clipboard unavailable");}else{memcpy(s->paste+s->paste_n,bytes,(size_t)n);s->paste_n+=(size_t)n;}}
   else if(n==0){if(hwt_text((coax_string){s->paste,(int64_t)s->paste_n},true)){if(s->paste_n&&s->to.text)s->to.text(s->to.user,s->paste,s->paste_n);}else hwt_error(s,"Clipboard unavailable");hwt_paste_close(s);}
   else if(errno!=EAGAIN&&errno!=EINTR){hwt_paste_close(s);hwt_error(s,"Clipboard unavailable");}
  }
 }
 for(int i=0;i<4;i++){HwtSend*j=&s->sends[i];if(!j->source)continue;HwtSource*q=j->source;
  if(!hwt_time(j->start,now)){hwt_send_close(j);hwt_error(s,"Clipboard unavailable");continue;}
  size_t left=q->length-j->sent;if(!left){hwt_send_close(j);continue;}if(left>32768)left=32768;
  // The pipe may have lost its reader; consume only this thread's SIGPIPE.
  sigset_t set,prior,pending;sigemptyset(&set);sigaddset(&set,SIGPIPE);pthread_sigmask(SIG_BLOCK,&set,&prior);sigpending(&pending);
  ssize_t n=write(j->fd,q->bytes+j->sent,left);int saved=errno;
  if(n<0&&saved==EPIPE&&!sigismember(&pending,SIGPIPE)){struct timespec zero={0};sigtimedwait(&set,NULL,&zero);}pthread_sigmask(SIG_SETMASK,&prior,NULL);
  if(n>0){j->sent+=(size_t)n;if(j->sent==q->length)hwt_send_close(j);}else if(n<0&&saved!=EAGAIN&&saved!=EINTR){hwt_send_close(j);hwt_error(s,"Clipboard unavailable");}
 }
}
static void hwt_close_sdk(HwtSdk*s){hwt_paste_close(s);for(int n=0;n<4;n++)hwt_send_close(&s->sends[n]);for(int n=0;n<4;n++){s->sources[n].current=0;hwt_source_release(&s->sources[n]);hwt_offer_close(&s->offers[n]);}if(s->input)zwp_text_input_v3_destroy(s->input);if(s->device){if(wl_proxy_get_version((struct wl_proxy*)s->device)>=2)wl_data_device_release(s->device);else wl_data_device_destroy(s->device);}if(s->manager)zwp_text_input_manager_v3_destroy(s->manager);if(s->data_manager)wl_data_device_manager_destroy(s->data_manager);memset(s,0,sizeof *s);s->paste_fd=-1;}
#endif
