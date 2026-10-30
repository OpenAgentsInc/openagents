// Native socket/argv effects only. No production key, relay or inference.
#define main fleet_effect_main
#include "fleet-host.c"
#undef main
#include <assert.h>
int coax_fleet_main(void) { return 0; }
static void address(uint32_t expected) {
  int64_t listener = fh_listen(0); assert(listener >= 0);
  struct sockaddr_in actual; socklen_t n = sizeof actual;
  assert(getsockname(fh_slots[listener].fd, (struct sockaddr *)&actual, &n) == 0);
  assert(ntohl(actual.sin_addr.s_addr) == expected); assert(fh_close(listener) == 0);
}
int main(void) {
  fh_init(); address(INADDR_LOOPBACK);
  char *local[] = {"fleet", "--identity", "m", "--listen-host", "127.0.0.1", "--port", "8080", NULL};
  assert(fh_platform_args(7, local) == 5); assert(!strcmp(local[3], "--port")); address(INADDR_LOOPBACK);
  char *all[] = {"fleet", "--listen-host", "0.0.0.0", "--identity", "m", NULL};
  assert(fh_platform_args(5, all) == 3); assert(!strcmp(all[1], "--identity")); address(INADDR_ANY);
  char *invalid[] = {"fleet", "--listen-host", "8.8.8.8", NULL}; assert(fh_platform_args(3, invalid) < 0);
  char *missing[] = {"fleet", "--listen-host", NULL}; assert(fh_platform_args(2, missing) < 0);
  char *duplicate[] = {"fleet", "--listen-host", "0.0.0.0", "--listen-host", "127.0.0.1", NULL}; assert(fh_platform_args(5, duplicate) < 0);
  puts("fleet host: loopback/default, explicit external bind and argv checks passed");
  return 0;
}
