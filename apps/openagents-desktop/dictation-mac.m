// Firm reason: 6 (AVFoundation's microphone permission, CoreAudio's devices and the user's defaults; every decision is Coax).
// The Mac's part of dictation for OpenAgents (linux/dictation-host.c, GitHub #13156): what
// Sources/Dictation.swift's MicrophoneDictationCapture was. The microphone's permission is asked
// without blocking the window; the input is the one chosen in Dictation Input… (kept, as before,
// under the defaults key OpenAgentsLocalDictationInputUID) or the system's default; capture is
// apps/openagents-cli/dictate-mac.m's AudioQueue, read like the Linux recorder's pipe.
#import <AVFoundation/AVFoundation.h>
#include <CoreAudio/CoreAudio.h>
#include <stdatomic.h>
#include <string.h>
#include "dictation.h"
#include "dictation-face.h"
#include "dictation-mac.h"

int64_t cxdict_mac_devices(void);
int64_t cxdict_mac_device_uid(int64_t i, coax_v_u8 out);
int64_t cxdict_mac_default(void);
int64_t cxdict_mac_open(int64_t i);

static NSString *const key = @"OpenAgentsLocalDictationInputUID";
static atomic_int asking;

int dm_permission(void) {
  AVAuthorizationStatus s = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
  if (s == AVAuthorizationStatusAuthorized) return 0;
  if (s != AVAuthorizationStatusNotDetermined) return 1;
  if (!atomic_exchange(&asking, 1))
    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL ok) { (void)ok; atomic_store(&asking, 0); }];
  return 2;
}
static int matches(const char *wanted, const char *observed, int is_default) {
  return dct_device_matches((const uint8_t *)wanted, (int32_t)strlen(wanted), (const uint8_t *)observed, (int32_t)strlen(observed), is_default) != 0;
}
static NSString *device_name(const char *uid) {
  CFStringRef u = CFStringCreateWithCString(NULL, uid, kCFStringEncodingUTF8);
  AudioDeviceID d = 0; UInt32 size = sizeof d;
  AudioValueTranslation t = {&u, sizeof u, &d, sizeof d};
  AudioObjectPropertyAddress a = {kAudioHardwarePropertyDeviceForUID, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
  size = sizeof t;
  OSStatus st = AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, &t);
  CFRelease(u);
  if (st != noErr || !d) return nil;
  CFStringRef name = NULL; size = sizeof name;
  AudioObjectPropertyAddress n = {kAudioObjectPropertyName, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
  if (AudioObjectGetPropertyData(d, &n, 0, NULL, &size, &name) != noErr || !name) return nil;
  return (__bridge_transfer NSString *)name;
}
int dm_inputs(DmInput *out, int cap) {
  int n = (int)cxdict_mac_devices(), k = 0;
  for (int i = 0; i < n && k < cap; i++) {
    uint8_t uid[256];
    int64_t m = cxdict_mac_device_uid(i, (coax_v_u8){uid, sizeof uid - 1});
    if (m <= 0) continue;
    uid[m] = 0;
    if (!matches((const char *)uid, (const char *)uid, 0)) continue;
    NSString *name = device_name((const char *)uid);
    snprintf(out[k].uid, sizeof out[k].uid, "%s", (const char *)uid);
    snprintf(out[k].name, sizeof out[k].name, "%s", name ? name.UTF8String : (const char *)uid);
    k++;
  }
  return k;
}
int dm_selected(char *out, int cap) {
  NSString *s = [NSUserDefaults.standardUserDefaults stringForKey:key];
  snprintf(out, (size_t)cap, "%s", s ? s.UTF8String : "");
  return (int)strlen(out);
}
int dm_select(const char *uid) {
  // An empty choice follows the system's default; an input that is not there is never kept.
  if (*uid) {
    DmInput all[32]; int n = dm_inputs(all, 32), found = 0;
    for (int i = 0; i < n; i++) if (matches(uid, all[i].uid, 0)) found = 1;
    if (!found) return 0;
  }
  [NSUserDefaults.standardUserDefaults setObject:[NSString stringWithUTF8String:uid] forKey:key];
  return 1;
}
int dm_open(void) {
  char wanted[256]; dm_selected(wanted, sizeof wanted);
  int n = (int)cxdict_mac_devices(), def = (int)cxdict_mac_default();
  for (int i = 0; i < n; i++) {
    uint8_t uid[256];
    int64_t m = cxdict_mac_device_uid(i, (coax_v_u8){uid, sizeof uid - 1});
    if (m <= 0) continue;
    uid[m] = 0;
    if (matches(wanted, (const char *)uid, i == def)) return (int)cxdict_mac_open(i);
  }
  return 3;
}
