// The C side of place.coax for a walkable realm's face: where you are, kept
// so a reload puts you back (eg-pos). It hands the kept text to the host
// (action 11, localStorage, synchronous), reads it back (message 11, after
// the page facts and before the first frame), asks the server for its copy
// once the facts say the player has an account (GET
// /api/playtest/progress/<slot>/place, the player's key) and sends it there
// every PLACE_SERVER_EVERY seconds while you move and when the page is
// hidden or left (a keepalive POST). place.coax decides when and which copy
// wins; this file decides nothing.
//
// The face includes its page's header (which #loads place.coax) and
// analytics-face.h (track_state, the page's clock and the wall clock), then
// defines, before including this file:
//   static coax_t_Place place_here(uint32_t state);         where you are now (ok false: not yet placed)
//   static void place_put(uint32_t state, coax_t_Place p);   put you there, at the nearest open ground
// and calls place_seen(state) once you are first placed, place_frame(state)
// after each step, place_message(state, kind, inbox, length) at the top of
// coaxial_message (true: it was the place's), and lets place_action() take
// the outbox when the page has no action of its own. A place the location's
// hash named (verse_place_named) wins only until you walk off it: the first
// step off it drops the hash too (action 3, empty), so a reload brings
// you back where you were, not to the place the link named.
#pragma once
#include <stdbool.h>
#include <stdint.h>

#define PLACE_ACTION 11
#define PLACE_MESSAGE 11

static coax_t_Place place_here(uint32_t state);
static void place_put(uint32_t state, coax_t_Place p);

static coax_t_PlaceKeeper place_keeper;
static uint8_t place_text_out[400];
static uint32_t place_text_n;
static bool place_out_now;          // the outbox the host reads is the place's
static bool place_unhash;           // the hash named a place and you walked off it: the page drops it (action 3)
static bool place_unhashed;         // ... once
static int32_t place_out_code;      // the action the place's outbox is for
static uint8_t place_url[128];
static uint8_t place_body[1024];
static int32_t place_ask_id = -1;
static coax_v_t_PlaceKeeper place_k(void) { return (coax_v_t_PlaceKeeper){&place_keeper, 1}; }
static double place_wall_ms(void) { return track_state.epoch > 0 ? track_state.epoch + track_state.t * 1000.0 : 0; }

// You were first put down (the spawn, or where the hash named).
static void place_seen(uint32_t state) { verse_place_open(place_k(), place_here(state)); }

static void place_keep(uint32_t state, bool leaving) {
  coax_t_Place here = place_here(state);
  here.at = place_wall_ms();
  int64_t due = verse_place_due(place_k(), here, track_state.t, leaving);
  if (place_keeper.moved && place_keeper.named && !place_unhashed) { place_unhash = true; place_unhashed = true; }
  if (!due) return;
  int64_t done = 0;
  if (due & 1) {
    int64_t n = verse_place_text(here, verse_place_slot(place_k()), (coax_v_u8){place_text_out, sizeof place_text_out});
    if (n > 0) { place_text_n = (uint32_t)n; done |= 1; }
  }
  if (due & 2) {
    int64_t un = verse_place_url(place_k(), (coax_v_u8){place_url, sizeof place_url});
    int64_t bn = verse_place_body(place_k(), here, (coax_v_u8){place_body, sizeof place_body});
    if (un > 0 && bn > 0 && coaxial_http((uint32_t)(uintptr_t)place_url, (uint32_t)un, (uint32_t)(uintptr_t)place_body, (uint32_t)bn, 1 | 8) >= 0) done |= 2;
  }
  verse_place_kept(place_k(), here, track_state.t, done);
}
static void place_frame(uint32_t state) { place_keep(state, false); }

// The page facts (600: the slot and the account; the server's copy is asked
// for), hidden (601) or left (603: kept at once), the kept text (11) and the
// server's answer. True when the message was the place's alone.
static bool place_message(uint32_t state, int32_t kind, const uint8_t *inbox, uint32_t length) {
  coax_string text = {(const char *)inbox, (int64_t)length};
  if (kind == 600) {
    verse_place_facts(place_k(), text);
    if (verse_place_ask(place_k())) {
      int64_t un = verse_place_url(place_k(), (coax_v_u8){place_url, sizeof place_url});
      if (un > 0) place_ask_id = coaxial_http((uint32_t)(uintptr_t)place_url, (uint32_t)un, 0, 0, 4 | 8);
    }
    return false;
  }
  if (kind == 601 || kind == 603) { place_keep(state, true); return false; }
  if (kind == PLACE_MESSAGE) {
    coax_t_Place p = verse_place_read(text, verse_place_slot(place_k()));
    if (verse_place_take(place_k(), p)) place_put(state, p);
    return true;
  }
  if (place_ask_id >= 0 && kind == 512 + place_ask_id) {
    place_ask_id = -1;
    coax_t_Place p = verse_place_server(place_k(), text);
    if (verse_place_take(place_k(), p)) place_put(state, p);
    return true;
  }
  return false;
}
// The page has no action of its own: the place's text, when one is waiting.
// The kept text first (a page being left gets one action), then the hash's drop.
static int32_t place_action(void) {
  if (place_text_n) { place_out_now = true; place_out_code = PLACE_ACTION; return PLACE_ACTION; }
  if (place_unhash) { place_out_now = true; place_out_code = 3; return 3; }
  return 0;
}
static uint32_t place_outbox(void) { return (uint32_t)(uintptr_t)place_text_out; }
static uint32_t place_outbox_len(void) { return place_out_code == 3 ? 0 : place_text_n; }
static void place_done(void) {
  if (place_out_code == 3) place_unhash = false; else place_text_n = 0;
  place_out_now = false; place_out_code = 0;
}
