// Firm reason: 6 (AppKit, WebKit, Metal and IOKit measurements; each check's verdict reads Coax state).
// The Metaverse Engine's checks (Forge.h): each runs off the main thread after startup, drives the real
// window, world, chat and children, writes its report, and ends the app.
#import "Forge.h"
#import "Aethyrforge-Swift.h"
#import <QuartzCore/QuartzCore.h>
#include <IOKit/IOKitLib.h>

static NSInteger num(id v) { return [v isKindOfClass:NSNumber.class] ? [v integerValue] : 0; }
static NSInteger numOr(id v, NSInteger d) { return [v isKindOfClass:NSNumber.class] ? [v integerValue] : d; }
static BOOL yes(id v) { return [v isKindOfClass:NSNumber.class] && [v boolValue]; }
static NSString *tail(NSString *s, NSUInteger n) { return s.length > n ? [s substringFromIndex:s.length - n] : s; }
static void write_json(id object, NSURL *url) { forge_write(forge_json_pretty(object), url); }
static double cpu_percent(int32_t pid, uint64_t before, double seconds) { return (double)(forge_process_cpu_ns(pid) - before) / 1e9 / seconds * 100; }

@implementation ForgeApp (Checks)

- (void)finish { [self main:^{ [self stopChildren]; [self terminate]; }]; }

// The scene tool (aethyrforge-scene) on the scene session: its JSON answer, or throws with its error.
- (NSDictionary *)scene:(NSArray *)args what:(NSString *)what {
  NSURL *session = self.chat.sceneSessionURL;
  FORGE_REQUIRE(session, @"%@ has no scene session.", what);
  NSData *data = forge_cli([self.resources URLByAppendingPathComponent:@"aethyrforge-scene"], [args arrayByAddingObjectsFromArray:@[ @"--session", session.path ]], 30, nil);
  NSDictionary *state = [NSJSONSerialization JSONObjectWithData:data options:0 error:nil];
  if (![state isKindOfClass:NSDictionary.class]) state = @{};
  FORGE_REQUIRE(yes(state[@"ok"]), @"%@: %@", what, state[@"error"] ?: @"unknown failure");
  return state;
}
static NSArray *list(NSDictionary *state, NSString *key) { NSArray *a = state[key]; return [a isKindOfClass:NSArray.class] ? a : @[]; }

// A check's own home for Coder: its folder, the bundled CLI, and (router) the owner's router.json copied in.
- (void)coderHome:(NSURL *)root router:(BOOL)router {
  NSFileManager *fm = NSFileManager.defaultManager;
  NSURL *cwd = [root URLByAppendingPathComponent:@"workspace"];
  [self main:^{ self.chat.home = root; self.chat.cwd = cwd; self.chat.cliOverride = [self.resources URLByAppendingPathComponent:@"openagents"]; }];
  NSError *e = nil;
  FORGE_REQUIRE([fm createDirectoryAtURL:cwd withIntermediateDirectories:YES attributes:nil error:&e], @"%@", e.localizedDescription);
  [fm createDirectoryAtURL:[root URLByAppendingPathComponent:@".openagents/aethyrforge"] withIntermediateDirectories:YES attributes:nil error:nil];
  if (!router) return;
  NSURL *config = [root URLByAppendingPathComponent:@".openagents/coder/router.json"];
  [fm createDirectoryAtURL:config.URLByDeletingLastPathComponent withIntermediateDirectories:YES attributes:nil error:nil];
  if (![fm fileExistsAtPath:config.path] && ![fm copyItemAtURL:[fm.homeDirectoryForCurrentUser URLByAppendingPathComponent:@".openagents/coder/router.json"] toURL:config error:&e]) forge_throw(e.localizedDescription);
}
// One Coder turn to its end (busy false), failing on an approval request.
- (void)turn:(NSString *)prompt what:(NSString *)what {
  [self main:^{ [self.chat startTurn:prompt voice:YES]; }];
  double end = forge_uptime() + 120;
  while (self.chat.busy && forge_uptime() < end) {
    FORGE_REQUIRE(!self.chat.pending, @"%@ requested an approval.", what);
    forge_sleep_ms(100);
  }
  __block NSString *transcript = nil; [self main:^{ transcript = self.chat.transcript.string; }];
  FORGE_REQUIRE(!self.chat.busy && [self.chat.lastEnding hasPrefix:@"finished"], @"%@ failed: %@", what, tail(transcript, 2000));
}
- (NSString *)transcriptTail:(NSUInteger)n { __block NSString *t = nil; [self main:^{ t = self.chat.transcript.string; }]; return tail(t, n); }

- (void)runCheck {
  NSURL *report = self.report;
  if (self.setupProfileURL) { [self waitInventory]; [self setupProfileCheck]; return; }
  if (self.offlineReportURL) { [self waitInventory]; [self offlineCheck]; return; }
  NSURL *profile = self.frameProfileURL;
  if (profile) dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    @try { [self captureFrameProfile:profile]; } @catch (NSException *x) { if (self.benchmarkReportURL) { NSString *m = x.reason; [self main:^{ [self fail:m]; }]; } }
  });
  if (self.launchReport) [self launchCheck];
  if ([self has:@"--hotbar-smoke"]) { [self waitInventory]; [self hotbarSmokeCheck:report]; }
  else if ([self has:@"--voice-warm-smoke"]) { [self waitInventory]; [self voiceWarmSmokeCheck:report]; }
  else if ([self has:@"--audio-smoke"]) [self audioSmokeCheck:report];
  else if (self.microphoneReport) { [self waitInventory]; [self microphoneCheck]; }
  else if ([self has:@"--npc-smoke"]) { [self waitInventory]; [self npcSmokeCheck:report]; }
  else if ([self has:@"--scene-smoke"]) { [self waitInventory]; [self sceneSmokeCheck:report]; }
  else if ([self has:@"--input-smoke"]) { [self waitInventory]; [self inputSmokeCheck:report]; }
  else if ([self has:@"--voice-smoke"]) { [self waitInventory]; [self voiceSmokeCheck:report]; }
  else if ([self has:@"--setup-smoke"]) { [self waitInventory]; [self setupSmokeCheck:report]; }
  else if ([self has:@"--smoke-test"]) { [self waitInventory]; [self main:^{ self.setupHidden = YES; [self updateSetup]; }]; [self smokeCheck:report]; }
}

- (void)launchCheck {
  NSURL *report = self.launchReport;
  [self waitInventory];
  [self main:^{ [self enterWorld]; }];
  forge_write([self launchState], report);
  [self main:^{ self.window.level = NSNormalWindowLevel; }];
  if (![self has:@"--launch-check-retry"] || self.retryCheckStarted) return;
  self.retryCheckStarted = YES;
  dispatch_async(dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{
    [self main:^{ [self.room terminate]; }];
    double end = forge_uptime() + 5;
    while (!self.stopping && forge_uptime() < end) forge_sleep_ms(50);
    __block BOOL ready = NO;
    [self main:^{ [self updateSetup]; ready = self.stopping && self.refreshEnabled && [self.refreshTitle isEqualToString:@"Retry startup"]; }];
    if (!ready) { [[@"Retry control did not become available." dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[report URLByAppendingPathExtension:@"error"] atomically:NO]; return; }
    self.retryCheckPassed = YES;
    [self main:^{ [self setupButton:2]; }];   // the panel's Retry startup
  });
}

// ---- the editor's save, the scene tool, NPCs and the hotbar

- (void)smokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report, @"--smoke-test needs a report path.");
  NSURL *world = [self.dataURL URLByAppendingPathComponent:@"world-everglade.json"];
  NSURL *expected = [report.URLByDeletingLastPathComponent URLByAppendingPathComponent:@"expected-world.json"];
  NSData *before = [NSData dataWithContentsOfURL:world];
  FORGE_REQUIRE(before, @"The local world is missing.");
  BOOL reopened = [NSFileManager.defaultManager fileExistsAtPath:expected.path];
  if (reopened) FORGE_REQUIRE([before isEqualToData:[NSData dataWithContentsOfURL:expected]], @"Reopening replaced the saved local world.");
  else {
    NSDictionary *old = [NSJSONSerialization JSONObjectWithData:before options:0 error:nil];
    forge_sleep_ms(2000);   // the first rendered frame can precede the initial world GET
    printf("Before input: %s\n", [[self js:@"coaxial.probe()"] description].UTF8String);
    // The seed already has a crate directly ahead at the gate. Walk back along the road so the placement obeys the collision rules.
    [self js:@"dispatchEvent(new KeyboardEvent('keydown',{key:'s',code:'KeyS',bubbles:true})); true"];
    forge_sleep_ms(1200);
    [self js:@"dispatchEvent(new KeyboardEvent('keyup',{key:'s',code:'KeyS',bubbles:true})); true"];
    printf("After walking: %s\n", [[self js:@"coaxial.probe()"] description].UTF8String);
    [self js:@"dispatchEvent(new KeyboardEvent('keydown',{key:'b',code:'KeyB',bubbles:true})); dispatchEvent(new KeyboardEvent('keyup',{key:'b',code:'KeyB',bubbles:true})); true"];
    // The real editor's '1' command places a thing only while EDIT is on.
    id used = [self js:@"(()=>{const e=new KeyboardEvent('keydown',{key:'1',code:'Digit1',bubbles:true,cancelable:true});dispatchEvent(e);dispatchEvent(new KeyboardEvent('keyup',{key:'1',code:'Digit1',bubbles:true}));return e.defaultPrevented;})()"];
    printf("Placement key consumed: %s\n", [used description].UTF8String);
    // DONE flushes the existing editor's pending save immediately.
    [self js:@"dispatchEvent(new KeyboardEvent('keydown',{key:'b',code:'KeyB',bubbles:true})); dispatchEvent(new KeyboardEvent('keyup',{key:'b',code:'KeyB',bubbles:true})); true"];
    double deadline = forge_uptime() + 15;
    BOOL saved = NO;
    while (forge_uptime() < deadline) {
      forge_sleep_ms(200);
      NSData *bytes = [NSData dataWithContentsOfURL:world];
      NSDictionary *doc = bytes ? [NSJSONSerialization JSONObjectWithData:bytes options:0 error:nil] : nil;
      if (![doc isKindOfClass:NSDictionary.class] || ![doc[@"version"] isKindOfClass:NSNumber.class]) continue;
      NSString *edits = doc[@"doc"][@"edits"];
      if ([doc[@"version"] integerValue] > numOr(old[@"version"], -1) && [edits isKindOfClass:NSString.class] && ![edits isEqual:old[@"doc"][@"edits"]]) {
        [bytes writeToURL:expected atomically:YES]; saved = YES; break;
      }
    }
    if (!saved) {
      [self captureCanvas:report];
      printf("World requests: %s\n", [[self js:@"JSON.stringify(__forgeRequests)"] description].UTF8String);
      forge_throw(@"The editor's placed thing was not saved through the local room.");
    }
  }
  NSMutableDictionary *object = [forge_parse([self js:@"JSON.stringify({ready:coaxial.ready, scene:coaxial.sceneReady, gpu:!!navigator.gpu, url:location.href, controls:[...document.querySelectorAll('[aria-label]')].map(e=>e.getAttribute('aria-label'))})"]) mutableCopy];
  FORGE_REQUIRE(object, @"Missing scene report.");
  object[@"savedByHand"] = @YES; object[@"reopened"] = @(reopened); object[@"firstFrameSeconds"] = @(self.firstFrameSeconds);
  object[@"roomPID"] = @(self.room.processIdentifier); object[@"pagePID"] = @(self.page.processIdentifier);
  [self captureCanvas:report];
  write_json(object, report);
  [self finish];
}

