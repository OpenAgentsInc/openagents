// Firm reason: 6 (actual Wayland client protocol objects and owned clipboard descriptors exercise the production SDK adapter without a compositor or window).
#define _GNU_SOURCE
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/socket.h>
#include <wayland-client.h>
#include "wayland-text-sdk.h"
static int wanted=1,texts,errors,rows;static size_t text_bytes;static uint64_t owner=1,hash;
static void row(int kind,int64_t a,int64_t b,int64_t c){printf("%s[%d,%lld,%lld,%lld]",rows++?",":"",kind,(long long)a,(long long)b,(long long)c);}
static void text_cb(void*d,const char*b,size_t n){(void)d;texts++;text_bytes=n;hash=0;for(size_t i=0;i<n;i++)hash=(hash*257+(unsigned char)b[i])%1000000007;}
static int wants_cb(void*d){(void)d;return wanted;}
static uint64_t owner_cb(void*d){(void)d;return owner;}
static int rect_cb(void*d,float r[4]){(void)d;r[0]=20;r[1]=30;r[2]=200;r[3]=24;return 1;}
static void error_cb(void*d,const char*m){(void)d;(void)m;errors++;}
static void reset(void){texts=errors=0;text_bytes=0;hash=0;}
static int request_fd(struct wl_display*display,int peer){
 if(wl_display_flush(display)<0)return -1;unsigned char bytes[65536],control[CMSG_SPACE(8*sizeof(int))];struct iovec io={bytes,sizeof bytes};struct msghdr m={.msg_iov=&io,.msg_iovlen=1,.msg_control=control,.msg_controllen=sizeof control};
 ssize_t n=recvmsg(peer,&m,MSG_DONTWAIT);if(n<0)return -1;int result=-1;for(struct cmsghdr*c=CMSG_FIRSTHDR(&m);c;c=CMSG_NXTHDR(&m,c))if(c->cmsg_level==SOL_SOCKET&&c->cmsg_type==SCM_RIGHTS){size_t count=(c->cmsg_len-CMSG_LEN(0))/sizeof(int);int *fds=(int*)CMSG_DATA(c);for(size_t i=0;i<count;i++){if(result<0)result=fds[i];else close(fds[i]);}}return result;
}
static int paste(HwtSdk*s,struct wl_display*display,int peer){if(!hwt_paste_start(s))return -1;return request_fd(display,peer);}
static void finish_paste(HwtSdk*s,int fd,const char*bytes,size_t length){if(length&&write(fd,bytes,length)!=(ssize_t)length)exit(3);close(fd);hwt_poll(s);hwt_poll(s);}
static size_t source_bytes(HwtSdk*s){size_t n=0;for(int i=0;i<4;i++)if(s->sources[i].bytes)n+=s->sources[i].length;return n;}
int main(void){
 int pair[2];if(socketpair(AF_UNIX,SOCK_STREAM,0,pair))return 2;struct wl_display*display=wl_display_connect_to_fd(pair[0]);if(!display)return 2;
 struct wl_registry*registry=wl_display_get_registry(display);struct wl_compositor*compositor=wl_registry_bind(registry,1,&wl_compositor_interface,4);struct wl_surface*surface=wl_compositor_create_surface(compositor);struct wl_seat*seat=wl_registry_bind(registry,2,&wl_seat_interface,8);
 HwtSdk s={0};s.manager=wl_registry_bind(registry,3,&zwp_text_input_manager_v3_interface,1);s.data_manager=wl_registry_bind(registry,4,&wl_data_device_manager_interface,3);
 hwt_open_sdk(&s,(HwtTo){.text=text_cb,.wants=wants_cb,.rect=rect_cb,.owner=owner_cb,.error=error_cb},surface,seat);hwt_focus(&s,1);hwt_enter(&s,s.input,surface);s.serial=42;
 printf("{\"schema\":\"aethyr.host.wayland-text.v1\",\"rows\":[");row(1,s.enabled,s.commits,s.since);
 // Preedit is buffered until done and never published as intermediate keys/text.
 hwt_preedit(&s,s.input,"候補",0,6);hwt_done(&s,s.input,s.commits);row(2,texts,s.preedit,errors);
 hwt_string(&s,s.input,"確定🙂");hwt_preedit(&s,s.input,"",0,0);hwt_done(&s,s.input,s.commits);row(3,texts,text_bytes,(int64_t)hash);
 // An older same-field done remains valid after a client cursor/state commit.
 reset();uint32_t old=s.commits;hwt_commit_state(&s);hwt_string(&s,s.input,"é");hwt_done(&s,s.input,old);row(4,texts,text_bytes,errors);
 reset();hwt_boundary_sdk(&s);hwt_sync(&s);hwt_string(&s,s.input,"stale");hwt_done(&s,s.input,old);row(5,texts,errors,s.enabled);
 hwt_string(&s,s.input,"prefix");hwt_delete(&s,s.input,1,0);hwt_done(&s,s.input,s.commits);row(6,texts,errors,0);
 reset();hwt_string(&s,s.input,"\xc3(");hwt_done(&s,s.input,s.commits);row(7,texts,errors,0);
 reset();char big[4098];memset(big,'a',4097);big[4097]=0;hwt_string(&s,s.input,big);hwt_done(&s,s.input,s.commits);row(8,texts,errors,0);
 reset();hwt_string(&s,s.input,"orphan");owner++;hwt_done(&s,s.input,s.commits);row(9,texts,errors,s.enabled);
 struct wl_data_offer*offer=(struct wl_data_offer*)wl_proxy_create((struct wl_proxy*)s.device,&wl_data_offer_interface);hwt_data_offer(&s,s.device,offer);HwtOffer*q=NULL;for(int i=0;i<4;i++)if(s.offers[i].offer==offer)q=&s.offers[i];if(!q)return 2;hwt_offer_mime(q,offer,"text/plain");hwt_offer_mime(q,offer,"UTF8_STRING");hwt_offer_mime(q,offer,"text/plain;charset=utf-8");hwt_selection(&s,s.device,offer);row(10,q->mime,0,0);
 reset();int fd=paste(&s,display,pair[1]);if(fd<0)return 2;hwt_poll(&s);row(11,texts,s.paste!=NULL,errors);finish_paste(&s,fd,"café🙂",9);row(12,texts,text_bytes,(int64_t)hash);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;finish_paste(&s,fd,"a\xc3(",3);row(13,texts,errors,s.paste==NULL);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;finish_paste(&s,fd,"a\0b",3);row(14,texts,errors,s.paste==NULL);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;owner++;hwt_poll(&s);close(fd);row(15,texts,errors,s.paste==NULL);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;wanted=0;hwt_poll(&s);close(fd);row(16,texts,s.enabled,s.paste==NULL);wanted=1;hwt_sync(&s);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;s.paste_start=hwt_now()-5000;hwt_poll(&s);close(fd);row(17,texts,errors,s.paste==NULL);
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;hwt_focus(&s,0);close(fd);row(18,texts,s.enabled,s.paste==NULL);hwt_focus(&s,1);
 // The full 2 MiB is physically received, then refused as one transaction when
 // the next byte arrives. No prefix is emitted and the owned FD is closed.
 reset();fd=paste(&s,display,pair[1]);if(fd<0)return 2;char chunk[4096];memset(chunk,'x',sizeof chunk);for(int i=0;i<512;i++){if(write(fd,chunk,sizeof chunk)!=sizeof chunk)return 2;hwt_poll(&s);}row(19,texts,s.paste_n,errors);if(write(fd,"x",1)!=1)return 2;hwt_poll(&s);close(fd);row(20,texts,errors,s.paste==NULL);
 // A cancelled source must retain bytes until its actual nonblocking send
 // completes; replacement does not change the in-flight source's contents.
 reset();if(!hwt_copy(&s,"old🙂",7))return 2;HwtSource*first=NULL;for(int i=0;i<4;i++)if(s.sources[i].bytes)first=&s.sources[i];int pipes[2];if(pipe(pipes))return 2;hwt_source_send(first,first->source,"text/plain;charset=utf-8",pipes[1]);hwt_source_cancel(first,first->source);row(21,first->refs,source_bytes(&s),first->current);
 if(!hwt_copy(&s,"new",3))return 2;hwt_poll(&s);char result[16]={0};ssize_t received=read(pipes[0],result,sizeof result);close(pipes[0]);row(22,received,received==7&&!memcmp(result,"old🙂",7),source_bytes(&s));
 // A closed receiver cannot terminate the host with SIGPIPE.
 HwtSource*second=NULL;for(int i=0;i<4;i++)if(s.sources[i].bytes)second=&s.sources[i];if(pipe(pipes))return 2;close(pipes[0]);hwt_source_send(second,second->source,"text/plain",pipes[1]);hwt_poll(&s);row(23,errors,second->refs,source_bytes(&s));
 // Source cells are bounded while the compositor has not cancelled them.
 int accepted=0;for(int i=0;i<5;i++)accepted+=hwt_copy(&s,"x",1);row(24,accepted,source_bytes(&s),0);
 hwt_close_sdk(&s);row(25,source_bytes(&s),s.paste==NULL,s.input==NULL);
 wl_surface_destroy(surface);wl_compositor_destroy(compositor);wl_seat_destroy(seat);wl_registry_destroy(registry);wl_display_disconnect(display);close(pair[1]);printf("]}\n");return 0;
}
