// Firm reason: 6 (the Mac's microphone permission and inputs; dictation-mac.m).
#ifndef OA_DICTATION_MAC_H
#define OA_DICTATION_MAC_H
typedef struct { char uid[256], name[128]; } DmInput;
int dm_permission(void);                // 0 granted, 1 refused, 2 being asked (the answer comes later)
int dm_inputs(DmInput *out, int cap);   // the inputs there are
int dm_selected(char *out, int cap);    // the chosen input's UID; empty: the system's default
int dm_select(const char *uid);         // 0 when that input is not there (nothing changes)
int dm_open(void);                      // capture from the chosen input: 0, or 3 when it could not start
#endif