- (void)sceneSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report && self.chat.sceneSessionURL, @"Scene fixture has no local tool session.");
  [self main:^{ [self enterWorld]; }];
  NSDictionary *(^command)(NSString *) = ^(NSString *json) { return [self scene:json ? @[ @"apply", json ] : @[ @"show" ] what:@"Scene tool"]; };
  NSArray *(^objects)(NSDictionary *) = ^(NSDictionary *s) { return list(s, @"objects"); };
  NSDictionary *original = command(nil);
  if ([self has:@"--scene-reopen"]) {
    NSDictionary *prior = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:[report.URLByDeletingLastPathComponent URLByAppendingPathComponent:@"result.json"]] ?: [NSData data] options:0 error:nil];
    FORGE_REQUIRE((NSInteger)objects(original).count == numOr(prior[@"totalObjects"], -1), @"Scene objects did not survive reopening.");
    if ([prior[@"resizedIndex"] isKindOfClass:NSNumber.class]) {
      NSInteger middle = [prior[@"resizedIndex"] integerValue], size = -1;
      for (NSDictionary *o in objects(original)) if (num(o[@"index"]) == middle) { size = num(o[@"size_percent"]); break; }
      FORGE_REQUIRE(size == 300, @"Individual crate resize did not survive reopening.");
    }
    forge_write(forge_json(@{@"reopened" : @YES, @"totalObjects" : @(objects(original).count), @"resizePersisted" : @YES}), report);
    [self finish]; return;
  }
  NSUInteger firstCount = objects(original).count;
  NSInteger getsBefore = num([self js:@"__forgeRequests.filter(r=>r.method==='GET').length"]);
  NSDictionary *row = command(@"{\"op\":\"place\",\"item\":\"crate\",\"count\":3,\"x_cm\":600,\"z_cm\":-1200}");
  NSDictionary *stack = command(@"{\"op\":\"place\",\"item\":\"crate\",\"count\":5,\"arrangement\":\"stack\",\"x_cm\":1200,\"z_cm\":-1200}");
  FORGE_REQUIRE([row[@"group"] isKindOfClass:NSNumber.class] && [stack[@"group"] isKindOfClass:NSNumber.class], @"Scene tool returned no groups.");
  NSInteger rowGroup = num(row[@"group"]), stackGroup = num(stack[@"group"]);
  NSDictionary *state = command(nil);
  NSMutableArray *lifts = [NSMutableArray array];
  for (NSDictionary *o in objects(state)) if (num(o[@"group"]) == stackGroup && o[@"lift_cm"]) [lifts addObject:o[@"lift_cm"]];
  NSArray *expectedLifts = @[ @0, @93, @186, @279, @372 ];
  FORGE_REQUIRE(objects(state).count == firstCount + 8 && [[lifts sortedArrayUsingSelector:@selector(compare:)] isEqualToArray:expectedLifts], @"Programmatic row/stack did not produce real scene objects.");
  NSMutableArray *rowObjects = [NSMutableArray array];
  for (NSDictionary *o in objects(state)) if (num(o[@"group"]) == rowGroup) [rowObjects addObject:o];
  [rowObjects sortUsingComparator:^NSComparisonResult(NSDictionary *a, NSDictionary *b) { return [@(num(a[@"x_cm"])) compare:@(num(b[@"x_cm"]))]; }];
  FORGE_REQUIRE(rowObjects.count > 1 && [state[@"version"] isKindOfClass:NSNumber.class], @"Scene row had no middle object/version.");
  NSInteger rowMiddle = num(rowObjects[1][@"index"]), resizeVersion = num(state[@"version"]);
  command([NSString stringWithFormat:@"{\"op\":\"resize\",\"index\":%ld,\"version\":%ld,\"scale_percent\":300}", (long)rowMiddle, (long)resizeVersion]);
  state = command(nil);
  NSInteger middleSize = -1;
  for (NSDictionary *o in objects(state)) if (num(o[@"index"]) == rowMiddle) { middleSize = num(o[@"size_percent"]); break; }
  FORGE_REQUIRE(middleSize == 300, @"Programmatic individual resize failed.");
  BOOL staleRefused = NO;
  @try { command([NSString stringWithFormat:@"{\"op\":\"resize\",\"index\":%ld,\"version\":%ld,\"size_percent\":100}", (long)rowMiddle, (long)resizeVersion]); } @catch (NSException *x) { staleRefused = YES; }
  FORGE_REQUIRE(staleRefused, @"An old version resized a selected object.");
  command([NSString stringWithFormat:@"{\"op\":\"move\",\"group\":%ld,\"dz_cm\":400}", (long)rowGroup]);
  command([NSString stringWithFormat:@"{\"op\":\"remove\",\"group\":%ld}", (long)stackGroup]);
  NSDictionary *beforeBad = command(nil);
  BOOL refused = NO;
  @try { command(@"{\"op\":\"place\",\"item\":\"crate\",\"x_cm\":0,\"z_cm\":0}"); } @catch (NSException *x) { refused = YES; }
  state = command(nil);
  FORGE_REQUIRE(refused && [state[@"version"] isEqual:beforeBad[@"version"]], @"Blocked placement changed the scene.");
  [self coderHome:report.URLByDeletingLastPathComponent router:YES];
  NSMutableArray *sessions = [NSMutableArray array];
  NSInteger resizedIndex = -1;
  NSArray *prompts = @[ @"Add three crates to the scene.", @"Make the middle crate of the three you just added 3x larger. Keep the other two crates unchanged.", @"Add a stack of five crates to the scene." ];
  for (NSUInteger turn = 0; turn < prompts.count; turn++) {
    NSDictionary *before = command(nil);
    [self turn:prompts[turn] what:@"Scene Coder"];
    state = command(nil);
    NSInteger added = (NSInteger)objects(state).count - (NSInteger)objects(before).count;
    FORGE_REQUIRE(added == [@[ @3, @0, @5 ][turn] integerValue], @"Coder said it edited the scene but added %ld objects: %@", (long)added, [self transcriptTail:2000]);
    if (turn == 1) {
      NSArray *beforeObjects = objects(before);
      NSArray *last = [[beforeObjects subarrayWithRange:NSMakeRange(beforeObjects.count >= 3 ? beforeObjects.count - 3 : 0, MIN(3u, beforeObjects.count))] sortedArrayUsingComparator:^NSComparisonResult(NSDictionary *a, NSDictionary *b) { return [@(num(a[@"x_cm"])) compare:@(num(b[@"x_cm"]))]; }];
      FORGE_REQUIRE(last.count == 3 && [last[1][@"index"] isKindOfClass:NSNumber.class], @"Natural-language row has no middle crate.");
      NSInteger middle = num(last[1][@"index"]);
      resizedIndex = middle;
      NSArray *now = objects(state);
      for (NSUInteger i = 0; i < MIN(beforeObjects.count, now.count); i++) {
        NSMutableDictionary *intended = [beforeObjects[i] mutableCopy];
        if (num(intended[@"index"]) == middle) intended[@"size_percent"] = @300;
        FORGE_REQUIRE([intended isEqualToDictionary:now[i]], @"Coder resized or rebuilt the wrong objects: %@ -> %@", beforeObjects[i], now[i]);
      }
    }
    [sessions addObject:self.chat.session ?: @""];
  }
  NSArray *all = objects(state);
  NSArray *latest = [all subarrayWithRange:NSMakeRange(all.count >= 5 ? all.count - 5 : 0, MIN(5u, all.count))];
  NSMutableArray *latestLifts = [NSMutableArray array]; NSMutableSet *groups = [NSMutableSet set];
  for (NSDictionary *o in latest) { if (o[@"lift_cm"]) [latestLifts addObject:o[@"lift_cm"]]; if (o[@"group"]) [groups addObject:o[@"group"]]; }
  FORGE_REQUIRE([[latestLifts sortedArrayUsingSelector:@selector(compare:)] isEqualToArray:expectedLifts] && groups.count == 1, @"Natural-language stack was not stacked.");
  forge_sleep_ms(3000);
  NSInteger getsAfter = num([self js:@"__forgeRequests.filter(r=>r.method==='GET' && r.status===200).length"]);
  FORGE_REQUIRE(getsAfter > getsBefore, @"The page did not fetch the changed world.");
  [self captureCanvas:report];
  write_json(@{@"ok" : @YES, @"programmaticRow" : @YES, @"programmaticStack" : @YES, @"groupMoveRemove" : @YES, @"blockedRefused" : @(refused), @"staleResizeRefused" : @(staleRefused),
               @"resizedIndex" : @(resizedIndex), @"middleSizePercent" : @300, @"neighboursUnchanged" : @YES, @"naturalLanguageSessions" : sessions, @"pageFetchedChanges" : @YES,
               @"totalObjects" : @(all.count), @"version" : state[@"version"] ?: @0, @"engine" : self.chat.selectedEngine ?: @""},
             report);
  [self finish];
}

- (void)npcSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report && self.chat.sceneSessionURL, @"NPC check has no scene session.");
  [self main:^{ [self enterWorld]; }];
  NSDictionary *(^command)(NSString *) = ^(NSString *json) { return [self scene:json ? @[ @"npc", json ] : @[ @"npcs" ] what:@"NPC tool"]; };
  NSDictionary *(^raid)(NSString *) = ^(NSString *json) { return [self scene:json ? @[ @"raid", json ] : @[ @"raid" ] what:@"Raid tool"]; };
  NSArray *(^people)(NSDictionary *) = ^(NSDictionary *s) { return list(s, @"npcs"); };
  NSInteger (^probe)(int) = ^(int field) { return numOr([self js:[NSString stringWithFormat:@"coaxial.netProbe(%d)", field]], -1); };
  NSDictionary *(^person)(NSDictionary *, NSInteger) = ^NSDictionary *(NSDictionary *s, NSInteger pid) { for (NSDictionary *p in people(s)) if (num(p[@"id"]) == pid) return p; return nil; };
  if ([self has:@"--npc-reopen"]) {
    FORGE_REQUIRE(people(command(nil)).count == 8, @"Authored NPCs did not survive reopening.");
    NSDictionary *event = [self scene:@[ @"raid" ] what:@"Raid tool"];
    FORGE_REQUIRE([event[@"active"] isEqual:@NO] && [event[@"cult_alive"] isEqual:@0], @"A transient raid replayed on reopening.");
    forge_write(forge_json(@{@"ok" : @YES, @"reopened" : @YES, @"npcs" : @8}), report);
    [self finish]; return;
  }
  command(@"{\"op\":\"spawn\",\"kind\":\"cultist\",\"count\":2,\"x_cm\":600,\"z_cm\":-1200}");
  command(@"{\"op\":\"spawn\",\"kind\":\"villager\",\"x_cm\":600,\"z_cm\":-1000}");
  NSDictionary *beforeBad = command(nil);
  BOOL refused = NO;
  @try { command(@"{\"op\":\"spawn\",\"kind\":\"cultist\",\"x_cm\":0,\"z_cm\":0}"); } @catch (NSException *x) { refused = YES; }
  NSDictionary *afterBad = command(nil);
  FORGE_REQUIRE(refused && [beforeBad[@"version"] isEqual:afterBad[@"version"]], @"Blocked NPC spawn changed the document.");
  BOOL crateRefused = NO;
  @try { command(@"{\"op\":\"spawn\",\"kind\":\"villager\",\"x_cm\":0,\"z_cm\":-8251}"); } @catch (NSException *x) { crateRefused = YES; }
  FORGE_REQUIRE(crateRefused, @"NPC spawn ignored the existing crate footprint.");
  [self coderHome:report.URLByDeletingLastPathComponent router:YES];
  NSMutableArray *sessions = [NSMutableArray array];
  void (^turn)(NSString *) = ^(NSString *prompt) { [self turn:prompt what:@"NPC Coder"]; [sessions addObject:self.chat.session ?: @""]; };
  turn(@"Have the cultists in group 1 attack villager NPC 3.");
  NSDictionary *state = command(nil);
  NSArray *first2 = [people(state) subarrayWithRange:NSMakeRange(0, MIN(2u, people(state).count))];
  BOOL ordered = first2.count == 2;
  for (NSDictionary *p in first2) ordered &= [p[@"action"] isEqual:@"attack"] && num(p[@"target_id"]) == 3;
  FORGE_REQUIRE(ordered, @"Coder did not set the group attack order.");
  double hurtEnd = forge_uptime() + 45;
  while (forge_uptime() < hurtEnd) {
    state = command(nil);
    if (numOr(person(state, 3)[@"hp"], 4) < 4) break;
    forge_sleep_ms(300);
  }
  FORGE_REQUIRE(person(state, 3) && numOr(person(state, 3)[@"hp"], 4) < 4, @"The directed attack did not deal real damage.");
  turn(@"Add three villagers nearby.");
  state = command(nil);
  FORGE_REQUIRE(people(state).count == 6, @"Coder did not spawn exactly three villagers.");
  NSInteger breaksBefore = probe(26);
  turn(@"Stop the cultists in group 1. Add two new cultists centred at x=6 metres, z=-70 metres, then order that new group to attack the stables.");
  state = command(nil);
  NSArray *all = people(state);
  BOOL stables = all.count == 8;
  for (NSUInteger i = all.count >= 2 ? all.count - 2 : 0; i < all.count; i++) stables &= [all[i][@"action"] isEqual:@"attack"] && num(all[i][@"target_id"]) == 9;
  FORGE_REQUIRE(stables, @"Coder did not direct the new cultists at the actual stables.");
  double breakEnd = forge_uptime() + 90;
  while (forge_uptime() < breakEnd) { if (probe(26) > breaksBefore) break; forge_sleep_ms(300); }
  NSInteger breaksAfter = probe(26);
  FORGE_REQUIRE(breaksAfter > breaksBefore, @"The building attack did not break real replicated pieces.");
  command(@"{\"op\":\"order\",\"group\":4,\"action\":\"stop\"}");
  forge_sleep_ms(1000);
  NSInteger visible = probe(16);
  FORGE_REQUIRE(visible >= 7, @"Spawned NPCs did not reach the rendered page: %ld.", (long)visible);
  turn(@"Trigger the planned cultist raid world event now.");
  NSDictionary *event = raid(nil);
  FORGE_REQUIRE(yes(event[@"active"]) && num(event[@"cult_target"]) == 30 && num(event[@"watch_target"]) == 24 && num(event[@"wards"]) == 3, @"Coder did not start the planned siege with its watch and wards: %@", event);
  turn(@"Double the cultist army in the event. Keep the town watch the same size.");
  event = raid(nil);
  FORGE_REQUIRE(num(event[@"cult_target"]) == 60 && num(event[@"sent"]) >= 60 && num(event[@"watch_target"]) == 24, @"Coder did not double the real cult army: %@", event);
  NSInteger armyVisible = probe(16);
  FORGE_REQUIRE(armyVisible >= 60, @"The event army did not reach the rendered page: %ld", (long)armyVisible);
  id priorVersion = event[@"version"];
  BOOL overflowRefused = NO;
  @try { raid(@"{\"op\":\"double\"}"); } @catch (NSException *x) { overflowRefused = YES; }
  event = raid(nil);
  FORGE_REQUIRE(overflowRefused && [event[@"version"] isEqual:priorVersion] && num(event[@"cult_target"]) == 60, @"Overflow changed the event army.");
  [self captureCanvas:report];
  turn(@"Set the event cultist army to 10 fighters, then add another wave of five cult fighters.");
  event = raid(nil);
  FORGE_REQUIRE(num(event[@"cult_target"]) == 15, @"Coder did not add the requested event wave: %@", event);
  raid(@"{\"op\":\"stop\"}");
  event = raid(nil); state = command(nil);
  FORGE_REQUIRE([event[@"active"] isEqual:@NO] && [event[@"cult_alive"] isEqual:@0] && [event[@"watch_alive"] isEqual:@0] && people(state).count == 8, @"Stopping the event failed or removed authored NPCs.");
  write_json(@{@"ok" : @YES, @"npcs" : @8, @"routedSessions" : sessions, @"npcDamage" : @YES, @"buildingBreaks" : @(breaksAfter - breaksBefore), @"visibleCreatures" : @(visible), @"blockedSpawnRefused" : @(refused),
               @"crateSpawnRefused" : @(crateRefused), @"eventDoubledTo" : @60, @"eventVisibleCreatures" : @(armyVisible), @"eventWaveTarget" : @15, @"eventOverflowRefused" : @(overflowRefused), @"eventStopped" : @YES},
             report);
  [self finish];
}

