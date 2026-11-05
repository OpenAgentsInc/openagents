// Firm reason: 6 (observe actual SDK allocation/read/copy effects for Coax).
#include "ImageCacheCheck.h"
#include "render-images.h"
#include <string.h>
static RhgiImages *cache;
static RhgiImage last;
static coax_v_u8 source;
static int64_t mode,sizes,reads,closed_bytes;
static uint8_t uri[65536];
static int64_t observed_size(void *user,coax_string url){(void)user;(void)url;sizes++;return mode==1?-1:mode==3?8388609:source.count;}
static int observed_read(void *user,coax_string url,uint8_t *out,size_t bytes){(void)user;(void)url;reads++;if(mode==4)return -1;if(mode==2||(uint64_t)source.count!=bytes)return 0;if(bytes)memcpy(out,source.data,bytes);return 1;}
int64_t icn_open(void){cache=rhgi_open();last=(RhgiImage){0};source=(coax_v_u8){0};mode=sizes=reads=closed_bytes=0;return cache!=NULL;}
void icn_source(coax_v_u8 bytes,int64_t fault){source=bytes;mode=fault;}
void icn_get(coax_v_u8 url){if(url.count<0||(uint64_t)url.count>sizeof uri){last=(RhgiImage){.slot=-1,.failed=1};return;}if(url.count)memcpy(uri,url.data,(size_t)url.count);last=rhgi_get(cache,1,(coax_string){(char*)uri,url.count},(RhgiSource){observed_size,observed_read,NULL});}
int64_t icn_info(int64_t field){switch(field){case 0:return last.slot;case 1:return last.decoded;case 2:return last.failed;case 3:return last.width;case 4:return last.height;case 5:return(int64_t)last.bytes;case 6:return(int64_t)rhgi_count(cache);case 7:return cache?(int64_t)rhgi_bytes(cache):closed_bytes;case 8:return sizes;case 9:return reads;}return -1;}
int64_t icn_copy(coax_v_u8 out){if(out.count<0||(uint64_t)out.count<last.bytes||(!out.data&&last.bytes))return -1;if(last.bytes)memcpy(out.data,last.pixels,last.bytes);return(int64_t)last.bytes;}
int64_t icn_notice(coax_v_u8 out){coax_t_HostMediaNotice row={.url=last.url,.ready=last.decoded,.failed=last.failed,.width=last.width,.height=last.height};return rhgi_notice(&row,1,out.data,(size_t)out.count);}
void icn_close(void){closed_bytes=rhgi_close(cache);cache=NULL;last=(RhgiImage){0};source=(coax_v_u8){0};}
