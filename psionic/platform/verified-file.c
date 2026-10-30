/* Verified-file records: effects only (see verified-file.h). */
#define _GNU_SOURCE
#include "decision-session.h"
#include "verified-file.h"
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/mman.h>
#include <unistd.h>

/* Nanosecond mtime and ctime: Linux names them st_mtim/st_ctim, macOS
   st_mtimespec/st_ctimespec. */
#if defined(__APPLE__)
#define VF_MTIME(st) ((int64_t)(st).st_mtimespec.tv_sec * 1000000000 + (st).st_mtimespec.tv_nsec)
#define VF_CTIME(st) ((int64_t)(st).st_ctimespec.tv_sec * 1000000000 + (st).st_ctimespec.tv_nsec)
#else
#define VF_MTIME(st) ((int64_t)(st).st_mtim.tv_sec * 1000000000 + (st).st_mtim.tv_nsec)
#define VF_CTIME(st) ((int64_t)(st).st_ctim.tv_sec * 1000000000 + (st).st_ctim.tv_nsec)
#endif

int64_t ps_shard_record(uint8_t kind, int64_t capacity, int64_t i, coax_t_PaStat st, coax_v_u8 out);

static int record_dir(char *out, size_t size) {
    const char *dir = getenv("PSIONIC_VERIFIED_DIR"), *cache = getenv("XDG_CACHE_HOME"), *home = getenv("HOME");
    int n;
    if (dir && *dir) n = snprintf(out, size, "%s", dir);
    else if (cache && *cache) n = snprintf(out, size, "%s/psionic/verified", cache);
    else if (home && *home) n = snprintf(out, size, "%s/.cache/psionic/verified", home);
    else return -1;
    return n > 0 && (size_t)n < size ? 0 : -1;
}

static int make_dirs(char *path) {
    for (char *p = path + 1; *p; p++) {
        if (*p != '/') continue;
        *p = 0;
        int r = mkdir(path, 0700);
        *p = '/';
        if (r && errno != EEXIST) return -1;
    }
    return mkdir(path, 0700) && errno != EEXIST ? -1 : 0;
}

void vf_attach_fd(VfEntry *e, int fd, coax_t_PaFile *f) {
    memset(e, 0, sizeof *e);
    f->record = (coax_v_u8){NULL, 0};
    struct stat st;
    char dir[3800];
    if (fstat(fd, &st) || st.st_size != f->data.count) return;
    e->stat = (coax_t_PaStat){(int64_t)st.st_dev, (int64_t)st.st_ino, (int64_t)st.st_size,
        VF_MTIME(st), VF_CTIME(st)};
    f->stat = e->stat;
    long page=sysconf(_SC_PAGESIZE);e->cached_pages=-1;
    if(page>0){
        e->total_pages=(f->data.count+page-1)/page;int64_t cached=0;int ok=1;unsigned char vec[65536];
        for(int64_t start=0;start<e->total_pages;start+=65536){int64_t n=e->total_pages-start;if(n>65536)n=65536;size_t length=(size_t)n*(size_t)page;
            if(mincore(f->data.data+start*page,length,(void*)vec)){ok=0;break;}for(int64_t i=0;i<n;i++)cached+=vec[i]&1;
        }if(ok)e->cached_pages=cached;
    }
    if (record_dir(dir, sizeof dir)) return;
    snprintf(e->path, sizeof e->path, "%s/%llu-%llu.rec", dir, (unsigned long long)st.st_dev, (unsigned long long)st.st_ino);
    FILE *in = fopen(e->path, "rb");
    if (!in) return;
    size_t n = fread(e->record, 1, sizeof e->record, in);
    fclose(in);
    e->length = (int64_t)n;
    e->found = 1;
    f->record = (coax_v_u8){e->record, e->length};
}

int vf_map_file(VfEntry *e, const char *path, coax_t_PaFile *f) {
    int fd = open(path, O_RDONLY); struct stat st;
    if (fd < 0) return -1;
    if (fstat(fd, &st) || st.st_size <= 0) { close(fd); return -1; }
    void *p = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
    if (p == MAP_FAILED) { close(fd); return -1; }
    coax_t_PaFile draft = {.data = {p, st.st_size}};
    vf_attach_fd(e, fd, &draft); close(fd);
    if (draft.stat.size != st.st_size) { munmap(p, (size_t)st.st_size); return -1; }
    *f = draft; return 0;
}

int vf_keep(VfEntry *e, uint8_t kind, int64_t capacity, int64_t shard) {
    if (!e->path[0]) return -1;
    uint8_t text[512];
    int64_t n = ps_shard_record(kind, capacity, shard, e->stat, (coax_v_u8){text, sizeof text});
    if (n <= 0) return -1;
    if (e->found && e->length == n && !memcmp(e->record, text, (size_t)n)) return 0;
    char dir[4096], tmp[4200];
    snprintf(dir, sizeof dir, "%s", e->path);
    char *slash = strrchr(dir, '/');
    if (!slash) return -1;
    *slash = 0;
    if (make_dirs(dir)) return -1;
    snprintf(tmp, sizeof tmp, "%s.%ld.tmp", e->path, (long)getpid());
    int fd = open(tmp, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (fd < 0) return -1;
    int ok = write(fd, text, (size_t)n) == (ssize_t)n && fsync(fd) == 0;
    ok = close(fd) == 0 && ok;
    if (!ok || rename(tmp, e->path)) { unlink(tmp); return -1; }
    return 1;
}