- (void)hotbarSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report, @"--hotbar-smoke needs a report path.");
  [self main:^{ [self enterWorld]; }];
  NSString *body = @"const sleep=ms=>new Promise(r=>setTimeout(r,ms));\n"
                   @"const wait=async f=>{const end=performance.now()+10000;while(!f()&&performance.now()<end)await sleep(20);if(!f())throw Error('Hotbar did not reach the expected state');};\n"
                   @"const key=(type,key,shiftKey=false)=>dispatchEvent(new KeyboardEvent(type,{key,shiftKey,bubbles:true}));\n"
                   @"const press=k=>{key('keydown',k);key('keyup',k);};\n"
                   @"const p=(b,f=0)=>coaxial.hotbarProbe(b,f);\n"
                   @"const box=b=>[p(b,1),p(b,2),p(b,3),p(b,4)];\n"
                   @"const pointer=(type,b)=>{const v=box(b),c=document.querySelector('canvas'),scale=c.width/c.clientWidth;dispatchEvent(new PointerEvent(type,{pointerType:'mouse',pointerId:7,clientX:(v[0]+v[2])/2/scale,clientY:(v[1]+v[3])/2/scale,button:0,buttons:type==='pointerup'?0:1,bubbles:true}));};\n"
                   @"try {\n"
                   @"  await wait(()=>coaxial.sceneReady&&p(21)>=0&&p(22)>=0);\n"
                   @"  if(p(21,6)!==0)throw Error('The editor opened at launch');\n"
                   @"  await wait(()=>p(21,3)>p(21,1)&&p(22,3)>p(22,1));\n"
                   @"  press('b');await wait(()=>p(21,6)===1);press('b');await wait(()=>p(21,6)===0);\n"
                   @"  const before={a:p(21),b:p(22),boxes:[box(21),box(22)]};\n"
                   @"  if(stage===0){\n"
                   @"    if(before.a!==0||before.b!==1)throw Error('Expected a fresh room kit');\n"
                   @"    key('keydown','Shift',true);pointer('pointerdown',21);pointer('pointermove',22);pointer('pointerup',22);key('keyup','Shift');\n"
                   @"  }\n"
                   @"  await wait(()=>p(21)===1&&p(22)===0&&box(21)[0]>box(22)[0]);\n"
                   @"  if(p(21,5)!==-1)throw Error('Dragging cast a spell');\n"
                   @"  press('1');const on1=p(21,5);press('1');press('2');const on2=p(21,5);press('2');\n"
                   @"  if(on1!==22||on2!==21)throw Error('Number keys did not follow the swapped icons: '+on1+','+on2);\n"
                   @"  key('keydown','Shift',true);pointer('pointerdown',21);pointer('pointermove',22);pointer('pointercancel',22);key('keyup','Shift');\n"
                   @"  if(p(21)!==1||p(22)!==0)throw Error('Cancelled drag changed assignment');\n"
                   @"  await wait(()=>!!localStorage.getItem('coaxial.everglade.bar'));\n"
                   @"  const saved=localStorage.getItem('coaxial.everglade.bar');\n"
                   @"  const places=saved.split('places=')[1].trim().split(',').map(Number);\n"
                   @"  if(places.length!==47||new Set(places).size!==47||places.some(v=>v<0))throw Error('Reduced kit corrupted the saved arrangement');\n"
                   @"  return {ok:true,stage,before,after:{a:p(21),b:p(22),boxes:[box(21),box(22)]},on1,on2,cancelled:true,saved};\n"
                   @"} catch(e) {return {ok:false,stage,error:String(e)};}";
  NSMutableArray *results = [NSMutableArray array];
  for (int stage = 0; stage < 2; stage++) {
    NSDictionary *result = [self callJS:body arguments:@{@"stage" : @(stage)}];
    FORGE_REQUIRE(yes(result[@"ok"]), @"Hotbar input regression: %@", result);
    [results addObject:result];
    if (stage == 0) { [self main:^{ [self reloadWorld]; }]; forge_sleep_ms(300); [self waitForScene]; }
  }
  [self captureCanvas:report];
  write_json(@{@"ok" : @YES, @"webkit" : @YES, @"editorClosedOnLaunchAndReload" : @YES, @"editorOpensOnlyOnRequest" : @YES, @"shiftDragAndNumberKeys" : @YES, @"reloadPreservesAssignment" : @YES, @"stages" : results}, report);
  [self finish];
}

// ---- Coder: the window's Enter route, the HUDs and a real CLI round trip

static int dropdowns(NSView *v) {
  int n = [v isKindOfClass:NSPopUpButton.class] || [v isKindOfClass:NSComboBox.class];
  for (NSView *s in v.subviews) n += dropdowns(s);
  return n;
}
// A key through the window, as AppKit delivers it (the host's event hook first).
- (void)key:(unsigned short)code down:(BOOL)down repeat:(BOOL)repeat mods:(NSEventModifierFlags)mods {
  NSString *text = code == 53 ? @"\x1b" : @"\r";
  [self main:^{
    NSEvent *e = [NSEvent keyEventWithType:down ? NSEventTypeKeyDown : NSEventTypeKeyUp location:NSZeroPoint modifierFlags:mods timestamp:forge_uptime() windowNumber:self.window.windowNumber context:nil characters:text charactersIgnoringModifiers:text isARepeat:repeat keyCode:code];
    [self.window sendEvent:e];
  }];
}
- (void)tap:(unsigned short)code { [self key:code down:YES repeat:NO mods:0]; [self key:code down:NO repeat:NO mods:0]; }

- (void)inputSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report, @"--input-smoke needs a report path.");
  ForgeChat *chat = self.chat;
  __block BOOL ok = NO;
  [self main:^{ [self enterWorld]; ok = dropdowns(chat) == 0 && dropdowns((__bridge NSView *)ui_view(self.host)) == 0; }];
  FORGE_REQUIRE(ok, @"Agent and model dropdowns must be replaced by automatic routing.");
  [self main:^{ ok = self.setupHidden && chat.presentation == 0 && !chat.hudShown && chat.inputScroll.hidden && chat.transcriptScroll.hidden && !chat.feedShown; }];
  FORGE_REQUIRE(ok, @"Coder must be completely absent at world entry.");
  [self js:@"globalThis.__forgeEnterDown=0;globalThis.__forgeEnterUp=0;addEventListener('keydown',e=>{if(e.key==='Enter')__forgeEnterDown++});addEventListener('keyup',e=>{if(e.key==='Enter')__forgeEnterUp++});true"];
  for (NSNumber *n in @[ @36, @76 ]) {
    unsigned short code = n.unsignedShortValue;
    [self key:code down:YES repeat:NO mods:0];
    [self main:^{ ok = chat.composerOpen && self.window.firstResponder == chat.input && !chat.inputScroll.hidden && chat.hudShown; }];
    FORGE_REQUIRE(ok, @"Enter did not open and focus Coder.");
    [self key:code down:YES repeat:YES mods:0];
    FORGE_REQUIRE(!chat.busy, @"Holding the opening Enter must not submit a message.");
    [self key:code down:NO repeat:NO mods:0];
    [self main:^{ chat.input.string = @"Retained draft"; }];
    [self tap:53];
    [self main:^{ ok = !chat.composerOpen && chat.presentation == 0 && [chat.input.string isEqualToString:@"Retained draft"]; }];
    FORGE_REQUIRE(ok, @"Escape must hide Coder and preserve the draft.");
    [self tap:code];
    [self main:^{ ok = [chat.input.string isEqualToString:@"Retained draft"]; }];
    FORGE_REQUIRE(ok, @"Opening Coder lost its draft.");
    // Clicking the world while Coder is open must not give Enter back to region chat; the next press focuses the composer again.
    [self main:^{ [self.window makeFirstResponder:self.worldView]; }];
    [self tap:code];
    [self main:^{ ok = self.window.firstResponder == chat.input; [chat closeComposer]; }];
    FORGE_REQUIRE(ok, @"Enter leaked after the world regained focus.");
  }
  [self main:^{ [chat openComposer]; chat.input.string = @"line"; [chat.input setSelectedRange:NSMakeRange(4, 0)]; }];
  [self key:36 down:YES repeat:NO mods:NSEventModifierFlagShift]; [self key:36 down:NO repeat:NO mods:NSEventModifierFlagShift];
  [self main:^{ ok = [chat.input.string isEqualToString:@"line\n"] && !chat.busy; }];
  FORGE_REQUIRE(ok, @"Shift-Enter must insert a line without submitting.");
  [self main:^{ chat.input.string = @""; }];
  [self tap:36];
  FORGE_REQUIRE(chat.composerOpen && !chat.busy, @"An empty draft must stay open.");
  __block int feedVertices = 0;
  [self main:^{
    chat.busy = YES; [chat closeComposer];
    [chat receive:@"{\"kind\":\"tool\",\"label\":\"hidden tool activity\"}"];
    ok = chat.presentation == 0;
  }];
  FORGE_REQUIRE(ok, @"Tool metadata must not reveal Coder controls.");
  [self main:^{
    [chat receive:@"{\"kind\":\"reply\",\"text\":\"Active reply on the left\"}"];
    ok = chat.presentation == 1 && chat.feedShown && !chat.hudShown && chat.heading.hidden && chat.status.hidden && chat.inputScroll.hidden && [chat.messageText isEqualToString:@"Active reply on the left\n\n"];
  }];
  FORGE_REQUIRE(ok, @"An active reply must show only message text.");
  [self main:^{ ok = [chat hitTest:NSMakePoint(30, 500)] == nil; }];
  FORGE_REQUIRE(ok, @"Closed Coder messages must not block the realm.");
  [self main:^{
    [chat appendMessage:@"Aethyr draws this message and owns its five-second lifetime.\nCurrent folder: ~/.openagents/aethyrforge/workspace\n"];
    feedVertices = [chat saveFeedFrame:[report.URLByDeletingPathExtension URLByAppendingPathExtension:@"png"] device:(__bridge id<MTLDevice>)ui_device(self.host) queue:(__bridge id<MTLCommandQueue>)ui_queue(self.host)];
    [chat receive:@"{\"kind\":\"ask\",\"text\":\"Review this request\"}"];
    ok = !chat.composerOpen && chat.allow.hidden && [chat.messageText containsString:@"Press Enter to review"];
  }];
  FORGE_REQUIRE(feedVertices > 0, @"The message feed did not draw.");
  FORGE_REQUIRE(ok, @"Pending tools must wait for review without opening controls.");
  [self tap:36];
  [self main:^{ ok = !chat.allow.hidden && !chat.deny.hidden; }];
  FORGE_REQUIRE(ok, @"Enter must reveal the pending tool's approval controls.");
  [self tap:53];
  FORGE_REQUIRE(chat.busy && chat.pending && !chat.composerOpen, @"Escape must close without stopping the agent or answering a tool.");
  [self main:^{ chat.busy = NO; chat.pending = NO; [chat finishMessages]; ok = chat.presentation == 1 && chat.feedShown; }];
  FORGE_REQUIRE(ok, @"Completed messages must remain visible while Coder is closed.");
  [self tap:36];
  [self main:^{ ok = [chat.transcript.string containsString:@"Active reply on the left"]; [chat newChat]; [chat closeComposer]; }];
  FORGE_REQUIRE(ok, @"Enter must restore conversation history.");
  [self coderHome:report.URLByDeletingLastPathComponent router:NO];
  __block NSURL *context = nil;
  [self main:^{ chat.engineOverride = @"echo"; context = chat.sceneContextURL; chat.sceneContextURL = nil; }];   // echo exercises input and streaming, without echoing the tool manual
  @try {
    [self tap:36];
    [self main:^{ chat.input.string = @"FORGE_ENTER_CODER_OK"; }];
    [self key:36 down:YES repeat:NO mods:0]; [self key:36 down:YES repeat:YES mods:0]; [self key:36 down:NO repeat:NO mods:0];
    [self main:^{ ok = chat.busy && !chat.composerOpen && chat.inputScroll.hidden; }];
    FORGE_REQUIRE(ok, @"Send must collapse the composer and leave the run active.");
    double deadline = forge_uptime() + 30;
    while (chat.busy && forge_uptime() < deadline) forge_sleep_ms(50);
    [self main:^{ ok = !chat.busy && [chat.lastEnding hasPrefix:@"finished"] && [chat.transcript.string containsString:@"FORGE_ENTER_CODER_OK"] && chat.session.length && chat.presentation == 1; }];
    FORGE_REQUIRE(ok, @"Native Enter did not finish its real Coder CLI round trip with the reply still visible.");
  } @finally {
    [self main:^{ chat.sceneContextURL = context; }];
  }
  double finishedAt = forge_uptime();
  forge_sleep_ms(4500);
  [self main:^{ ok = chat.feedShown && !chat.hudShown && chat.inputScroll.hidden && [chat hitTest:NSMakePoint(30, 500)] == nil; }];
  FORGE_REQUIRE(ok, @"The finished reply disappeared early or opened controls.");
  double hideDeadline = forge_uptime() + 2;
  for (;;) { [self main:^{ ok = chat.feedShown; }]; if (!ok || forge_uptime() >= hideDeadline) break; forge_sleep_ms(25); }
  double lingered = forge_uptime() - finishedAt;
  [self main:^{ ok = chat.presentation == 0 && !chat.feedShown; }];
  FORGE_REQUIRE(ok && lingered >= 4.9 && lingered < 6.5, @"Completed reply did not hide after five seconds: %f.", lingered);
  NSString *realm = [self js:@"JSON.stringify({downs:__forgeEnterDown,ups:__forgeEnterUp,typing:!!document.querySelector('input[aria-label=\"Typed text\"]'),scene:coaxial.sceneReady})"] ?: @"{}";
  NSDictionary *state = forge_parse(realm) ?: @{};
  FORGE_REQUIRE([state[@"downs"] isEqual:@0] && [state[@"ups"] isEqual:@0] && [state[@"typing"] isEqual:@NO] && [state[@"scene"] isEqual:@YES], @"Native Enter reached the realm's region chat: %@", realm);
  write_json(@{@"ok" : @YES, @"realm" : state, @"nativeReturnAndKeypad" : @YES, @"heldEnter" : @YES, @"shiftEnter" : @YES, @"escapeKeepsDraftAndRun" : @YES, @"replyLingerSeconds" : @(lingered),
               @"feedComponent" : @"aethyr/ui/feed-draw.coax", @"feedVertices" : @(feedVertices), @"realCoder" : chat.session ?: @"", @"ending" : chat.lastEnding ?: @""},
             report);
  printf("Native Enter / Coder presentation smoke passed\n");
  [self finish];
}

