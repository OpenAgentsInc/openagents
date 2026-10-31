// Thin native platform effects only. All tool parsing, corpus labels, request
// construction, response admission and metrics execute in tool.coax.
#include "tool.h"
#include <stdio.h>
#include <string.h>
static int arg_count;
static char **arg_values;
int64_t gt_argc(void){return arg_count;}
int64_t gt_arg_len(int64_t i){return i>=0&&i<arg_count?(int64_t)strlen(arg_values[i]):-1;}
int64_t gt_arg_byte(int64_t i,int64_t k){return i>=0&&i<arg_count&&k>=0&&(uint64_t)k<strlen(arg_values[i])?(unsigned char)arg_values[i][k]:-1;}
int64_t gt_read_byte(void){int b=fgetc(stdin);return b==EOF?(ferror(stdin)?-2:-1):b;}
_Bool gt_write_byte(int64_t byte){return fputc((unsigned char)byte,stdout)!=EOF;}
int main(int argc,char **argv){arg_count=argc;arg_values=argv;return (int)gev_tool();}
