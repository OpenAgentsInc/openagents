/* CoreAudio capture effects for `openagents dictate` on macOS (GitHub #12667).
   Effects only: Coax (dictate.coax) chooses the device by exact UID through
   the shared dci policy, and owns timing, silence and recognition.

   - cxdict_mac_permission: the microphone's TCC state; asks once when it
     is undetermined (macOS attributes it to the terminal app).
   - cxdict_mac_devices / _device_uid / _default: input devices and the
     system default's index.
   - cxdict_mac_open: an AudioQueue on that device, mono f32 at 48 kHz
     (CoreAudio converts), samples kept in memory only. A device-list or
     default-input change fails the capture; nothing switches devices.
   - cxdict_mac_read / _rate / _close / _active: dictate-native.c reads it
     like the Linux recorder pipe. */
#import <AVFoundation/AVFoundation.h>
#include <AudioToolbox/AudioToolbox.h>
#include <CoreAudio/CoreAudio.h>
#include <pthread.h>
#include <stdint.h>
#include <string.h>
#include <time.h>
#include "dictation.h"
#include "entropy-foreign.h"

enum { DEVICES = 64, RATE = 48000, RING = RATE * 4, FRAMES = 960, BUFFERS = 3 };
static AudioDeviceID ids[DEVICES]; static int id_n;
static AudioQueueRef queue; static AudioQueueBufferRef buffers[BUFFERS];
static pthread_mutex_t ring_lock = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t ring_ready = PTHREAD_COND_INITIALIZER;
static float ring[RING]; static int64_t ring_head, ring_tail; static int failed, active;

int64_t cxdict_mac_permission(void) {
  AVAuthorizationStatus s = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
  if (s == AVAuthorizationStatusAuthorized) return 0;
  if (s != AVAuthorizationStatusNotDetermined) return 1;
  dispatch_semaphore_t done = dispatch_semaphore_create(0);
  __block BOOL granted = NO;
  [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL ok) { granted = ok; dispatch_semaphore_signal(done); }];
  if (dispatch_semaphore_wait(done, dispatch_time(DISPATCH_TIME_NOW, 120 * NSEC_PER_SEC)) != 0) return 1;
  return granted ? 0 : 1;
}