// Native input, layout and a real CLI round trip, isolated from the live world (no startup).
- (void)chatSmokeCheck {
  NSURL *report = self.report;
  ForgeChat *chat = self.chat;
  @try {
    FORGE_REQUIRE(report, @"--chat-smoke needs a report path.");
    [self main:^{ self.setupHidden = YES; [self updateSetup]; self.notice.hidden = YES; }];
    [self coderHome:report.URLByDeletingLastPathComponent router:NO];
    NSString *engine = NSProcessInfo.processInfo.environment[@"FORGE_CHAT_SMOKE_ENGINE"] ?: @"echo";
    __block BOOL loaded = NO;
    [self main:^{
      chat.engineOverride = [engine isEqualToString:@"auto"] ? nil : engine;
      [(__bridge NSView *)ui_view(self.host) layoutSubtreeIfNeeded];
      loaded = [chat pipelineOn:(__bridge id<MTLDevice>)ui_device(self.host)] != nil;
    }];
    FORGE_REQUIRE(loaded, @"Aethyr HUD Metal pipeline did not load.");
    void (^ask)(NSString *, double) = ^(NSString *text, double seconds) {
      [self main:^{ chat.input.string = text; [chat submit]; }];
      double end = forge_uptime() + seconds;
      while (chat.busy && forge_uptime() < end) forge_sleep_ms(50);
    };
    ask(@"Remember the token FORGE_CODER_HELIOTROPE. Reply exactly FORGE_CODER_OK. Do not run tools or change files.", 120);
    FORGE_REQUIRE(!chat.busy && [chat.lastEnding hasPrefix:@"finished"] && [[self transcriptTail:100000] containsString:@"FORGE_CODER_OK"] && chat.session.length, @"First Coder turn failed: %@", [self transcriptTail:1200]);
    NSString *first = chat.session;
    NSArray *(^records)(NSString *) = ^(NSString *sid) {
      NSString *log = [NSString stringWithContentsOfURL:[chat.sessions URLByAppendingPathComponent:[sid stringByAppendingString:@".jsonl"]] encoding:NSUTF8StringEncoding error:nil];
      FORGE_REQUIRE(log, @"The session %@ has no log.", sid);
      NSMutableArray *rows = [NSMutableArray array];
      for (NSString *line in [log componentsSeparatedByString:@"\n"]) { NSDictionary *r = forge_parse(line); if ([r isKindOfClass:NSDictionary.class]) [rows addObject:r]; }
      return rows;
    };
    NSString *(^replies)(NSString *) = ^(NSString *sid) {
      NSMutableArray *texts = [NSMutableArray array];
      for (NSDictionary *r in records(sid)) if ([r[@"kind"] isEqual:@"reply"] && [r[@"text"] isKindOfClass:NSString.class]) [texts addObject:r[@"text"]];
      return [texts componentsJoinedByString:@"\n"];
    };
    FORGE_REQUIRE([replies(first) containsString:@"FORGE_CODER_OK"], @"The provider did not reply to the first turn.");
    ask(@"What token did I ask you to remember? Reply with only that token. Do not run tools.", 120);
    FORGE_REQUIRE(!chat.busy && [chat.lastEnding hasPrefix:@"finished"] && ![chat.session isEqualToString:first], @"Follow-up Coder turn failed.");
    NSString *log = [NSString stringWithContentsOfURL:[chat.sessions URLByAppendingPathComponent:[chat.session stringByAppendingString:@".jsonl"]] encoding:NSUTF8StringEncoding error:nil];
    FORGE_REQUIRE([log containsString:[NSString stringWithFormat:@"\"parent\":\"%@\"", first]] && [replies(chat.session) containsString:@"FORGE_CODER_HELIOTROPE"], @"Coder did not carry the previous turn.");
    if ([engine isEqualToString:@"auto"])
      for (NSString *sid in @[ first, chat.session ]) {
        BOOL routed = NO;
        for (NSDictionary *r in records(sid)) routed |= [r[@"kind"] isEqual:@"route"] && [r[@"schema"] isEqual:@"coder.route.v2"] && yes(r[@"ok"]);
        FORGE_REQUIRE(routed, @"Automatic first/follow-up turn did not receive an Elephant decision.");
      }
    NSString *followup = chat.session;
    BOOL approved = NO, stopped = NO; int approvalRequests = 0;
    if ([NSProcessInfo.processInfo.environment[@"FORGE_CHAT_SMOKE_ACTIONS"] isEqualToString:@"1"]) {
      [self main:^{ chat.input.string = @"Use a shell command to write exactly HUD_APPROVED to hud-approved.txt in the working directory. Do not write any other file."; [chat submit]; }];
      double end = forge_uptime() + 120;
      while (chat.busy && forge_uptime() < end) {
        if (chat.pending) { approvalRequests++; [self main:^{ [chat stopTurn]; }]; break; }
        forge_sleep_ms(50);
      }
      while (chat.busy && forge_uptime() < end) forge_sleep_ms(50);
      NSString *written = [NSString stringWithContentsOfURL:[chat.cwd URLByAppendingPathComponent:@"hud-approved.txt"] encoding:NSUTF8StringEncoding error:nil];
      approved = [[written stringByTrimmingCharactersInSet:NSCharacterSet.whitespaceAndNewlineCharacterSet] isEqualToString:@"HUD_APPROVED"];
      FORGE_REQUIRE(!chat.busy && approved && approvalRequests == 0, @"Coder unattended write flow failed: %@", [self transcriptTail:1200]);
      [self main:^{ chat.input.string = @"Run the shell command sleep 60, then reply DONE."; [chat submit]; }];
      double stopEnd = forge_uptime() + 30;
      while (chat.busy && !chat.session.length && forge_uptime() < stopEnd) forge_sleep_ms(50);
      [self main:^{ [chat stopTurn]; }];
      while (chat.busy && forge_uptime() < stopEnd) forge_sleep_ms(50);
      stopped = !chat.busy && [chat.lastEnding hasPrefix:@"stopped"];
      FORGE_REQUIRE(stopped, @"Coder did not stop the active turn.");
    }
    __block NSString *route = nil; [self main:^{ route = chat.router.stringValue; }];
    write_json(@{@"ok" : @YES, @"engine" : engine, @"actualEngine" : chat.selectedEngine ?: @"", @"route" : route ?: @"", @"first" : first, @"followup" : followup, @"ending" : chat.lastEnding ?: @"",
                 @"unattendedWrite" : @(approved), @"approvalRequests" : @(approvalRequests), @"stop" : @(stopped), @"hud" : @"aethyr/ui/feed-draw.coax", @"cli" : chat.cli.path},
               report);
    printf("Coder HUD smoke passed: %s\n", engine.UTF8String);
  } @catch (NSException *x) {
    if (report) [[x.reason dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[report URLByAppendingPathExtension:@"error"] atomically:NO];
    printf("%s\n", x.reason.UTF8String);
  }
  [self main:^{ [chat shutdown]; [self terminate]; }];
}

// ---- voice and the microphone

// Warmup observes real local WELCOME and worklet preparation while prohibiting hardware
// microphone access; silence then exercises three holds.
- (void)voiceWarmSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report, @"--voice-warm-smoke needs a report path.");
  [self main:^{ [self enterWorld]; }];
  [self callJS:@"const {instance}=await WebAssembly.instantiate(await (await fetch('/build/voice.wasm')).arrayBuffer(),{wasi_snapshot_preview1:new Proxy({},{get:()=>()=>0})});globalThis.__warmCodec=instance.exports;return true;" arguments:nil];
  [self js:@"globalThis.__warmCheck={microphones:0,sockets:0,sends:0,phases:[],wire:[],received:[]};\n"
           @"const decode=b=>{const a=ArrayBuffer.isView(b)?new Uint8Array(b.buffer,b.byteOffset,b.byteLength):new Uint8Array(b),x=__warmCodec;new Uint8Array(x.memory.buffer,x.vw_frame(),a.length).set(a);const kind=x.vw_decode(a.length),field=x.vw_field(0),epoch=x.vw_field(3),samples=x.vw_field(4);let n=x.vw_string(2);const turn=new TextDecoder().decode(new Uint8Array(x.memory.buffer,x.vw_text(),n));n=x.vw_string(0);const text=new TextDecoder().decode(new Uint8Array(x.memory.buffer,x.vw_text(),n));return {kind,field,epoch,samples,turn,text};};\n"
           @"const W=WebSocket;globalThis.WebSocket=class extends W {\n"
           @"  constructor(...a){super(...a);this.__local=String(a[0])===coaxialVoiceHost;if(this.__local){__warmCheck.sockets++;globalThis.__warmLast=this;this.addEventListener('message',e=>{if(e.data instanceof ArrayBuffer)__warmCheck.received.push(decode(e.data));});}}\n"
           @"  send(b){if(this.__local){__warmCheck.sends++;const row=decode(b);const last=__warmCheck.wire.at(-1);if(row.kind===3&&last?.kind===3)last.audioFrames=(last.audioFrames||1)+1;else __warmCheck.wire.push(row);}return super.send(b);}\n"
           @"};\n"
           @"const original=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);\n"
           @"globalThis.__warmRestore=()=>{navigator.mediaDevices.getUserMedia=original;globalThis.WebSocket=W;};\n"
           @"navigator.mediaDevices.getUserMedia=async()=>{__warmCheck.microphones++;throw new Error('Warmup must not request the microphone');};\n"
           @"true"];
  double started = forge_uptime();
  [self prepareVoice];
  double warmSeconds = forge_uptime() - started;
  NSDictionary *idle = [self callJS:@"const a={...__warmCheck,state:coaxial.voice.prepared()};\nawait new Promise(r=>setTimeout(r,250));\nreturn {...a,idleExtraSends:__warmCheck.sends-a.sends};" arguments:nil] ?: @{};
  FORGE_REQUIRE([idle[@"microphones"] isEqual:@0] && [idle[@"idleExtraSends"] isEqual:@0] && yes(idle[@"state"][@"ready"]), @"Voice prewarm did not become ready without capture: %@", idle);
  NSDictionary *holdReport = [self callJS:
      @"const fixture=new AudioContext();await fixture.resume();\n"
      @"const result=[];\n"
      @"const toggle=coaxial.voice.toggle;\n"
      @"coaxial.voice.toggle=want=>{const pending=toggle(want);if(want&&want!=='end')pending.then(()=>{globalThis.__warmHoldReady=performance.now();});return pending;};\n"
      @"try {\n"
      @"const key=(type,k)=>{dispatchEvent(new KeyboardEvent(type,{key:k,code:'Key'+k.toUpperCase(),bubbles:true}));for(let i=0;i<2;i++)if(globalThis.__forgeFrame){const f=__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());}};\n"
      @"for(let i=0;i<3;i++){\n"
      @"  await coaxial.voice.warm();\n"
      @"  const before=__warmCheck.sockets;\n"
      @"  const capture=fixture.createMediaStreamDestination();\n"
      @"  navigator.mediaDevices.getUserMedia=async()=>{__warmCheck.microphones++;return capture.stream;};\n"
      @"  const phaseAt=globalThis.coaxialVoice?.length||0;\n"
      @"  globalThis.__warmHoldReady=0;\n"
      @"  const started=performance.now();\n"
      @"  if(i===0){key('keydown','v');const end=performance.now()+1500;while(!(globalThis.coaxialVoice||[]).slice(phaseAt).some(s=>s.startsWith('listening|'))&&performance.now()<end)await new Promise(r=>setTimeout(r,1));}\n"
      @"  else await coaxial.voice.toggle('on');\n"
      @"  const state=coaxial.voice.prepared();\n"
      @"  result.push({milliseconds:__warmHoldReady?__warmHoldReady-started:10000,actualV:i===0,extraSockets:__warmCheck.sockets-before,connecting:(globalThis.coaxialVoice||[]).slice(phaseAt).some(s=>s.startsWith('connecting|')),...state});\n"
      @"  await new Promise(r=>setTimeout(r,80));\n"
      @"  if(i===0)key('keyup','v');else coaxial.voice.toggle(false);\n"
      @"  const released=performance.now()+500;\n"
      @"  while(capture.stream.getTracks().some(t=>t.readyState==='live')&&performance.now()<released)await new Promise(r=>setTimeout(r,5));\n"
      @"  if(capture.stream.getTracks().some(t=>t.readyState==='live'))throw new Error('Capture survived cancellation');\n"
      @"  await new Promise(r=>setTimeout(r,100));\n"
      @"}\n"
      @"return {rows:result};\n"
      @"} catch(e) {return {rows:result,error:String(e),stack:e.stack,state:coaxial.voice.prepared()};}\n"
      @"finally {coaxial.voice.toggle(false);coaxial.voice.toggle=toggle;await fixture.close();}"
                                arguments:nil] ?: @{};
  NSArray *holds = list(holdReport, @"rows");
  NSString *wav = [[NSData dataWithContentsOfURL:[self.resources URLByAppendingPathComponent:@"hello-foreman.wav"]] base64EncodedStringWithOptions:0];
  NSString *crates = [[NSData dataWithContentsOfURL:[self.resources URLByAppendingPathComponent:@"add-two-giant-crates.wav"]] base64EncodedStringWithOptions:0];
  FORGE_REQUIRE(wav && crates, @"The voice fixtures are missing.");
  NSDictionary *speech = [self callJS:
      @"const originalFinal=globalThis.coaxialVoiceFinal;\n"
      @"const originalMedia=navigator.mediaDevices.getUserMedia.bind(navigator.mediaDevices);\n"
      @"const fixture=new AudioContext();await fixture.resume();\n"
      @"try {\n"
      @"  let transcript='',tracksClosed=true,heldDuringPause=false;const transcripts=[];\n"
      @"  globalThis.coaxialVoiceFinal=e=>{transcript=e.text;return true;};\n"
      @"  const buffer=await fixture.decodeAudioData(Uint8Array.from(atob(wav),c=>c.charCodeAt(0)).buffer);\n"
      @"  const crates=await fixture.decodeAudioData(Uint8Array.from(atob(cratesWav),c=>c.charCodeAt(0)).buffer);\n"
      @"  const gap=2.4,leading=1,rate=fixture.sampleRate;\n"
      @"  const whole=fixture.createBuffer(1,Math.ceil((leading+crates.duration+gap+buffer.duration+0.3)*rate),rate);\n"
      @"  whole.getChannelData(0).set(crates.getChannelData(0),Math.round(leading*rate));\n"
      @"  whole.getChannelData(0).set(buffer.getChannelData(0),Math.round((leading+crates.duration+gap)*rate));\n"
      @"  const key=type=>{dispatchEvent(new KeyboardEvent(type,{key:'v',code:'KeyV',bubbles:true}));for(let i=0;i<2;i++)if(globalThis.__forgeFrame){const f=__forgeFrame;globalThis.__forgeFrame=null;f(performance.now());}};\n"
      @"  for(let i=0;i<2;i++){\n"
      @"    transcript='';const source=fixture.createBufferSource();source.buffer=i===0?whole:buffer;\n"
      @"    const capture=fixture.createMediaStreamDestination();source.connect(capture);\n"
      @"    navigator.mediaDevices.getUserMedia=async()=>capture.stream;\n"
      @"    await coaxial.voice.warm();coaxial.voice.face('Coding agent');\n"
      @"    if(i===0){const at=globalThis.coaxialVoice.length;key('keydown');const end=performance.now()+1500;while(!globalThis.coaxialVoice.slice(at).some(s=>s.startsWith('listening|'))&&performance.now()<end)await new Promise(r=>setTimeout(r,5));}\n"
      @"    else await coaxial.voice.toggle('hold');\n"
      @"    const played=new Promise(r=>{source.onended=r;source.start();});\n"
      @"    if(i===0){\n"
      @"      await new Promise(r=>setTimeout(r,(leading+crates.duration+gap-0.3)*1000));\n"
      @"      const state=coaxial.voice.prepared();heldDuringPause=state.running&&state.microphone&&!transcript;\n"
      @"      if(!heldDuringPause)throw Error('Held V finalized or closed capture during the pause');\n"
      @"    }\n"
      @"    await played;\n"
      @"    await new Promise(r=>setTimeout(r,300));\n"
      @"    if(i===0)key('keyup');else await coaxial.voice.toggle('end');\n"
      @"    const end=performance.now()+20000;while(!transcript && performance.now()<end)await new Promise(r=>setTimeout(r,50));\n"
      @"    transcripts.push(transcript);tracksClosed=tracksClosed&&capture.stream.getTracks().every(t=>t.readyState==='ended');\n"
      @"    coaxial.voice.toggle(false);await coaxial.voice.warm();\n"
      @"  }\n"
      @"  const late=fixture.createMediaStreamDestination();let deliver;\n"
      @"  navigator.mediaDevices.getUserMedia=()=>new Promise(r=>{deliver=r});\n"
      @"  const starting=coaxial.voice.toggle('on');coaxial.voice.toggle(false);deliver(late.stream);await starting;\n"
      @"  const cancelled=late.stream.getTracks().every(t=>t.readyState==='ended');\n"
      @"  await new Promise(r=>setTimeout(r,100));await coaxial.voice.warm();\n"
      @"  const recovery=fixture.createMediaStreamDestination();navigator.mediaDevices.getUserMedia=async()=>recovery.stream;\n"
      @"  await coaxial.voice.toggle('on');const sockets=__warmCheck.sockets;__warmLast.close();\n"
      @"  await new Promise(r=>setTimeout(r,30));coaxial.voice.toggle(false);await coaxial.voice.warm();\n"
      @"  await new Promise(r=>setTimeout(r,650));\n"
      @"  const reconnected=coaxial.voice.prepared().ready && __warmCheck.sockets===sockets+1 && recovery.stream.getTracks().every(t=>t.readyState==='ended');\n"
      @"  const {startVoice}=await import('/coaxial/voice.js');const phases=[];\n"
      @"  const cold=startVoice({send:(_,s)=>phases.push(s),route:'everglade',wasmUrl:'/build/voice.wasm'});\n"
      @"  const fresh=fixture.createMediaStreamDestination();navigator.mediaDevices.getUserMedia=async()=>fresh.stream;\n"
      @"  await cold.toggle('on');const until=performance.now()+3000;\n"
      @"  while(!phases.some(s=>s.startsWith('listening|')) && performance.now()<until)await new Promise(r=>setTimeout(r,20));\n"
      @"  const coldReady=phases.some(s=>s.startsWith('listening|'));cold.toggle(false);\n"
      @"  return {transcript,transcripts,heldDuringPause,heldSeconds:whole.duration,cancelled,coldReady,reconnected,tracksClosed,wire:__warmCheck.wire,received:__warmCheck.received};\n"
      @"} catch(e) {return {error:String(e)};}\n"
      @"finally {coaxial.voice.toggle(false);navigator.mediaDevices.getUserMedia=originalMedia;globalThis.coaxialVoiceFinal=originalFinal;await fixture.close();__warmRestore();}"
                            arguments:@{@"wav" : wav, @"cratesWav" : crates}] ?: @{};
  NSString *transcript = [speech[@"transcript"] isKindOfClass:NSString.class] ? [speech[@"transcript"] lowercaseString] : @"";
  NSArray *transcripts = list(speech, @"transcripts");
  BOOL heard = transcripts.count == 2 && [transcripts.firstObject rangeOfString:@"add (two|2|to) giant crates" options:NSRegularExpressionSearch | NSCaseInsensitiveSearch].location != NSNotFound;
  for (NSString *t in transcripts) heard &= [t.lowercaseString containsString:@"hello"] && [t.lowercaseString containsString:@"building today"];
  FORGE_REQUIRE(heard && yes(speech[@"heldDuringPause"]) && [transcript containsString:@"hello"] && [transcript containsString:@"building today"] && yes(speech[@"cancelled"]) && yes(speech[@"coldReady"]) && yes(speech[@"reconnected"]) && yes(speech[@"tracksClosed"]),
                @"Prepared voice speech/cancellation regression: %@", speech);
  write_json(@{@"warmSeconds" : @(warmSeconds), @"idle" : idle, @"holds" : holds, @"speech" : speech, @"hardwareMicrophone" : @NO}, report);
  BOOL prepared = !holdReport[@"error"] && holds.count == 3;
  for (NSDictionary *h in holds) prepared &= [h[@"extraSockets"] isEqual:@0] && [h[@"connecting"] isEqual:@NO] && yes(h[@"connected"]) && yes(h[@"microphone"]) && [h[@"milliseconds"] doubleValue] < 250;
  FORGE_REQUIRE(prepared, @"A prepared voice hold still waited for a connection: %@", holdReport);
  [self finish];
}

