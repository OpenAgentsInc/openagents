#ifndef METAREAL_HOST_H
#define METAREAL_HOST_H
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
// Resident platform storage. Handles are generation checked and confined to
// one calling thread. No graph, animation, layout, or interaction policy here.
typedef struct { float x,y,size,light,soft,crisp,pad0,pad1; } MRDot;
typedef struct { float ax,ay,bx,by,lightA,lightB,width,pad; } MRLine;
typedef struct { float x,y,width,height,press,flash; char label[33]; } MRBar;
int64_t mr_create(const uint8_t *config, size_t size, size_t cells);
void mr_destroy(int64_t handle);
void mr_clear_frame(int64_t handle);
bool mr_ok(int64_t handle);
const MRDot *mr_dots(int64_t handle);
const MRLine *mr_lines(int64_t handle);
size_t mr_dot_count(int64_t handle);
size_t mr_line_count(int64_t handle);
const MRBar *mr_bar(int64_t handle);
const uint8_t *mr_output(int64_t handle);
size_t mr_output_size(int64_t handle);
#endif
