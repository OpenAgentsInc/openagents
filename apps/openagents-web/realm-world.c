// A player's world on the realm page (world.coax, pt-change-offers),
// included by realm-face.c: the page's hash names the world. Its published
// house opens over the built-in one; its owner's edits are published when
// they leave edit mode (or at the quest's publish step), everyone else's are
// offered to the owner; the owner sees the newest offer waiting and accepts
// or declines it. The Haunted House itself (no hash) keeps its saves as
// before.
static char world[65];
static int64_t world_n;
static bool world_owner, world_changed;
static int32_t world_ask = -1, owner_ask = -1, log_ask = -1, send_ask = -1, decide_ask = -1;
static bool world_offering;
static coax_t_SaveSlots world_slots;
static bool world_waiting;
static char world_url_buf[200];
static char offer_id[65], offer_who[64], offer_what[320];
static int64_t offers_n, offer_who_n, offer_what_n;

static coax_string world_name(void) { return (coax_string){world, world_n}; }
static bool world_holds(void) { return world_n > 0; }

static int32_t world_get(int64_t what, int32_t flags) {
  int64_t n = world_url(world_name(), what, (coax_string){offer_id, (int64_t)strlen(offer_id)}, (coax_v_u8){(uint8_t *)world_url_buf, sizeof world_url_buf});
  return n > 0 ? coaxial_http((uint32_t)(uintptr_t)world_url_buf, (uint32_t)n, 0, 0, (uint32_t)flags) : -1;
}
static void world_ask_log(void) { if (world_owner) log_ask = world_get(2, HTTP_ANSWER_PLAYER); }

static void world_apply(void) {
  if (!world_waiting || !opened) return;
  world_waiting = false;
  bool fell_back = false; double time = 0;
  if (!open_save(server_bytes, sizeof server_bytes, world_slots, &fell_back, &time)) return;
  loaded_at = time;
  house.dirty = false;
  world_changed = false;
}

static void world_take(uint32_t length) {
  world_slots = world_from_server((coax_string){(const char *)inbox, length}, (coax_v_u8){server_bytes, sizeof server_bytes});
  world_waiting = world_slots.cur_n > 0;
  world_apply();
}

static void world_take_offers(uint32_t length) {
  coax_string a = {(const char *)inbox, length};
  coax_t_WorldOffers w = world_offers(a);
  offers_n = w.count;
  if (!w.count) return;
  memcpy(offer_id, inbox + w.id_at, (size_t)w.id_n); offer_id[w.id_n] = 0;
  offer_who_n = world_unescape((coax_string){a.data + w.who_at, w.who_at >= 0 ? w.who_n : 0}, (coax_v_u8){(uint8_t *)offer_who, sizeof offer_who});
  offer_what_n = world_unescape((coax_string){a.data + w.summary_at, w.summary_at >= 0 ? w.summary_n : 0}, (coax_v_u8){(uint8_t *)offer_what, sizeof offer_what});
}

static int world_status(uint32_t length) {
  int status = 0;
  for (uint32_t i = 0; i < length && inbox[i] >= '0' && inbox[i] <= '9'; i++) status = status * 10 + (inbox[i] - '0');
  return status;
}

// The house as it is now, published (the owner) or offered (anyone else).
static bool world_send(void) {
  if (!world_holds() || !opened || send_ask >= 0) return false;
  int64_t n = verse_haunt_save(house, (coax_v_u8){saved, sizeof saved}, (coax_v_u8){scratch, sizeof scratch});
  if (n <= 0) return false;
  int64_t cn = save_pack(text(SAVE_WORLD), SAVE_SCHEMA, (coax_v_u8){saved, n}, n, (coax_v_u8){container, sizeof container});
  if (cn <= 0) return false;
  world_offering = !world_owner && !sandbox;   // the Sandbox is open: everyone publishes
  int64_t bn = world_body((coax_v_u8){container, cn}, cn, wall_ms(), text(sandbox ? "Built in the Sandbox" : placed_n ? "Placed things in the house" : "Changed the house"),
                          world_offering, (coax_v_u8){(uint8_t *)post, sizeof post});
  int64_t un = world_url(world_name(), world_offering ? 3 : 0, (coax_string){"", 0}, (coax_v_u8){(uint8_t *)world_url_buf, sizeof world_url_buf});
  if (bn <= 0 || un <= 0) return false;
  send_ask = coaxial_http((uint32_t)(uintptr_t)world_url_buf, (uint32_t)un, (uint32_t)(uintptr_t)post, (uint32_t)bn, 1 | HTTP_ANSWER_PLAYER);
  if (send_ask < 0) return false;
  world_changed = false;
  toast(world_offering ? "Offering your change to the owner" : "Publishing your world");
  return true;
}