- (void)voiceSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report && !self.voiceHost, @"Voice loaded before it was requested.");
  [self main:^{ [self enterWorld]; }];
  FORGE_REQUIRE(self.setupHidden, @"Voice-to-Coder needs the ready world.");
  ForgeChat *chat = self.chat;
  [self coderHome:report.URLByDeletingLastPathComponent router:YES];
  [self main:^{ chat.input.string = @"UNSENT_TYPED_DRAFT"; }];
  [self ensureVoiceHost];
  NSString *fixture = [[NSData dataWithContentsOfURL:[self.resources URLByAppendingPathComponent:@"hello-foreman.wav"]] base64EncodedStringWithOptions:0];
  FORGE_REQUIRE(fixture, @"The voice fixture is missing.");
  [self js:[NSString stringWithFormat:
      @"globalThis.__voiceCheck = []; globalThis.__voiceError = ''; globalThis.__voiceSockets = 0; globalThis.__voiceFinals = [];\n"
      @"globalThis.__voiceRuntimeErrors = [];\n"
      @"addEventListener('error',e=>__voiceRuntimeErrors.push(String(e.message)));\n"
      @"addEventListener('unhandledrejection',e=>__voiceRuntimeErrors.push(String(e.reason)));\n"
      @"globalThis.__voiceDeliverFinal = globalThis.coaxialVoiceFinal;\n"
      @"globalThis.coaxialVoiceFinal = e => {__voiceFinals.push(e); return __voiceDeliverFinal(e);};\n"
      @"globalThis.__voiceTest = (async () => {\n"
      @"  const {startVoice} = await import('/coaxial/voice.js');\n"
      @"  const W = WebSocket;\n"
      @"  globalThis.WebSocket = class extends W { constructor(...a) { super(...a); if(String(a[0]) === coaxialVoiceHost) __voiceSockets++; } };\n"
      @"  const md = navigator.mediaDevices, original = md.getUserMedia.bind(md);\n"
      @"  const fixtureContext = new AudioContext(); await fixtureContext.resume();\n"
      @"  globalThis.__voiceFixtureContext = fixtureContext;\n"
      @"  const dest = fixtureContext.createMediaStreamDestination();\n"
      @"  let deliver;\n"
      @"  md.getUserMedia = () => new Promise(r => {deliver = r});\n"
      @"  const cancelled = startVoice({send:()=>{},route:'everglade',wasmUrl:'/build/voice.wasm'});\n"
      @"  const starting = cancelled.toggle('on'); cancelled.toggle(false); deliver(dest.stream);\n"
      @"  await starting;\n"
      @"  globalThis.__voiceCancelled = !cancelled.running() && dest.stream.getTracks().every(t=>t.readyState==='ended') && __voiceSockets===0;\n"
      @"  globalThis.WebSocket = W;\n"
      @"  const source = fixtureContext.createBufferSource();\n"
      @"  const wav = Uint8Array.from(atob('%@'), c=>c.charCodeAt(0));\n"
      @"  source.buffer = await fixtureContext.decodeAudioData(wav.buffer);\n"
      @"  const capture = fixtureContext.createMediaStreamDestination(); source.connect(capture);\n"
      @"  md.getUserMedia = async () => capture.stream;\n"
      @"  globalThis.__voiceFixture = source;\n"
      @"  globalThis.__voiceFixtureEnded = false; source.onended = () => {__voiceFixtureEnded = true;};\n"
      @"  globalThis.__voiceClient = startVoice({send:(k,t)=>__voiceCheck.push(t),route:'everglade',wasmUrl:'/build/voice.wasm'});\n"
      @"  __voiceClient.face('Coding agent');\n"
      @"  globalThis.__voiceRestore = () => {md.getUserMedia=original;};\n"
      @"})().catch(e=>{globalThis.__voiceError=String(e)}); true;", fixture]];
  BOOL (^until)(NSString *, double, int) = ^BOOL(NSString *probe, double seconds, int every) {
    double end = forge_uptime() + seconds;
    while (forge_uptime() < end) { if (yes([self js:probe])) return YES; forge_sleep_ms(every); }
    return NO;
  };
  until(@"!!globalThis.__voiceClient", 15, 50);
  [self js:@"__voiceClient.toggle('on'); true"];
  FORGE_REQUIRE(until(@"__voiceCheck.some(t=>t.startsWith('listening|'))", 15, 100), @"Browser voice did not listen: %@", [self js:@"__voiceError || JSON.stringify(__voiceCheck)"]);
  [self js:@"__voiceFixture.start(); true"];
  until(@"__voiceFixtureEnded", 15, 100);
  forge_sleep_ms(800);
  [self js:@"__voiceClient.toggle('end'); true"];
  double finalDeadline = forge_uptime() + 30;
  NSString *transcript = @"";
  while (forge_uptime() < finalDeadline) {
    id t = [self js:@"__voiceFinals[0]?.text || ''"];
    transcript = [t isKindOfClass:NSString.class] ? t : @"";
    if (transcript.length) break;
    forge_sleep_ms(100);
  }
  BOOL cancelled = yes([self js:@"__voiceCancelled === true"]);
  [self js:@"__voiceClient.toggle(false); __voiceRestore?.(); __voiceFixtureContext.close(); true"];
  if (!cancelled || ![transcript.lowercaseString containsString:@"hello foreman"] || ![transcript.lowercaseString containsString:@"building today"])
    forge_throw([NSString stringWithFormat:@"The local browser voice fixture failed: %@. Cancellation: %d. %@", transcript, cancelled, [self js:@"JSON.stringify({phases:__voiceCheck,error:__voiceError,runtime:__voiceRuntimeErrors,finals:__voiceFinals})"]]);
  double coderDeadline = forge_uptime() + 90;
  int deniedRequests = 0;
  while ((chat.busy || !chat.session.length) && forge_uptime() < coderDeadline) {
    if (chat.pending) { [self main:^{ [chat reject]; }]; deniedRequests++; }
    if (deniedRequests > 3) { [self main:^{ [chat stopTurn]; }]; break; }
    forge_sleep_ms(50);
  }
  __block BOOL ok = NO;
  [self main:^{ ok = !chat.busy && [chat.lastEnding hasPrefix:@"finished"] && chat.liveChunks > 0 && chat.streamedWithoutComposer && !chat.composerOpen && [chat.input.string isEqualToString:@"UNSENT_TYPED_DRAFT"] && chat.session.length; }];
  FORGE_REQUIRE(ok, @"Final Parakeet words did not finish a routed, streamed Coder turn with the bar closed: %@", chat.lastEnding);
  NSString *session = chat.session;
  NSString *log = [NSString stringWithContentsOfURL:[chat.sessions URLByAppendingPathComponent:[session stringByAppendingString:@".jsonl"]] encoding:NSUTF8StringEncoding error:nil] ?: @"";
  FORGE_REQUIRE([log containsString:@"\"schema\":\"coder.route.v2\""] && [log containsString:@"\"ok\":true"] && [log containsString:@"\"kind\":\"reply\""] && ![log containsString:@"reply_delta"], @"Routed voice turn did not retain a canonical whole-reply log.");
  [self js:@"__voiceDeliverFinal(__voiceFinals[0]); true"];
  forge_sleep_ms(200);
  FORGE_REQUIRE(!chat.busy && [chat.session isEqualToString:session], @"A duplicate final started another Coder turn.");
  write_json(@{@"local" : @YES, @"transcript" : transcript, @"lateMicrophoneCancelled" : @(cancelled), @"voicePID" : @(self.voiceHost.processIdentifier), @"roomPID" : @(self.room.processIdentifier),
               @"pagePID" : @(self.page.processIdentifier), @"startupModelLoaded" : @NO, @"coderSession" : session, @"engine" : chat.selectedEngine ?: @"", @"liveChunks" : @(chat.liveChunks),
               @"streamedWithBarClosed" : @(chat.streamedWithoutComposer), @"draftPreserved" : @YES, @"duplicateIgnored" : @YES, @"canonicalLog" : @YES, @"fixtureToolRequestsDenied" : @(deniedRequests)},
             report);
  [self finish];
}

