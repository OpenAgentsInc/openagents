/* Independent native boundary oracle: invalid foreign descriptors never select
 * a device or dereference storage; no microphone or model is used. */
#include "../dictation-face.h"
#include <assert.h>
#include <stdio.h>
int main(void) {
  const uint8_t uid[] = "usb";
  assert(dct_device_matches(uid,3,uid,3,0));
  assert(!dct_device_matches(uid,-1,uid,3,0));
  assert(!dct_device_matches(uid,3,uid,-1,0));
  assert(!dct_device_matches(NULL,1,uid,3,1));
  assert(!dct_device_matches(uid,3,NULL,1,0));
  assert(!dct_device_matches(NULL,0,NULL,0,1));
  assert(dct_device_matches(NULL,0,uid,3,1));
  assert(!dct_device_matches(NULL,0,uid,3,0));
  assert(dct_device_format(48000,2,2,1));
  assert(dct_device_format(44100,2,1,1));
  assert(!dct_device_format(96000,2,2,1));
  assert(!dct_device_format(16000.5,1,1,1));
  assert(!dct_device_format(16000,2,3,1));
  assert(!dct_device_format(16000,2,2,0));
  puts("Dictation device native boundary: 14 cases PASS");
  return 0;
}