static void world_edited(void) { if (world_holds() && world_changed) world_send(); }

static void world_decide(bool keep_it) {
  if (!world_owner || offers_n <= 0 || decide_ask >= 0) return;
  int64_t n = world_url(world_name(), keep_it ? 4 : 5, (coax_string){offer_id, (int64_t)strlen(offer_id)}, (coax_v_u8){(uint8_t *)world_url_buf, sizeof world_url_buf});
  if (n <= 0) return;
  decide_ask = coaxial_http((uint32_t)(uintptr_t)world_url_buf, (uint32_t)n, (uint32_t)(uintptr_t)"{}", 2, 1 | HTTP_ANSWER_PLAYER);
  if (decide_ask >= 0) offers_n = 0;
  if (decide_ask >= 0) toast(keep_it ? "Accepting the change" : "Declining the change");
}

// The server's answers; true when one was this file's.
static bool world_message(int32_t kind, uint32_t length) {
  if (world_ask >= 0 && kind == 512 + world_ask) { world_ask = -1; world_take(length); return true; }
  if (owner_ask >= 0 && kind == 512 + owner_ask) {
    owner_ask = -1;
    world_owner = world_you((coax_string){(const char *)inbox, length});
    world_ask_log();
    return true;
  }
  if (log_ask >= 0 && kind == 512 + log_ask) { log_ask = -1; world_take_offers(length); return true; }
  if (send_ask >= 0 && kind == 512 + send_ask) {
    send_ask = -1;
    int status = world_status(length);
    toast(status == 200 ? (world_offering ? "Your change is offered to the owner" : sandbox ? "Published: everyone in the Sandbox sees it" : "Published: everyone with the link sees it")
                        : "That didn't reach the server; try again");
    if (status != 200) world_changed = true;
    return true;
  }
  if (decide_ask >= 0 && kind == 512 + decide_ask) {
    decide_ask = -1;
    if (world_status(length) == 200) { world_ask = world_get(0, 4); toast("Done"); }
    else toast("That didn't reach the server; try again");
    world_ask_log();
    return true;
  }
  return false;
}

static void world_describe(coax_t_RealmView *v) {
  v->offers = world_owner ? offers_n : 0;
  v->offer_who = (coax_string){offer_who, offer_who_n};
  v->offer_what = (coax_string){offer_what, offer_what_n};
}

static void world_frame(void) {
  if (!world_holds()) return;
  if (house.dirty) { world_changed = true; house.dirty = false; }
  world_apply();
}

// Opens the named world over the document: what was published, and whether you own it.
static void world_open(coax_string h) {
  if (h.count > 64 || (world_holds() && h.count == world_n && !memcmp(world, h.data, (size_t)h.count))) return;
  memcpy(world, h.data, (size_t)h.count); world[h.count] = 0; world_n = h.count;
  world_owner = false; offers_n = 0; offer_id[0] = 0;
  world_ask = world_get(0, 4);
  owner_ask = world_get(1, HTTP_ANSWER_PLAYER);
}

EXPORT("coaxial_hash") void coaxial_hash(uint32_t state, uint32_t length) {
  (void)state;
  if (length > 64 || flight_hash(inbox, length)) return;
  coax_string h = {(const char *)inbox, length};
  if (sandbox || !world_id_ok(h) || (length == strlen(SAVE_WORLD) && !memcmp(inbox, SAVE_WORLD, length))) return;
  world_open(h);
}