- (void)microphoneCheck {
  NSURL *report = self.microphoneReport;
  @try {
    [self main:^{ [self showSetup]; }];
    BOOL prepared = [self authorizeMicrophone];
    __block BOOL ok = NO;
    [self main:^{ ok = [self.microphoneStatusText containsString:@"Microphone allowed"]; }];
    FORGE_REQUIRE(prepared && ok && !self.voiceHost && self.microphoneDecisions == 0, @"Microphone permission was not ready during setup before the first V capture.");
    [self main:^{ [self enterWorld]; }];
    FORGE_REQUIRE(self.setupHidden, @"Microphone check needs the ready world.");
    [self main:^{ [self.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES]; }];
    forge_sleep_ms(300);
    // Reproduce the reported state: visible foreground app, no key window.
    __block BOOL withoutKey = NO;
    [self main:^{ [self.window resignKeyWindow]; withoutKey = !self.window.keyWindow; }];
    NSDictionary *first = [self callJS:@"const stream=await navigator.mediaDevices.getUserMedia({audio:true});\n"
                                       @"const live=stream.getAudioTracks().some(t=>t.readyState==='live');\n"
                                       @"stream.getTracks().forEach(t=>t.stop());\n"
                                       @"await coaxialVoicePrepareCapture(101);\n"
                                       @"const ticket=coaxialCoderVoiceTicket;\n"
                                       @"await coaxialVoiceCancelCapture(101);\n"
                                       @"return {live,ticket,closed:stream.getTracks().every(t=>t.readyState==='ended')};"
                             arguments:nil] ?: @{};
    self.microphoneCheckFailStartup = YES;
    NSDictionary *second = [self callJS:@"const stream=await navigator.mediaDevices.getUserMedia({audio:true});\n"
                                        @"stream.getTracks().forEach(t=>t.stop());\n"
                                        @"let failed=false;try{await coaxialVoicePrepareCapture(102);}catch(e){failed=String(e).includes('test startup failure');}\n"
                                        @"const again=await navigator.mediaDevices.getUserMedia({audio:true});\n"
                                        @"const live=again.getAudioTracks().some(t=>t.readyState==='live');\n"
                                        @"again.getTracks().forEach(t=>t.stop());\n"
                                        @"await coaxialVoicePrepareCapture(103);\n"
                                        @"const ticket=coaxialCoderVoiceTicket;\n"
                                        @"await coaxialVoiceCancelCapture(103);\n"
                                        @"return {live,ticket,failed,closed:again.getTracks().every(t=>t.readyState==='ended')};"
                              arguments:nil] ?: @{};
    self.microphoneCheckDelay = YES;
    BOOL cancelled = yes([self callJS:@"const pending=coaxialVoicePrepareCapture(104);\nawait coaxialVoiceCancelCapture(104);\ntry{await pending;return false;}catch(e){return String(e).includes('Voice stopped');}" arguments:nil]);
    NSInteger one = num(first[@"ticket"]), two = num(second[@"ticket"]);
    FORGE_REQUIRE(withoutKey && yes(first[@"live"]) && yes(second[@"live"]) && yes(first[@"closed"]) && yes(second[@"closed"]) && yes(second[@"failed"]) && one > 0 && two > one && cancelled && self.microphoneDecisions > 0,
                  @"Microphone regression failed: %@, %@, cancelled=%d, key=%d.", first, second, cancelled, !withoutKey);
    write_json(@{@"authorized" : @([AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio] == AVAuthorizationStatusAuthorized), @"realMicrophone" : @YES, @"noKeyWindow" : @(withoutKey),
                 @"tracksClosed" : @YES, @"repeatCapture" : @YES, @"freshTickets" : @[ @(one), @(two) ], @"startupFailureRecovered" : @YES, @"cancelledPreparation" : @(cancelled),
                 @"permissionDecisions" : @(self.microphoneDecisions), @"audioRecordedOrSent" : @NO, @"authorizedBeforeCapture" : @YES},
               report);
  } @catch (NSException *x) {
    [[x.reason dataUsingEncoding:NSUTF8StringEncoding] writeToURL:[report URLByAppendingPathExtension:@"error"] atomically:NO];
  }
  [self finish];
}

- (void)setupSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report && fgs_can_enter(self.sceneReady) && fgs_status(1) == 1, @"Setup did not discover the CLI and installed shared voice model without hashing.");
  // Fake only the macOS authorization transport, never the Coax policy or a microphone stream: the first-run request and a racing V check.
  __block AVAuthorizationStatus status = AVAuthorizationStatusNotDetermined;
  __block int requests = 0;
  [self main:^{
    self.microphoneAuthorization = ^AVAuthorizationStatus { return status; };
    self.microphoneAccessRequest = ^BOOL {
      requests++; forge_sleep_ms(80);
      dispatch_sync(dispatch_get_main_queue(), ^{ status = AVAuthorizationStatusAuthorized; });
      return YES;
    };
    [self refreshMicrophoneSetup];
  }];
  __block BOOL ok = NO;
  [self main:^{ ok = self.microphoneEnabled && fgs_status(3) == 1; }];
  FORGE_REQUIRE(ok, @"Setup has no microphone permission action.");
  __block BOOL granted = NO;
  dispatch_group_t first = dispatch_group_create();
  dispatch_group_async(first, dispatch_get_global_queue(QOS_CLASS_USER_INITIATED, 0), ^{ granted = [self authorizeMicrophone]; });
  forge_sleep_ms(20);
  [self main:^{ ok = self.microphonePrompt && !self.microphoneEnabled; }];
  FORGE_REQUIRE(ok, @"Setup must show an in-flight permission request.");
  BOOL second = [self authorizeMicrophone];
  dispatch_group_wait(first, DISPATCH_TIME_FOREVER);
  [self main:^{ ok = [self.microphoneStatusText containsString:@"Microphone allowed"]; }];
  FORGE_REQUIRE(granted && second && requests == 1 && ok && !self.voiceHost && self.microphoneDecisions == 0, @"Setup did not acquire permission before voice, or duplicated the system request.");
  [self main:^{ status = AVAuthorizationStatusDenied; [self refreshMicrophoneSetup]; }];
  BOOL denied = [self authorizeMicrophone];
  [self main:^{ ok = self.microphoneEnabled && [self.microphoneTitle isEqualToString:@"Open Settings"]; }];
  FORGE_REQUIRE(!denied && requests == 1 && ok, @"Denied permission must link to Settings without asking again.");
  __block BOOL skipped = NO;
  [self main:^{
    self.microphoneAuthorization = nil; self.microphoneAccessRequest = nil; [self refreshMicrophoneSetup];
    skipped = self.setupHidden; ok = skipped == fgs_completed() && self.enterEnabled;
  }];
  FORGE_REQUIRE(ok, @"Launch did not respect the saved setup completion.");
  [self main:^{ [self enterWorld]; ok = self.setupHidden && fgs_completed() && [NSFileManager.defaultManager fileExistsAtPath:self.settingsURL.path]; }];
  FORGE_REQUIRE(ok, @"Setup could not enter the world and keep its choices.");
  [self main:^{ [self showSetup]; [self refreshSetup:NO]; }];
  [self waitInventory];
  FORGE_REQUIRE(!self.setupHidden, @"Manually opened setup disappeared after refresh.");
  __block NSString *microphone = nil; [self main:^{ microphone = self.microphoneStatusText; }];
  NSDictionary *result = @{@"ready" : @(self.sceneReady), @"cli" : @(fgs_status(0) == 1), @"voiceInstalled" : @(fgs_status(1) > 0), @"voiceVerified" : @(fgs_status(1) == 2), @"localAgents" : @(fgs_count(-1)),
                           @"selectedAgent" : @(fgs_selected()), @"voicePath" : forge_setup_text(6), @"setupVisible" : @(!self.setupHidden), @"setupSkippedOnLaunch" : @(skipped), @"settingsSaved" : @YES,
                           @"firstFrameSeconds" : @(self.firstFrameSeconds), @"roomPID" : @(self.room.processIdentifier), @"pagePID" : @(self.page.processIdentifier), @"microphoneBeforeVoice" : @YES,
                           @"permissionRequests" : @(requests), @"concurrentPermissionShared" : @YES, @"deniedPermissionSettings" : @YES, @"microphoneOpened" : @NO, @"microphoneStatus" : microphone ?: @""};
  [self captureCanvas:report];
  write_json(result, report);
  [self finish];
}

// Native Web Audio: the real realm's mix observed before the test-only silent output; normal configuration inspected apart.
- (void)audioSmokeCheck:(NSURL *)report {
  FORGE_REQUIRE(report, @"--audio-smoke needs a report path.");
  __block WKWebView *normal = nil;
  [self main:^{
    normal = [[WKWebView alloc] initWithFrame:NSZeroRect configuration:[WKWebViewConfiguration new]];   // configureAudioOutput adds nothing unmuted
    [normal loadHTMLString:@"<!doctype html><title>Audio configuration check</title>" baseURL:nil];
  }];
  for (;;) { __block BOOL loading = YES; [self main:^{ loading = normal.loading; }]; if (!loading) break; forge_sleep_ms(20); }
  __block id untouched = nil;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  dispatch_async(dispatch_get_main_queue(), ^{ [normal evaluateJavaScript:@"typeof globalThis.coaxialAudioTap === 'undefined'" completionHandler:^(id r, NSError *e) { (void)e; untouched = r; dispatch_semaphore_signal(done); }]; });
  dispatch_semaphore_wait(done, DISPATCH_TIME_FOREVER);
  FORGE_REQUIRE(yes(untouched), @"Normal desktop configuration still overrides Everglade's speaker output.");
  [self main:^{ [normal stopLoading]; }];
  [self js:@"globalThis.__forgeAudioCheck = null;\n"
           @"(async()=>{\n"
           @"  const silent=globalThis.coaxialAudioTap;\n"
           @"  let meter,sink;\n"
           @"  globalThis.coaxialAudioTap=c=>{sink=silent(c);meter=c.createAnalyser();meter.fftSize=2048;meter.connect(sink);return meter;};\n"
           @"  const ctx=coaxial.audio.start();await ctx.resume();\n"
           @"  const samples=new Float32Array(meter.fftSize);let peak=0,cues=0;\n"
           @"  const end=performance.now()+5000;\n"
           @"  while(performance.now()<end){\n"
           @"    if(!cues)cues=coaxial.audio.probe(-1,0,1,0);\n"
           @"    meter.getFloatTimeDomainData(samples);for(const s of samples)peak=Math.max(peak,Math.abs(s));\n"
           @"    await new Promise(r=>setTimeout(r,40));\n"
           @"  }\n"
           @"  __forgeAudioCheck={normalOutput:true,context:ctx.state,peak,cues,testOutputGain:sink.gain.value,beds:coaxial.audio.beds(),microphoneRequested:false};\n"
           @"})().catch(e=>{__forgeAudioCheck={error:String(e)}});true"];
  double deadline = forge_uptime() + 15;
  while (forge_uptime() < deadline) { if (yes([self js:@"!!globalThis.__forgeAudioCheck"])) break; forge_sleep_ms(100); }
  NSString *raw = [self js:@"JSON.stringify(__forgeAudioCheck)"] ?: @"null";
  NSDictionary *state = forge_parse(raw);
  FORGE_REQUIRE([state isKindOfClass:NSDictionary.class] && [state[@"context"] isEqual:@"running"] && [state[@"peak"] doubleValue] > 0.00001 && num(state[@"cues"]) > 0 && [state[@"testOutputGain"] isEqual:@0],
                @"Everglade did not produce live sound through its silent test output: %@", raw);
  forge_write(raw, report);
  [self finish];
}

// ---- profiles and measurements

- (void)captureFrameProfile:(NSURL *)url {
  if (self.benchmarkReportURL) { [self renderBenchmark:url]; return; }
  forge_sleep_ms(3000);
  [self js:@"coaxial.takeFrameTrace();true"];
  NSMutableArray *windows = [NSMutableArray array];
  BOOL walking = [self has:@"--frame-profile-walk"];
  void (^walk)(NSString *, BOOL) = ^(NSString *key, BOOL down) {
    [self js:[NSString stringWithFormat:@"dispatchEvent(new KeyboardEvent('%@',{key:'%@',code:'Key%@',bubbles:true}));true", down ? @"keydown" : @"keyup", key, key.uppercaseString]];
  };
  for (int i = 0; i < 6; i++) {
    if (walking) walk(@"w", YES);
    forge_sleep_ms(2500);
    if (walking) { walk(@"w", NO); walk(@"s", YES); }
    forge_sleep_ms(2500);
    if (walking) walk(@"s", NO);
    if (self.stopping) return;
    NSMutableDictionary *snapshot = [forge_parse([self js:@"JSON.stringify({samples:coaxial.takeFrameTrace(),quality:Array.from({length:9},(_,i)=>coaxial.quality(i)),queue:coaxial.frameQueue(),hidden:document.hidden,focused:document.hasFocus()})"]) mutableCopy] ?: [NSMutableDictionary dictionary];
    [self main:^{
      snapshot[@"visible"] = @(self.window.visible && (self.window.occlusionState & NSWindowOcclusionStateVisible));
      snapshot[@"displayFPS"] = @(self.window.screen.maximumFramesPerSecond);
      snapshot[@"active"] = @(NSApp.active);
    }];
    snapshot[@"thermal"] = @(NSProcessInfo.processInfo.thermalState);
    snapshot[@"lowPower"] = @(NSProcessInfo.processInfo.lowPowerModeEnabled);
    snapshot[@"highRefreshEnabled"] = @(self.highRefreshEnabled);
    snapshot[@"walking"] = @(walking);
    [windows addObject:snapshot];
    forge_write(forge_json(@{@"schema" : @"aethyrforge.frame-profile.v1", @"windows" : windows}), url);
  }
}

