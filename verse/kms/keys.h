// The realm key for an evdev key code (linux/input.h), 0 for none: what
// verse/kms reads from /dev/input and what a Wayland seat sends.
#ifndef VERSE_KMS_KEYS_H
#define VERSE_KMS_KEYS_H
#include "realm.h"
#include <linux/input-event-codes.h>
#include <stddef.h>

static inline int realm_key_of(int code) {
  switch (code) {
    case KEY_ESC: return REALM_ESCAPE;
    case KEY_ENTER: case KEY_KPENTER: return REALM_ENTER;
    case KEY_BACKSPACE: return REALM_BACKSPACE;
    case KEY_LEFT: return REALM_LEFT;
    case KEY_RIGHT: return REALM_RIGHT;
    case KEY_UP: return REALM_UP;
    case KEY_DOWN: return REALM_DOWN;
    case KEY_SPACE: return ' ';
  }
  static const struct { int code; char c; } letters[] = {
    {KEY_A,'a'},{KEY_B,'b'},{KEY_C,'c'},{KEY_D,'d'},{KEY_E,'e'},{KEY_F,'f'},{KEY_G,'g'},{KEY_H,'h'},{KEY_I,'i'},
    {KEY_J,'j'},{KEY_K,'k'},{KEY_L,'l'},{KEY_M,'m'},{KEY_N,'n'},{KEY_O,'o'},{KEY_P,'p'},{KEY_Q,'q'},{KEY_R,'r'},
    {KEY_S,'s'},{KEY_T,'t'},{KEY_U,'u'},{KEY_V,'v'},{KEY_W,'w'},{KEY_X,'x'},{KEY_Y,'y'},{KEY_Z,'z'},
    {KEY_1,'1'},{KEY_2,'2'},{KEY_3,'3'},{KEY_4,'4'},{KEY_5,'5'},{KEY_6,'6'},{KEY_7,'7'},{KEY_8,'8'},{KEY_9,'9'},{KEY_0,'0'}};
  for (size_t i = 0; i < sizeof letters / sizeof letters[0]; i++)
    if (letters[i].code == code) return letters[i].c;
  return 0;
}
#endif
