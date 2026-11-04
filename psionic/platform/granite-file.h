/* Granite GGUF admission effects: map the file, then hand its header to the
   Coax binding (psionic/lib/granite/model.coax) and tokenizer tables
   (psionic/lib/granite/tokenizer.coax). No metadata is interpreted here. */
#ifndef PSIONIC_GRANITE_FILE_H
#define PSIONIC_GRANITE_FILE_H
#include <stddef.h>
#include <stdint.h>
typedef struct GrFile {
    const uint8_t *bytes; size_t size; int fd;
    coax_t_PgMeta *meta, *meta_work; coax_t_PgTensor *tensors, *tensor_work; coax_t_GrTensor *table;
    coax_t_GrBinding binding; coax_t_GrTables tables;
} GrFile;
/* 0 on success; otherwise a message in error. */
int gr_file_open(const char *path, GrFile *f, char *error, size_t error_bytes);
void gr_file_close(GrFile *f);
#endif