- (void)renderBenchmark:(NSURL *)report {
  [self main:^{ [self enterWorld]; }];
  BOOL probeOnly = [self has:@"--bench-probe"];
  int warmup = probeOnly ? 2 : 40;
  double deadline = forge_uptime() + 60;
  while (forge_uptime() < deadline) {
    NSInteger ready = num([self js:@"if(coaxial.netProbe(0)!==2){dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',bubbles:true}));} Math.floor(__evergladeBenchmark.replaySeconds())"]);
    if (ready >= warmup) break;
    forge_sleep_ms(200);
  }
  FORGE_REQUIRE(yes([self js:[NSString stringWithFormat:@"coaxial.netProbe(0)===2 && __evergladeBenchmark.replaySeconds()>=%d", warmup]]), @"Benchmark replay did not connect.");
  [self js:@"__evergladeBenchmark.snapshot();true"];
  __block int32_t pid = 0, gpuPID = 0; [self main:^{ pid = self.webProcessPID; gpuPID = self.gpuProcessPID; }];
  int32_t me = getpid();
  double began = forge_uptime();
  uint64_t appCPU = forge_process_cpu_ns(me), webCPU = forge_process_cpu_ns(pid), gpuCPU = forge_process_cpu_ns(gpuPID);
  NSMutableArray *windows = [NSMutableArray array];
  for (int i = 0; i < (probeOnly ? 1 : 6); i++) {
    [self js:@"__evergladeBenchmark.tap();true"];
    forge_sleep_ms(probeOnly ? 1000 : 5000);
    NSMutableDictionary *w = [forge_parse([self js:@"JSON.stringify(__evergladeBenchmark.snapshot())"]) mutableCopy] ?: [NSMutableDictionary dictionary];
    [self main:^{ w[@"visible"] = @(self.window.visible && (self.window.occlusionState & NSWindowOcclusionStateVisible)); }];
    w[@"webResidentBytes"] = @(forge_process_resident_bytes(pid)); w[@"gpuResidentBytes"] = @(forge_process_resident_bytes(gpuPID));
    w[@"thermal"] = @(NSProcessInfo.processInfo.thermalState); w[@"lowPower"] = @(NSProcessInfo.processInfo.lowPowerModeEnabled);
    [windows addObject:w];
  }
  double seconds = forge_uptime() - began;
  __block NSInteger fps = 0; [self main:^{ fps = self.window.screen.maximumFramesPerSecond; }];
  write_json(@{@"schema" : @"everglade.benchmark.v1", @"host" : @"WKWebView", @"windows" : windows, @"firstFrameSeconds" : @(self.firstFrameSeconds), @"seconds" : @(seconds),
               @"appCPUPercentOfOneCore" : @(cpu_percent(me, appCPU, seconds)), @"webCPUPercentOfOneCore" : @(cpu_percent(pid, webCPU, seconds)), @"appResidentBytes" : @(forge_process_resident_bytes(me)),
               @"webResidentBytes" : @(forge_process_resident_bytes(pid)), @"gpuPID" : @(gpuPID), @"gpuResidentBytes" : @(forge_process_resident_bytes(gpuPID)),
               @"gpuProcessCPUPercentOfOneCore" : @(cpu_percent(gpuPID, gpuCPU, seconds)), @"displayFPS" : @(fps)},
             report);
  [self captureCanvas:report];
  [self finish];
}

static NSNumber *gpu_utilization(void) {
  io_service_t service = IOServiceGetMatchingService(kIOMainPortDefault, IOServiceMatching("AGXAccelerator"));
  if (!service) return nil;
  CFTypeRef stats = IORegistryEntryCreateCFProperty(service, CFSTR("PerformanceStatistics"), kCFAllocatorDefault, 0);
  IOObjectRelease(service);
  if (!stats) return nil;
  NSDictionary *d = CFBridgingRelease(stats);
  id v = [d isKindOfClass:NSDictionary.class] ? d[@"Device Utilization %"] : nil;
  return [v isKindOfClass:NSNumber.class] ? v : nil;
}

- (void)setupProfileCheck {
  NSURL *report = self.setupProfileURL;
  FORGE_REQUIRE(report && self.sceneReady && fgs_can_enter(self.sceneReady), @"Setup profiling needs the real first frame and ready discovery.");
  __block int32_t webPID = 0; [self main:^{ webPID = self.webProcessPID; }];
  FORGE_REQUIRE(webPID > 0, @"Cannot measure the WKWebView process CPU.");
  int32_t me = getpid();
  NSMutableArray *rows = [NSMutableArray array];
  void (^sample)(NSString *, BOOL) = ^(NSString *name, BOOL expectPaused) {
    forge_sleep_ms(1200);
    NSDictionary *before = [self worldLoop];
    double start = forge_uptime();
    uint64_t appCPU = forge_process_cpu_ns(me), webCPU = forge_process_cpu_ns(webPID);
    __block int draws = 0; __block double gpu = 0;
    [self main:^{ draws = self.panelDraws; gpu = self.panelGPUSeconds; }];
    NSMutableArray *utilization = [NSMutableArray array];
    for (int i = 0; i < 40; i++) { NSNumber *v = gpu_utilization(); if (v) [utilization addObject:v]; forge_sleep_ms(200); }
    double elapsed = forge_uptime() - start;
    NSDictionary *after = [self worldLoop];
    NSInteger (^delta)(NSString *) = ^NSInteger(NSString *k) { return num(after[k]) - num(before[k]); };
    NSInteger frames = delta(@"frames"), submissions = delta(@"submissions"), messages = delta(@"socketFrames"), pending = numOr(after[@"pending"], -1);
    FORGE_REQUIRE([after[@"paused"] isEqual:@(expectPaused)] && (expectPaused ? frames == 0 && submissions == 0 && pending == 0 : frames > 0 && submissions > 0) && messages > 0,
                  @"World/room visibility regression in %@: %@, %@", name, before, after);
    [utilization sortUsingSelector:@selector(compare:)];
    __block int drawsAfter = 0; __block double gpuAfter = 0;
    [self main:^{ drawsAfter = self.panelDraws; gpuAfter = self.panelGPUSeconds; }];
    NSMutableDictionary *row = [@{@"phase" : name, @"seconds" : @(elapsed), @"paused" : @(expectPaused), @"worldFrames" : @(frames), @"worldSubmissions" : @(submissions), @"roomFramesReceived" : @(messages),
                                  @"appCPUPercentOfOneCore" : @(cpu_percent(me, appCPU, elapsed)), @"webCPUPercentOfOneCore" : @(cpu_percent(webPID, webCPU, elapsed)), @"panelDraws" : @(drawsAfter - draws),
                                  @"panelGPUSeconds" : @(gpuAfter - gpu), @"worldPendingGPUFrames" : @(pending)} mutableCopy];
    if (utilization.count) { row[@"systemGPUUtilizationP50Percent"] = utilization[utilization.count / 2]; row[@"systemGPUUtilizationMaxPercent"] = utilization.lastObject; }
    [rows addObject:row];
    write_json(@{@"schema" : @"aethyrforge.setup-profile.v1", @"firstFrameSeconds" : @(self.firstFrameSeconds), @"webPID" : @(webPID), @"rows" : rows}, report);
    if ([name isEqualToString:@"setup"] || [name isEqualToString:@"status"]) {
      // The panel is the shared host's components (chrome.coax), not native controls.
      __block BOOL shared = NO;
      [self main:^{
        shared = self.panelVertices > 0 && self.panelDraws > 0;
        for (NSView *v in ((__bridge NSView *)ui_view(self.host)).subviews) shared &= ![v isKindOfClass:NSVisualEffectView.class];
      }];
      FORGE_REQUIRE(shared, @"Panel is not rendered by shared Zeron Metal.");
      [self capturePanel:[report.URLByDeletingLastPathComponent URLByAppendingPathComponent:[name stringByAppendingString:@".png"]]];
    }
  };
  [self main:^{ [self enterWorld]; }]; sample(@"play", NO);
  [self main:^{ [self showSetup]; }]; sample(@"setup", YES);
  [self main:^{ [self showStatus]; }]; sample(@"status", YES);
  [self main:^{ [self reloadWorld]; }];
  double reloaded = forge_uptime() + 20;
  while (!self.sceneReady && forge_uptime() < reloaded) forge_sleep_ms(100);
  if (!self.sceneReady) {
    id debug = nil;
    @try { debug = [self js:@"JSON.stringify({url:location.href,ready:coaxial?.ready,scene:coaxial?.sceneReady,loop:coaxial?.worldLoop?.(),render:coaxial?.renderStatus,errors:globalThis.__forgeErrors,loading:document.querySelector('#coaxial-load')?.textContent})"]; } @catch (NSException *x) { (void)x; }
    forge_throw([NSString stringWithFormat:@"Setup blocked the first frame after reload: %@", debug]);
  }
  sample(@"statusReloaded", YES);
  [self main:^{ [self enterWorld]; [self.window miniaturize:nil]; }]; sample(@"minimized", YES);
  [self main:^{ [self.window deminiaturize:nil]; [self.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES]; }]; sample(@"resumed", NO);
  [self main:^{ [NSApp hide:nil]; }]; sample(@"hidden", YES);
  [self main:^{ [NSApp unhide:nil]; [self.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES]; }]; sample(@"unhidden", NO);
  [self finish];
}

- (void)offlineCheck {
  NSURL *report = self.offlineReportURL;
  [self main:^{ [self enterWorld]; }];
  NSDictionary *(^snapshot)(void) = ^{
    NSDictionary *d = [self callJS:@"return JSON.parse(JSON.stringify({url:location.href,host:coaxial.roomHost || {},workers:globalThis.__forgeRoomWorkers || 0,net:coaxial.netProbe(0),seat:coaxial.netProbe(1),loop:coaxial.worldLoop()}));" arguments:nil];
    return [d isKindOfClass:NSDictionary.class] ? d : @{};
  };
  NSDictionary *state = @{};
  double deadline = forge_uptime() + 30;
  while (forge_uptime() < deadline) {
    state = snapshot();
    NSDictionary *host = state[@"host"];
    if ([host isKindOfClass:NSDictionary.class] && host[@"native"] && num(state[@"net"]) == 2 && [state[@"seat"] isEqual:@0]) break;
    forge_sleep_ms(100);
  }
  NSDictionary *host = state[@"host"];
  BOOL worker = self.offlineWorker;
  FORGE_REQUIRE([host isKindOfClass:NSDictionary.class] && [host[@"native"] isEqual:@(!worker)] && num(state[@"net"]) == 2 && [state[@"seat"] isEqual:@0] && num(state[@"workers"]) == (worker ? 1 : 0), @"Wrong offline transport: %@", state);
  NSDictionary *before = [self worldLoop];
  double start = forge_uptime();
  __block int32_t pid = 0; [self main:^{ pid = self.webProcessPID; }];
  int32_t roomPID = self.room.processIdentifier, me = getpid();
  uint64_t appCPU = forge_process_cpu_ns(me), webCPU = forge_process_cpu_ns(pid), roomCPU = forge_process_cpu_ns(roomPID);
  NSMutableArray *samples = [NSMutableArray array];
  for (int i = 0; i < 15; i++) { forge_sleep_ms(1000); [samples addObject:snapshot()]; }
  NSDictionary *after = [self worldLoop];
  double elapsed = forge_uptime() - start;
  NSInteger frames = num(after[@"frames"]) - num(before[@"frames"]), messages = num(after[@"socketFrames"]) - num(before[@"socketFrames"]);
  NSDictionary *final = [samples.lastObject[@"host"] isKindOfClass:NSDictionary.class] ? samples.lastObject[@"host"] : nil;
  NSString *failed = [final[@"failed"] isKindOfClass:NSString.class] ? final[@"failed"] : @"";
  FORGE_REQUIRE(frames > 100 && messages > 100 && final && [final[@"tickP50Us"] doubleValue] > 0 && !failed.length, @"Offline room did not simulate and deliver: %@", samples.lastObject ?: @{});
  write_json(@{@"schema" : @"aethyrforge.offline.v1", @"transport" : worker ? @"wasm-worker" : @"native-room", @"firstFrameSeconds" : @(self.firstFrameSeconds), @"seconds" : @(elapsed),
               @"worldFrames" : @(frames), @"roomFramesReceived" : @(messages), @"workersCreated" : state[@"workers"] ?: @-1, @"roomCPUPercentOfOneCore" : @(cpu_percent(roomPID, roomCPU, elapsed)),
               @"appCPUPercentOfOneCore" : @(cpu_percent(me, appCPU, elapsed)), @"webCPUPercentOfOneCore" : @(cpu_percent(pid, webCPU, elapsed)), @"samples" : samples},
             report);
  [self finish];
}

