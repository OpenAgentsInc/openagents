/* Scalar, byte-preserving bridge between separately compiled Coax units.
   Opcodes only select ABI entries; protocol decisions live in Coax. */
#pragma once
#include <stdint.h>
#include <string.h>
#define HOME_INPUT_CAP 1048576
extern unsigned char nl_inputs[3][HOME_INPUT_CAP],nl_output_bytes[4194304];
extern int64_t nl_output_count;
extern int32_t filter(int32_t,int32_t,int32_t),begin(int32_t),request(void),close_query(void),frame(int32_t),state(void);
extern int32_t fold_begin(int32_t,int32_t,int32_t),fold_feed(int32_t),fold_result(void),budget(int64_t,int64_t),npub(int32_t);
extern int64_t home_fold_ids(int64_t),home_fold_known(void),home_watch_request(int64_t,int64_t,int64_t),home_watch_message(int64_t,int64_t,int64_t);
static int64_t home_lengths[3];
__attribute__((import_module("env"),import_name("coaxial_now_seconds"))) int64_t coaxial_now_seconds(void);
int64_t home_wall(void) {return coaxial_now_seconds();}
int64_t home_clear(int64_t slot) {if(slot<0||slot>2)return 0;home_lengths[slot]=0;return 1;}
int64_t home_push(int64_t slot,int64_t byte) {if(slot<0||slot>2||home_lengths[slot]>=HOME_INPUT_CAP||byte<0||byte>255)return 0;nl_inputs[slot][home_lengths[slot]++]=(unsigned char)byte;return 1;}
int64_t home_output_size(void) {return nl_output_count;}
int64_t home_output_byte(int64_t i) {return i>=0&&i<nl_output_count?nl_output_bytes[i]:-1;}
int64_t home_copy_output(int64_t slot) {if(slot<0||slot>2||nl_output_count<0||nl_output_count>HOME_INPUT_CAP)return 0;memcpy(nl_inputs[slot],nl_output_bytes,(size_t)nl_output_count);home_lengths[slot]=nl_output_count;return 1;}
int64_t home_call(int64_t op,int64_t a,int64_t b,int64_t now) {
 switch(op){
 case 0:return filter((int32_t)a,(int32_t)home_lengths[0],(int32_t)home_lengths[1]);
 case 1:return begin((int32_t)home_lengths[0]);case 2:return request();case 3:return close_query();
 case 4:return frame((int32_t)home_lengths[0]);case 5:return state();
 case 6:return fold_begin((int32_t)home_lengths[0],(int32_t)home_lengths[1],(int32_t)home_lengths[2]);
 case 7:return fold_feed((int32_t)home_lengths[0]);case 8:return fold_result();
 case 9:return home_fold_ids(a);case 10:return home_fold_known();
 case 11:return home_watch_request(a,home_lengths[1],now);
 case 12:return home_watch_message(home_lengths[0],home_lengths[1],now);
 case 13:return budget(a,b);
 case 14:return npub((int32_t)home_lengths[0]);
 default:return -1;
 }
}
