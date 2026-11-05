// Firm reason: 1 (scalar SDK projections of the shared Coax lifecycle/frame-clock authority).
// The including host transport face supplies HOST_TRANSPORT_HEADER.
#include HOST_TRANSPORT_HEADER
#include <stdint.h>
#ifdef __wasm__
#define HOST_LIFE_EXPORT(name) __attribute__((export_name(name)))
#else
#define HOST_LIFE_EXPORT(name)
#endif
static coax_t_HostLifecycle host_lifecycle;
static coax_t_HostFrameOffer host_frame_offer;
static coax_t_LifeActions host_life_actions;
static coax_v_t_HostLifecycle host_lifecycle_view(void){return (coax_v_t_HostLifecycle){&host_lifecycle,1};}
__attribute__((constructor)) static void host_lifecycle_defaults(void){coax_default_t_HostLifecycle(&host_lifecycle);}
HOST_LIFE_EXPORT("coaxial_host_lifecycle_version") uint32_t coaxial_host_lifecycle_version(void){return 1;}
HOST_LIFE_EXPORT("coaxial_host_lifecycle_reset") void coaxial_host_lifecycle_reset(void){coax_default_t_HostLifecycle(&host_lifecycle);}
HOST_LIFE_EXPORT("coaxial_host_lifecycle_event") int64_t coaxial_host_lifecycle_event(int32_t kind,int64_t now_us,int64_t arg){host_life_actions=htl_event_us(host_lifecycle_view(),kind,now_us,arg);return host_life_actions.does;}
HOST_LIFE_EXPORT("coaxial_host_frame_offer") uint32_t coaxial_host_frame_offer(int64_t now_us,int64_t pending){host_frame_offer=htl_offer(host_lifecycle_view(),now_us,pending);return host_frame_offer.admit;}
HOST_LIFE_EXPORT("coaxial_host_frame_dt") double coaxial_host_frame_dt(void){return host_frame_offer.dt;}
HOST_LIFE_EXPORT("coaxial_host_lifecycle_info") int64_t coaxial_host_lifecycle_info(int32_t field){switch(field){case 0:return host_lifecycle.focused;case 1:return host_lifecycle.shown;case 2:return host_lifecycle.profile.e[0].active;case 3:return host_lifecycle.profile.e[0].drawing;case 4:return host_lifecycle.last_us;default:return -1;}}