// The authored preview workload through the real menu command; raw platform facts for the Coax receipt.
- (void)measureWebPreview:(NSURL *)planURL {
  NSDictionary *plan = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:planURL] ?: [NSData data] options:0 error:nil];
  FORGE_REQUIRE([plan isKindOfClass:NSDictionary.class], @"Invalid preview plan.");
  NSMutableDictionary *samples = [NSMutableDictionary dictionary];
  NSMutableArray *menus = [NSMutableArray array];
  NSString *originalURL = self.pageURL.absoluteString;
  for (NSDictionary *operation in list(plan, @"operations")) {
    NSString *op = operation[@"op"];
    if ([op isEqualToString:@"resize"]) {
      [self main:^{ self.window.minSize = NSMakeSize(640, 360); [self.window setContentSize:NSMakeSize(640, 360)]; }];
    } else if ([op isEqualToString:@"connect"]) {
      [self main:^{ void *h = self.metalWorld.host; if (h && mf_info(h, 7) != 2) { mf_key(h, -2, true, 0); mf_key(h, -2, false, 0); } }];
      if (!self.metalWorld) [self js:@"if(coaxial.netProbe(0)!==2){dispatchEvent(new KeyboardEvent('keydown',{key:'Enter',bubbles:true}));dispatchEvent(new KeyboardEvent('keyup',{key:'Enter',bubbles:true}));} true"];
      double end = forge_uptime() + 20;
      while (!self.stopping) {
        __block BOOL native = NO, connected = NO;
        [self main:^{ void *h = self.metalWorld.host; native = h != NULL; if (h) connected = mf_info(h, 7) == 2; }];
        if (!native) connected = yes([self js:@"coaxial.netProbe(0)===2"]);
        if (connected) break;
        FORGE_REQUIRE(forge_uptime() < end, @"Preview room did not connect.");
        forge_sleep_ms(50);
      }
    } else if ([op isEqualToString:@"menu"]) {
      // The View menu's preview item, as AppKit validates and sends it (ui_menus' command_state, then command).
      __block BOOL enabled = NO, sent = NO, disabled = NO; __block NSString *title = nil;
      [self main:^{
        title = [self previewTitle];
        enabled = self.previewTarget >= 0;
        if (enabled) { [self togglePreview]; sent = self.switching; }
        disabled = self.previewTarget < 0;
      }];
      for (;;) { __block BOOL busy = NO; [self main:^{ busy = self.switching; }]; if (!busy) break; forge_sleep_ms(20); }
      [menus addObject:@{@"title" : title ?: @"", @"enabled" : @(enabled), @"sent" : @(sent), @"busyDisabled" : @(disabled)}];
    } else if ([op isEqualToString:@"observe"]) {
      NSString *label = operation[@"label"];
      __block BOOL native = NO; [self main:^{ native = self.metalWorld.host != NULL; }];
      if (native) {
        NSDictionary *auxiliary = forge_parse([self js:@"JSON.stringify({url:location.href,voice:!!globalThis.coaxial?.voice,world:typeof globalThis.coaxial?.netProbe!=='undefined',canvas:document.querySelectorAll('canvas').length,closed:location.pathname==='/metal-voice.html' && !!globalThis.coaxial?.voice && typeof globalThis.coaxial?.netProbe==='undefined' && document.querySelectorAll('canvas').length===0})"]) ?: @{};
        [self main:^{
          void *h = self.metalWorld.host;
          samples[label] = @{@"frames" : @(mf_info(h, 0)), @"completed" : @(mf_info(h, 10)), @"net" : @(mf_info(h, 7)), @"seat" : @(mf_info(h, 8)), @"audio" : [self.metalWorld audioObservations],
                             @"auxiliary" : auxiliary, @"auxiliaryClosed" : auxiliary[@"closed"] ?: @NO, @"sameURL" : @([self.pageURL.absoluteString isEqualToString:originalURL])};
        }];
      } else {
        NSString *raw = [self callJS:@"let completed=false;if(coaxial.hostFrameAt){await coaxial.hostFrameAt(performance.now()*1000);completed=true;}return JSON.stringify({ready:coaxial.ready,scene:coaxial.sceneReady,net:coaxial.netProbe(0),seat:coaxial.netProbe(1),loop:coaxial.worldLoop(),completed,url:location.href});" arguments:nil];
        samples[label] = forge_parse(raw) ?: @{};
      }
    } else forge_throw(@"Unknown preview SDK operation.");
    if (self.stopping) break;
  }
  NSDictionary *beforeRow = samples[@"before"] ?: @{}, *shown = samples[@"web"] ?: @{}, *returned = samples[@"returned"] ?: @{};
  NSDictionary *retired = self.lastPreviewRetired ?: @{};
  NSDictionary *oldAudio = [retired[@"audio"] isKindOfClass:NSDictionary.class] ? retired[@"audio"] : @{}, *newAudio = [returned[@"audio"] isKindOfClass:NSDictionary.class] ? returned[@"audio"] : @{};
  __block NSDictionary *closed = @{};
  [self main:^{
    ForgeMetalWorld *source = self.metalWorld;
    [self stopChildren];
    closed = source ? source.closedAudio : @{};
  }];
  int64_t (^number)(NSDictionary *, NSString *) = ^int64_t(NSDictionary *row, NSString *key) { id v = row[key]; return [v isKindOfClass:NSNumber.class] ? [v longLongValue] : -1; };
  NSDictionary *first = menus.firstObject ?: @{}, *back = menus.lastObject ?: @{};
  NSArray *observed = @[
    @(self.worldBackend), @(number(beforeRow, @"frames")), @(number(beforeRow, @"net")), @(number(beforeRow, @"seat")), @(number(first, @"sent")), @(number(first, @"busyDisabled")),
    @(number(retired, @"ledgerLive")), @(number(retired, @"ledgerWords")), @(number(retired, @"mediaBytes")), @(number(oldAudio, @"retired")), @(number(shown, @"ready")), @(number(shown, @"scene")),
    @(number(shown, @"net")), @(number(shown, @"seat")), @(number(shown, @"completed")), @(number(back, @"sent")), @(number(returned, @"frames")), @(number(returned, @"net")), @(number(returned, @"seat")),
    @(number(returned, @"auxiliaryClosed")), @(number(newAudio, @"started")), @(mf_closed_info(0)), @(mf_closed_info(1)), @(mf_closed_info(2)), @(number(closed, @"retired")), @(number(closed, @"active"))
  ];
  write_json(@{@"schema" : @"aethyrforge.web-preview.v1", @"observed" : observed, @"menu" : first[@"title"] ?: @"", @"back" : back[@"title"] ?: @"", @"samples" : samples, @"menus" : menus,
               @"retired" : retired, @"closedAudio" : closed, @"originalURL" : originalURL ?: @""},
             [NSURL fileURLWithPath:plan[@"report"]]);
  [self terminate];
}

// Only SDK effects from the Coax-authored native measurement plan; Coax reduces the raw clocks and decides.
- (void)measureMetal:(NSURL *)planURL {
  __block ForgeMetalWorld *view = nil; __block void *host = NULL;
  [self main:^{ view = self.metalWorld; host = view.host; }];
  NSDictionary *plan = [NSJSONSerialization JSONObjectWithData:[NSData dataWithContentsOfURL:planURL] ?: [NSData data] options:0 error:nil];
  FORGE_REQUIRE(view && host && [plan isKindOfClass:NSDictionary.class] && [plan[@"schema"] isEqual:@"aethyr.metal.window-plan.v1"] && [plan[@"report"] isKindOfClass:NSString.class], @"Invalid native measurement plan.");
  NSMutableArray *observations = [NSMutableArray array], *retirements = [NSMutableArray array];
  double started = forge_uptime();
  int32_t me = getpid();
  uint64_t cpu = forge_process_cpu_ns(me);
  for (NSDictionary *op in list(plan, @"operations")) {
    __block BOOL same = NO; [self main:^{ same = view.host == host; }];
    if (self.stopping || !same) {
      __block NSString *notice = nil; [self main:^{ notice = self.notice.stringValue; }];
      [observations addObject:@{@"label" : @"host-stopped", @"error" : notice ?: @"", @"stopping" : @(self.stopping), @"sameHost" : @(same)}];
      break;
    }
    NSString *kind = op[@"op"];
    if ([kind isEqualToString:@"wait"]) { forge_sleep_ms([op[@"ms"] intValue]); continue; }
    if ([kind isEqualToString:@"reload"]) {
      __block NSDictionary *audio = nil;
      [self main:^{ [self reloadWorld]; audio = view.closedAudio; }];
      [retirements addObject:@{@"ledgerLive" : @(mf_closed_info(0)), @"ledgerWords" : @(mf_closed_info(1)), @"mediaBytes" : @(mf_closed_info(2)), @"audio" : audio ?: @{}}];
      __block void *replacement = NULL; [self main:^{ replacement = view.host; }];
      FORGE_REQUIRE(replacement, @"Native reload did not open a replacement host.");
      host = replacement;
      [self waitForScene];
      continue;
    }
    if ([kind isEqualToString:@"jpeg"]) {
      __block NSData *image = nil;
      [self main:^{ size_t count = 0; const uint8_t *bytes = mf_frame_jpeg(host, &count); if (bytes) image = [[NSData alloc] initWithBase64EncodedData:[NSData dataWithBytes:bytes length:count] options:0]; }];
      FORGE_REQUIRE(image.length, @"Native display JPEG failed.");
      [image writeToFile:op[@"file"] atomically:YES];
      continue;
    }
    __block NSString *problem = nil;
    [self main:^{
      if ([kind isEqualToString:@"raise"]) { [NSApp unhide:nil]; [self.window deminiaturize:nil]; [self.window makeKeyAndOrderFront:nil]; [NSApp activateIgnoringOtherApps:YES]; }
      else if ([kind isEqualToString:@"reset"]) mf_measure_reset(host);
      else if ([kind isEqualToString:@"audio"]) [view startAudio];
      else if ([kind isEqualToString:@"presentation"]) [view setPresentationHz:[op[@"hz"] floatValue]];
      else if ([kind isEqualToString:@"key"]) mf_key(host, [op[@"key"] intValue], [op[@"down"] boolValue], 0);
      else if ([kind isEqualToString:@"status"]) [self showStatus];
      else if ([kind isEqualToString:@"world"]) [self enterWorld];
      else if ([kind isEqualToString:@"focus"]) [view focus:[op[@"focused"] boolValue]];
      else if ([kind isEqualToString:@"recover"]) { if (!mf_recover(host)) problem = @"Native device recovery failed."; }
      else if ([kind isEqualToString:@"resize"]) [self.window setContentSize:NSMakeSize([op[@"width"] doubleValue], [op[@"height"] doubleValue])];
      else if ([kind isEqualToString:@"snapshot"]) { if (!mf_snapshot(host, [op[@"file"] UTF8String])) problem = @"Native snapshot failed."; }
      else if ([kind isEqualToString:@"presented"]) { if (!mf_snapshot_presented(host, [op[@"file"] UTF8String])) problem = @"Native display readback failed."; }
      else if ([kind isEqualToString:@"observe"]) {
        size_t count = 0, dropped = 0;
        const MFFrameObservation *r = mf_measure_rows(host, &count, &dropped);
        NSMutableArray *rows = [NSMutableArray array];
        for (size_t i = 0; r && i < count; i++)
          [rows addObject:@[ @(r[i].frame), @(r[i].start_ns), @(r[i].submit_ns), @(r[i].complete_ns), @(r[i].gpu_ns), @(r[i].input_sequence), @(r[i].input_ns), @(r[i].x_mm), @(r[i].z_mm), @(r[i].width), @(r[i].height), @(r[i].scale) ]];
        NSMutableArray *face = [NSMutableArray array];
        for (int i = 0; i < 11; i++) [face addObject:@(mf_observe(host, -2, i))];
        [observations addObject:@{@"label" : op[@"label"] ?: @"", @"rows" : rows, @"dropped" : @(dropped), @"frames" : @(mf_info(host, 0)), @"completed" : @(mf_info(host, 10)), @"pending" : @(mf_info(host, 2)),
                                  @"net" : @(mf_info(host, 7)), @"seat" : @(mf_info(host, 8)), @"messages" : @(view.delivered), @"paused" : @(view.paused), @"setup" : @(!self.setupHidden), @"ledgerLive" : @(mf_info(host, 3)),
                                  @"recoveries" : @(mf_info(host, 12)), @"width" : @(view.bounds.size.width), @"height" : @(view.bounds.size.height), @"backingScale" : @(self.window.backingScaleFactor),
                                  @"audio" : [view audioObservations], @"nativeFace" : face, @"windowVisible" : @(self.window.visible), @"occlusionVisible" : @((self.window.occlusionState & NSWindowOcclusionStateVisible) != 0),
                                  @"onActiveSpace" : @(self.window.onActiveSpace), @"appHidden" : @(NSApp.hidden), @"appActive" : @(NSApp.active)}];
      } else problem = @"Unknown native measurement effect.";
    }];
    if (problem) forge_throw(problem);
  }
  double seconds = forge_uptime() - started;
  __block NSMutableDictionary *result = nil;
  [self main:^{
    result = [@{@"schema" : @"aethyr.metal.window-result.v1", @"observations" : observations, @"seconds" : @(seconds), @"cpuPercentOfOneCore" : @(cpu_percent(me, cpu, seconds)),
                @"residentBytes" : @(forge_process_resident_bytes(me)), @"displayFPS" : @(self.window.screen.maximumFramesPerSecond), @"preferredFPS" : @(view.preferredHz),
                @"visible" : @(self.window.visible && (self.window.occlusionState & NSWindowOcclusionStateVisible)), @"thermal" : @(NSProcessInfo.processInfo.thermalState),
                @"lowPower" : @(NSProcessInfo.processInfo.lowPowerModeEnabled), @"firstFrameSeconds" : @(self.firstFrameSeconds)} mutableCopy];
    [view close];
    result[@"closedLedgerLive"] = @(mf_closed_info(0)); result[@"closedLedgerWords"] = @(mf_closed_info(1)); result[@"closedMediaBytes"] = @(mf_closed_info(2));
    result[@"retirements"] = retirements; result[@"closedAudio"] = view.closedAudio ?: @{};
  }];
  write_json(result, [NSURL fileURLWithPath:plan[@"report"]]);
  [self finish];
}
@end
