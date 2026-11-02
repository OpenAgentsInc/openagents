// Test-only seed bridge. Every lifecycle/input/frame/media export above it
// is the actual production Villa face, included without modification.
#include "../../../apps/openagents-web/villa-face.c"
EXPORT("coaxial_render_fixture") void coaxial_render_fixture(uint32_t state) {
  start(state);render_fixture_seed(at(state));
}
EXPORT("coaxial_render_fixture_video") void coaxial_render_fixture_video(uint32_t state) {
  start(state);render_fixture_video_seed(at(state));
}
