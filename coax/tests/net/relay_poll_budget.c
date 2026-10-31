#include "budget.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int closes, reads, deliver;
int64_t cxsock_close(int64_t h){(void)h;closes++;return 0;}
int64_t cxtls_close(int64_t h){return cxsock_close(h);}
int64_t cxsock_read(int64_t h,int64_t wait){(void)h;(void)wait;reads++;if(deliver){deliver=0;return 3;}return -2;}
int64_t cxtls_read(int64_t h,int64_t wait){return cxsock_read(h,wait);}
int64_t cxsock_byte(int64_t h,int64_t i){(void)h;return i==0?0x81:i==1?1:'x';}
int64_t cxtls_byte(int64_t h,int64_t i){return cxsock_byte(h,i);}
int64_t cxsock_write(int64_t h,int64_t b){(void)h;(void)b;return 0;}
int64_t cxtls_write(int64_t h,int64_t b){return cxsock_write(h,b);}
int64_t cxsock_flush(int64_t h,int64_t wait){(void)h;(void)wait;return 0;}
int64_t cxtls_flush(int64_t h,int64_t wait){return cxsock_flush(h,wait);}
int64_t cxsock_host_push(int64_t b){(void)b;return 0;}
int64_t cxtls_host_push(int64_t b){return cxsock_host_push(b);}
int64_t cxsock_connect(int64_t p,int64_t wait){(void)p;(void)wait;return -1;}
int64_t cxtls_connect(int64_t p,int64_t wait){return cxsock_connect(p,wait);}
int64_t cxsock_now_ms(void){return 0;}
int64_t cxrand_fill(int64_t n){(void)n;return 0;}
int64_t cxrand_byte(int64_t i){(void)i;return 0;}

static coax_t_RelayConn state;
static coax_v_t_RelayConn reset(void){memset(&state,0,sizeof state);state.h=4;closes=reads=deliver=0;return (coax_v_t_RelayConn){&state,1};}
static void append(int a,int b){state.inb.e[state.used++]=(uint8_t)a;state.inb.e[state.used++]=(uint8_t)b;}
static void text(void){append(0x81,1);state.inb.e[state.used++]='x';}
int main(void){
 coax_v_t_RelayConn c=reset();for(int i=0;i<128;i++)append(0x8a,0);text();
 assert(budget_poll(c,0,64)==3&&state.h==-1&&state.pongs==64&&closes==1&&reads==0);
 c=reset();append(1,0);for(int i=1;i<128;i++)append(0,0);append(0x80,1);state.inb.e[state.used++]='x';
 assert(budget_poll(c,0,64)==3&&state.h==-1&&closes==1&&reads==0);
 c=reset();text();assert(budget_poll(c,0,1)==1&&state.text_n==1&&state.text.e[0]=='x'&&state.h==4);
 c=reset();append(0x8a,0);append(0x8a,0);text();assert(budget_poll(c,0,3)==1&&state.pongs==2);
 c=reset();deliver=1;assert(budget_poll(c,0,2)==1&&reads==1&&state.text.e[0]=='x');
 c=reset();assert(budget_poll(c,0,1)==0&&reads==1&&state.h==4);
 const int64_t invalid[]={INT64_MIN,-1,0,65537,INT64_MAX};
 for(unsigned i=0;i<sizeof invalid/sizeof invalid[0];i++){c=reset();text();assert(budget_poll(c,0,invalid[i])==3&&closes==1&&reads==0);}
 c=reset();for(int i=0;i<128;i++)append(0x8a,0);text();assert(legacy_poll(c,0)==1&&state.pongs==128&&state.text.e[0]=='x');
 c=reset();append(1,0);for(int i=1;i<127;i++)append(0,0);append(0x80,1);state.inb.e[state.used++]='x';
 assert(budget_poll(c,0,128)==1&&state.text_n==1&&state.text.e[0]=='x');
 c=reset();state.inb.e[0]=0x81;state.used=1;assert(budget_poll(c,0,1)==0&&state.used==1&&state.h==4);
 c=reset();append(1,0);assert(budget_poll(c,0,2)==0&&state.rd.e[0].open&&state.h==4);
 puts("relay poll budget: controls, empty fragments, exact work, reads, invalid bounds and legacy behavior passed");
}
