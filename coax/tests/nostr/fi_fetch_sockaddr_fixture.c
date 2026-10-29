/* Compiled test-only into the same translation unit to exercise bounds before
   reading family-specific sockaddr storage. */
#include "fixtures/fi_transport/fi_fetch.c"
#include <assert.h>
int main(void) {
 const char *url="https://example.com/";assert(fi_fetch_start(url,strlen(url),1,100)==NULL);
 struct sockaddr_in a={0},b={0};a.sin_family=AF_INET;b.sin_family=AF_INET;
 a.sin_port=b.sin_port=htons(443);inet_pton(AF_INET,"8.8.8.8",&a.sin_addr);b.sin_addr=a.sin_addr;
 assert(fi_same_socket((struct sockaddr *)&a,sizeof(a),(struct sockaddr *)&b,sizeof(b)));
 assert(!fi_same_socket((struct sockaddr *)&a,0,(struct sockaddr *)&b,sizeof(b)));
 assert(!fi_same_socket((struct sockaddr *)&a,sizeof(a),(struct sockaddr *)&b,0));
 assert(!fi_same_socket((struct sockaddr *)&a,sizeof(a)-1,(struct sockaddr *)&b,sizeof(b)));
 assert(!fi_same_socket(NULL,sizeof(a),(struct sockaddr *)&b,sizeof(b)));
 FiNativeFetch f={0};atomic_init(&f.cancelled,0);f.deadline_ns=fi_clock()+1000000000LL;memcpy(&f.pinned,&a,sizeof(a));f.pinned_length=sizeof(a);
 struct curl_sockaddr address={0};address.family=AF_INET;address.socktype=SOCK_STREAM;address.protocol=IPPROTO_TCP;address.addrlen=0;
 assert(fi_socket(&f,CURLSOCKTYPE_IPCXN,&address)==CURL_SOCKET_BAD);
 return 0;
}
