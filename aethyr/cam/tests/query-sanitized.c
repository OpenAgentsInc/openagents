// Thin SDK projection: the paid loop, state and arithmetic live in Coax.
#include <stdint.h>
#include <stdio.h>
int32_t camera_trial(int32_t fps);
int64_t camera_word(int32_t row,int32_t word);
int main(void) {
  const int32_t cadences[] = {30,60,144,0};
  for (unsigned i=0;i<4;i++) {
    if (camera_trial(cadences[i])!=64) return 1;
    for (int32_t row=0;row<64;row++) {
      printf("%d %d",cadences[i],row);
      for (int32_t word=0;word<28;word++) printf(" %lld",(long long)camera_word(row,word));
      putchar('\n');
    }
  }
  return 0;
}