static int input_channels(AudioDeviceID d) {
  AudioObjectPropertyAddress a = {kAudioDevicePropertyStreamConfiguration, kAudioObjectPropertyScopeInput, kAudioObjectPropertyElementMain};
  UInt32 size = 0;
  if (AudioObjectGetPropertyDataSize(d, &a, 0, NULL, &size) != noErr || size == 0 || size > 65536) return 0;
  AudioBufferList *list = malloc(size); if (!list) return 0;
  int n = 0;
  if (AudioObjectGetPropertyData(d, &a, 0, NULL, &size, list) == noErr) for (UInt32 i = 0; i < list->mNumberBuffers; i++) n += (int)list->mBuffers[i].mNumberChannels;
  free(list); return n;
}
int64_t cxdict_mac_devices(void) {
  AudioObjectPropertyAddress a = {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
  AudioDeviceID all[DEVICES]; UInt32 size = sizeof all;
  id_n = 0;
  if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, all) != noErr) return 0;
  for (UInt32 i = 0; i < size / sizeof(AudioDeviceID); i++) if (input_channels(all[i]) > 0 && id_n < DEVICES) ids[id_n++] = all[i];
  return id_n;
}
static CFStringRef copy_uid(AudioDeviceID d) {
  AudioObjectPropertyAddress a = {kAudioDevicePropertyDeviceUID, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
  CFStringRef uid = NULL; UInt32 size = sizeof uid;
  if (AudioObjectGetPropertyData(d, &a, 0, NULL, &size, &uid) != noErr) return NULL;
  return uid;
}
int64_t cxdict_mac_device_uid(int64_t i, coax_v_u8 out) {
  if (i < 0 || i >= id_n) return -1;
  CFStringRef uid = copy_uid(ids[i]); if (!uid) return -1;
  char text[1024]; Boolean ok = CFStringGetCString(uid, text, sizeof text, kCFStringEncodingUTF8); CFRelease(uid);
  if (!ok) return -1;
  int64_t n = (int64_t)strlen(text); if (n > out.count) return -1;
  memcpy(out.data, text, (size_t)n); return n;
}
int64_t cxdict_mac_default(void) {
  AudioObjectPropertyAddress a = {kAudioHardwarePropertyDefaultInputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain};
  AudioDeviceID d = 0; UInt32 size = sizeof d;
  if (AudioObjectGetPropertyData(kAudioObjectSystemObject, &a, 0, NULL, &size, &d) != noErr) return -1;
  for (int i = 0; i < id_n; i++) if (ids[i] == d) return i;
  return -1;
}

static void fail_capture(void) { pthread_mutex_lock(&ring_lock); failed = 1; pthread_cond_broadcast(&ring_ready); pthread_mutex_unlock(&ring_lock); }
static OSStatus route_changed(AudioObjectID o, UInt32 n, const AudioObjectPropertyAddress *a, void *p) { (void)o; (void)n; (void)a; (void)p; fail_capture(); return noErr; }
static const AudioObjectPropertyAddress watched[2] = {
  {kAudioHardwarePropertyDevices, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain},
  {kAudioHardwarePropertyDefaultInputDevice, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain}};
static void on_input(void *p, AudioQueueRef q, AudioQueueBufferRef b, const AudioTimeStamp *t, UInt32 n, const AudioStreamPacketDescription *d) {
  (void)p; (void)t; (void)n; (void)d;
  const float *x = b->mAudioData; int64_t count = b->mAudioDataByteSize / sizeof(float);
  pthread_mutex_lock(&ring_lock);
  for (int64_t i = 0; i < count; i++) {
    if (ring_head - ring_tail >= RING) { failed = 1; break; }   /* the reader fell 4 s behind */
    ring[ring_head % RING] = x[i]; ring_head++;
  }
  pthread_cond_broadcast(&ring_ready);
  pthread_mutex_unlock(&ring_lock);
  if (active) AudioQueueEnqueueBuffer(q, b, 0, NULL);
}
int64_t cxdict_mac_close(void);
/* 0 capturing; 3 the device could not be opened. */
int64_t cxdict_mac_open(int64_t i) {
  if (i < 0 || i >= id_n || queue) return 3;
  AudioStreamBasicDescription f = {0};
  f.mSampleRate = RATE; f.mFormatID = kAudioFormatLinearPCM; f.mFormatFlags = kAudioFormatFlagIsFloat | kAudioFormatFlagIsPacked;
  f.mChannelsPerFrame = 1; f.mBitsPerChannel = 32; f.mBytesPerFrame = 4; f.mFramesPerPacket = 1; f.mBytesPerPacket = 4;
  ring_head = ring_tail = 0; failed = 0;
  if (AudioQueueNewInput(&f, on_input, NULL, NULL, NULL, 0, &queue) != noErr) { queue = NULL; return 3; }
  CFStringRef uid = copy_uid(ids[i]);
  OSStatus st = uid ? AudioQueueSetProperty(queue, kAudioQueueProperty_CurrentDevice, &uid, sizeof uid) : -1;
  if (uid) CFRelease(uid);
  if (st != noErr) { cxdict_mac_close(); return 3; }
  for (int k = 0; k < 2; k++) if (AudioObjectAddPropertyListener(kAudioObjectSystemObject, &watched[k], route_changed, NULL) != noErr) { cxdict_mac_close(); return 3; }
  for (int k = 0; k < BUFFERS; k++) {
    if (AudioQueueAllocateBuffer(queue, FRAMES * 4, &buffers[k]) != noErr || AudioQueueEnqueueBuffer(queue, buffers[k], 0, NULL) != noErr) { cxdict_mac_close(); return 3; }
  }
  active = 1;
  if (AudioQueueStart(queue, NULL) != noErr) { cxdict_mac_close(); return 3; }
  return 0;
}
int64_t cxdict_mac_read(coax_v_f32 out, int64_t timeout_ms) {
  pthread_mutex_lock(&ring_lock);
  if (ring_head == ring_tail && !failed && timeout_ms > 0) {
    struct timespec until; clock_gettime(CLOCK_REALTIME, &until);
    until.tv_nsec += timeout_ms * 1000000; until.tv_sec += until.tv_nsec / 1000000000; until.tv_nsec %= 1000000000;
    pthread_cond_timedwait(&ring_ready, &ring_lock, &until);
  }
  int64_t n = 0;
  if (failed) n = -1;
  else while (ring_tail < ring_head && n < out.count) { out.data[n++] = ring[ring_tail % RING]; ring_tail++; }
  pthread_mutex_unlock(&ring_lock);
  return n;
}
int64_t cxdict_mac_rate(void) { return RATE; }
int64_t cxdict_mac_active(void) { return queue != NULL; }
int64_t cxdict_mac_close(void) {
  active = 0;
  for (int k = 0; k < 2; k++) AudioObjectRemovePropertyListener(kAudioObjectSystemObject, &watched[k], route_changed, NULL);
  if (queue) { AudioQueueStop(queue, true); AudioQueueDispose(queue, true); queue = NULL; }
  pthread_mutex_lock(&ring_lock); memset(ring, 0, sizeof ring); ring_head = ring_tail = 0; pthread_mutex_unlock(&ring_lock);
  return 0;
}
