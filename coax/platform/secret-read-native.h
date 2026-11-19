/* Shared platform credential read. Callers choose service/account in Coax;
   this file only calls the OS. A headless reader can prohibit UI entirely. */
#ifndef COAX_SECRET_READ_NATIVE_H
#define COAX_SECRET_READ_NATIVE_H
#include <stdint.h>
#include <string.h>
#if defined(__APPLE__)
#include <Security/Security.h>
#endif
static inline int64_t cxsecret_copy(const char *service,const char *account,int no_ui,
                                   unsigned char *out,int64_t capacity) {
#if defined(__APPLE__)
  CFMutableDictionaryRef q=CFDictionaryCreateMutable(NULL,0,&kCFTypeDictionaryKeyCallBacks,&kCFTypeDictionaryValueCallBacks);
  if (!q) return -1;
  CFStringRef s=CFStringCreateWithCString(NULL,service,kCFStringEncodingUTF8);
  if (!s) {CFRelease(q);return -1;}
  CFDictionarySetValue(q,kSecClass,kSecClassGenericPassword);
  CFDictionarySetValue(q,kSecAttrService,s);CFRelease(s);
  if (account) {CFStringRef a=CFStringCreateWithCString(NULL,account,kCFStringEncodingUTF8);if (!a) {CFRelease(q);return -1;}CFDictionarySetValue(q,kSecAttrAccount,a);CFRelease(a);}
  CFDictionarySetValue(q,kSecReturnData,kCFBooleanTrue);
  CFDictionarySetValue(q,kSecMatchLimit,kSecMatchLimitOne);
  /* Deprecated but still honoured; tools built with -Werror include this. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
  if (no_ui) CFDictionarySetValue(q,kSecUseAuthenticationUI,kSecUseAuthenticationUIFail);
#pragma clang diagnostic pop
  CFTypeRef got=NULL;OSStatus status=SecItemCopyMatching(q,&got);CFRelease(q);
  if (status==errSecItemNotFound) return 0;
  if (status!=errSecSuccess || !got) {if (got) CFRelease(got);return -3;}
  if (CFGetTypeID(got)!=CFDataGetTypeID()) {CFRelease(got);return -1;}
  CFIndex n=CFDataGetLength((CFDataRef)got);int64_t answer=-1;
  if (n>0 && n<=capacity) {memcpy(out,CFDataGetBytePtr((CFDataRef)got),(size_t)n);answer=n;}
  CFRelease(got);return answer;
#else
  (void)service;(void)account;(void)no_ui;(void)out;(void)capacity;return -2;
#endif
}
#endif
