// Scalar ABI projections for aethyr/host/transport.coax. The including face
// supplies its generated header. C stores typed results; Coax decides every
// slot, phase, owner, byte limit and terminal outcome.
#ifndef HOST_TRANSPORT_HEADER
#define HOST_TRANSPORT_HEADER "Render.h"
#endif
#include HOST_TRANSPORT_HEADER
#include <stdint.h>
#ifdef __wasm__
#define HOST_EXPORT(name) __attribute__((export_name(name)))
#else
#define HOST_EXPORT(name)
#endif
static coax_t_HostTransport host_transport;
static coax_t_HostOwner host_owner;
static coax_t_HostDelivery host_delivery;
static uint8_t host_route[4096];
static uint8_t host_target[12288];
static coax_v_t_HostTransport host_transport_view(void) { return (coax_v_t_HostTransport){&host_transport,1}; }
static coax_t_HostOwner host_owned(int32_t id,int64_t generation,int64_t engine) { return (coax_t_HostOwner){id,generation,engine}; }
__attribute__((constructor)) static void host_transport_defaults(void) { coax_default_t_HostTransport(&host_transport); }
HOST_EXPORT("coaxial_host_transport_version") uint32_t coaxial_host_transport_version(void) { return 1; }
HOST_EXPORT("coaxial_host_transport_reset") uint32_t coaxial_host_transport_reset(void) { return ht_reset(host_transport_view()); }
HOST_EXPORT("coaxial_host_owner") int64_t coaxial_host_owner(int32_t field) { switch(field){case 0:return host_owner.id;case 1:return host_owner.generation;case 2:return host_owner.engine;default:return -1;} }
HOST_EXPORT("coaxial_host_delivery") int64_t coaxial_host_delivery(int32_t field) { switch(field){case 0:return host_delivery.kind;case 1:return host_delivery.bytes;case 2:return host_delivery.failure;default:return -1;} }
HOST_EXPORT("coaxial_host_socket_reserve") int32_t coaxial_host_socket_reserve(void) { host_owner=ht_socket_reserve(host_transport_view());return (int32_t)host_owner.id; }
HOST_EXPORT("coaxial_host_socket_send") uint32_t coaxial_host_socket_send(int32_t id,int64_t generation,int64_t engine) { return ht_socket_send(host_transport_view(),host_owned(id,generation,engine)); }
HOST_EXPORT("coaxial_host_socket_close") uint32_t coaxial_host_socket_close(int32_t id,int64_t generation,int64_t engine) { return ht_socket_close(host_transport_view(),host_owned(id,generation,engine)); }
HOST_EXPORT("coaxial_host_socket_event") uint32_t coaxial_host_socket_event(int32_t id,int64_t generation,int64_t engine,int32_t event,int64_t bytes,int64_t capacity) { host_delivery=ht_socket_deliver(host_transport_view(),host_owned(id,generation,engine),event,bytes,capacity);return host_delivery.deliver; }
HOST_EXPORT("coaxial_host_http_reserve") int32_t coaxial_host_http_reserve(int32_t flags,int32_t bounded,int64_t timeout,int64_t now,int64_t capacity) { host_owner=ht_http_reserve(host_transport_view(),flags,bounded!=0,timeout,now,capacity);return (int32_t)host_owner.id; }
HOST_EXPORT("coaxial_host_http_chunk") uint32_t coaxial_host_http_chunk(int32_t id,int64_t generation,int64_t engine,int64_t bytes) { return ht_http_chunk(host_transport_view(),host_owned(id,generation,engine),bytes); }
HOST_EXPORT("coaxial_host_http_finish") uint32_t coaxial_host_http_finish(int32_t id,int64_t generation,int64_t engine,int64_t now,int32_t ok,int32_t utf8) { host_delivery=ht_http_finish(host_transport_view(),host_owned(id,generation,engine),now,ok!=0,utf8!=0);return host_delivery.deliver; }
HOST_EXPORT("coaxial_host_http_timeout") uint32_t coaxial_host_http_timeout(int32_t id,int64_t generation,int64_t engine,int64_t now) { host_delivery=ht_http_timeout(host_transport_view(),host_owned(id,generation,engine),now);return host_delivery.deliver; }
HOST_EXPORT("coaxial_host_http_cancel") uint32_t coaxial_host_http_cancel(int32_t id) { return ht_http_cancel(host_transport_view(),id); }
HOST_EXPORT("coaxial_host_route_storage") uint32_t coaxial_host_route_storage(void) { return (uint32_t)(uintptr_t)host_route; }
HOST_EXPORT("coaxial_host_route_capacity") uint32_t coaxial_host_route_capacity(void) { return sizeof host_route; }
HOST_EXPORT("coaxial_host_http_path") uint32_t coaxial_host_http_path(uint32_t n) { return n<=sizeof host_route && htr_http_path((coax_string){(const char *)host_route,n}); }
HOST_EXPORT("coaxial_host_http_target") int32_t coaxial_host_http_target(uint32_t n) { return n<=sizeof host_route ? (int32_t)htr_http_target((coax_string){(const char *)host_route,n},(coax_v_u8){host_target,sizeof host_target}) : -1; }
HOST_EXPORT("coaxial_host_target_storage") uint32_t coaxial_host_target_storage(void) { return (uint32_t)(uintptr_t)host_target; }
HOST_EXPORT("coaxial_host_local_uri") uint32_t coaxial_host_local_uri(uint32_t n) { return n<=sizeof host_route && htr_local_uri((coax_string){(const char *)host_route,n}); }
HOST_EXPORT("coaxial_host_local_origin") uint32_t coaxial_host_local_origin(uint32_t n) { return n<=sizeof host_route && htr_local_origin((coax_string){(const char *)host_route,n}); }
HOST_EXPORT("coaxial_host_copy_span") uint32_t coaxial_host_copy_span(uint64_t address,uint64_t capacity,uint64_t bytes,uint64_t memory_bytes) { return htb_copy_span(address,capacity,bytes,memory_bytes); }
