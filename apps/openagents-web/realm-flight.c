// The Haunted House's fly-over (verse/cinematic.coax): /realm/haunted-house#flyover
// flies round the house, cuts inside to the parlour's candles and back out
// with a flash of lightning behind the cut, and hands the view back to your
// camera. Any key or a tap skips it. Included by realm-face.c.
#include "Cinematic.h"

static coax_t_Flight flight;
static bool flight_wanted;
static int64_t flight_take_was = -1;

static coax_t_Point fp(float x, float y, float z) { return (coax_t_Point){x, y, z}; }

// The hash asks for it: true when the hash was the fly-over's.
static bool flight_hash(const uint8_t *hash, uint32_t length) {
  if (length != 7 || memcmp(hash, "flyover", 7)) return false;
  flight_wanted = true;
  return true;
}

static void flight_begin(bool reduced) {
  coax_t_Flight f = verse_flight_new();
  f = verse_flight_key(f, 0, fp(7, 26, -16), fp(8, 2, 30), false, 0);
  f = verse_flight_key(f, 4.5f, fp(33, 21, 6), fp(8, 2, 32), false, 1);
  f = verse_flight_key(f, 8.5f, fp(31, 17, 62), fp(8, 2, 36), false, 1);
  f = verse_flight_key(f, 9.5f, fp(15.2f, 1.75f, 28.6f), fp(18, 1.1f, 31.4f), true, 0);
  f = verse_flight_key(f, 13, fp(16.6f, 1.45f, 33.6f), fp(18, 1.3f, 34.4f), false, 3);
  f = verse_flight_key(f, 13.6f, fp(7, 10, -7), fp(7, 1.5f, 12), true, 0);
  f = verse_flight_key(f, 17, fp(7, 3.2f, -0.5f), fp(7, 1.6f, 10), false, 1);
  flight = verse_flight_start(f, now, reduced);
  flight_take_was = -1;
  if (!reduced) toast("A key or a tap skips the flight");
}

// The view this frame: the flight's while it has the camera.
static bool flight_view(coax_t_Vec3 *eye, coax_t_Vec3 *at, bool reduced) {
  if (flight_wanted) { flight_wanted = false; flight_begin(reduced); }
  if (!verse_flight_active(flight, now)) return false;
  int64_t take = verse_flight_take(flight, now);
  if (take != flight_take_was && take > 0) house.flash_until = now + 0.2;
  flight_take_was = take;
  coax_t_Shot live = {fp(eye->x, eye->y, eye->z), fp(at->x, at->y, at->z)};
  coax_t_Shot s = verse_flight_shot(flight, now, live);
  *eye = (coax_t_Vec3){s.eye.x, s.eye.y, s.eye.z};
  *at = (coax_t_Vec3){s.at.x, s.at.y, s.at.z};
  return true;
}

// A key or a touch while it flies: it hands back now. True when it was the flight's.
static bool flight_skip(void) {
  if (!verse_flight_flying(flight, now)) return false;
  flight = verse_flight_skip(flight, now);
  house.toast_until = now;
  return true;
}
