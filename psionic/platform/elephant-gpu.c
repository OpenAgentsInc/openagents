/* Elephant on a CUDA device: device effects only (see elephant-gpu.h). */
#define _GNU_SOURCE
#include "elephant-gpu.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>
#include <time.h>

static char note_text[512];
static void eg_note(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vsnprintf(note_text, sizeof note_text, fmt, ap); va_end(ap); }
const char *eg_log(void) { return note_text[0] ? note_text : pc_log(); }

static double seconds_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
#define CHECK(x) do { PcStatus s_ = (x); if (s_ != PC_OK) return s_ == PC_ABSENT || s_ == PC_NO_NVRTC ? EG_SKIP : EG_FAILED; } while (0)

/* The session's view of weight id w: QlWeights and QhWeights are runs of
   {pointer, count} views in the plan's weight order. */
typedef struct { const void *data; int64_t count; } View;
static View weight_view(coax_t_QxModel *m, int64_t w) {
    if (w == EG_WEIGHTS - 2) return (View){m->head.data, m->head.count};
    if (w == EG_WEIGHTS - 1) return (View){m->final_norm.data, m->final_norm.count};
    coax_t_QeLayer *l = &m->layers.data[w / 16];
    const View *fields = l->kind == 0 ? (const View *)&l->linear : (const View *)&l->full;
    return fields[w % 16];
}

const void *eg_weight(EgEngine *e, int64_t w) { return weight_view(&e->model, w).data; }

int eg_compile(EgEngine *e, const EgSources *src) {
    memset(e, 0, sizeof *e);
    PcStatus s = pc_open(&e->device);
    if (s == PC_ABSENT) return EG_SKIP;
    if (s) return EG_FAILED;
    static const char *names[3] = {"project.cu", "layer.cu", "warp.cu"};
    for (int i = 0; i < 3; i++) {
        double seconds = 0;
        CHECK(pc_compile(src->text[i], src->length[i], names[i], &e->modules[i], &seconds));
        e->compile_seconds += seconds;
    }
    for (int k = 0; k < EG_KERNELS; k++) {
        coax_string name = dp_kernel_name(k);
        char text[64];
        snprintf(text, sizeof text, "%.*s", (int)name.count, name.data);
        CHECK(pc_kernel(e->modules[k < 3 ? 0 : k < 17 ? 1 : 2], text, &e->kernels[k]));
    }
    return EG_OK;
}

int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget) {
    e->model = model;
    e->max_tokens = max_tokens;
    size_t free_bytes, total_bytes;
    CHECK(pc_memory(&free_bytes, &total_bytes));
    e->free_before = (int64_t)free_bytes;
    e->budget = (int64_t)free_bytes - ((int64_t)512 << 20); /* left for other processes and the driver */
    if (budget > 0 && budget < e->budget) e->budget = budget;
    e->op_cap = 4096;
    e->ops = calloc((size_t)e->op_cap, sizeof *e->ops);
    if (!e->ops) return EG_FAILED;
    /* Place for the largest decision; later decisions reuse the placement. */
    e->placed = dp_plan(max_tokens, 0, e->budget, (coax_v_bool){e->stream, 96}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->placed.status) { eg_note("the plan refused %lld tokens in %lld bytes: status %u", (long long)max_tokens, (long long)e->budget, e->placed.status); return EG_REFUSED; }
    double t0 = seconds_now();
    CHECK(pc_alloc((size_t)e->placed.workspace, &e->workspace));
    if (e->placed.slot) { CHECK(pc_alloc((size_t)e->placed.slot, &e->slots[0])); CHECK(pc_alloc((size_t)e->placed.slot, &e->slots[1])); }
    CHECK(pc_alloc((size_t)e->placed.resident, &e->arena));
    CHECK(pc_stream(&e->compute));
    CHECK(pc_stream(&e->copy));
    int64_t at = 0;
    for (int64_t w = 0; w < EG_WEIGHTS; w++) {
        e->bytes[w] = dp_weight_bytes(w);
        if (!e->bytes[w]) continue; /* no such tensor in this kind of layer */
        View v = weight_view(&e->model, w);
        /* The head is bytes; the final norm and the other non-bf16 weights are f32. */
        int64_t element = w == EG_WEIGHTS - 2 ? 1 : w == EG_WEIGHTS - 1 ? 4 : dp_weight_bf16(w) ? 1 : 4;
        if (element * v.count != e->bytes[w]) { eg_note("weight %lld: the session has %lld bytes, the plan expects %lld", (long long)w, (long long)(element * v.count), (long long)e->bytes[w]); return EG_REFUSED; }
        if (dp_streamed(w, (coax_v_bool){e->stream, 96})) {
            CHECK(pc_host_alloc((size_t)e->bytes[w], &e->pinned[w]));
            memcpy(e->pinned[w], v.data, (size_t)e->bytes[w]);
            e->pinned_bytes += e->bytes[w];
            continue;
        }
        e->address[w] = e->arena + (PcBuffer)at;
        CHECK(pc_upload(e->address[w], v.data, (size_t)e->bytes[w], e->copy));
        at += (e->bytes[w] + 255) / 256 * 256;
    }
    CHECK(pc_sync());
    e->resident_bytes = at;
    e->upload_seconds = seconds_now() - t0;
    for (int i = 0; i < 2; i++) { CHECK(pc_event(&e->filled[i])); CHECK(pc_event(&e->released[i])); }
    CHECK(pc_host_alloc((size_t)max_tokens * EG_WIDTH * 4, (void **)&e->x));
    CHECK(pc_host_alloc((size_t)max_tokens * 64 * 4, (void **)&e->rope));
    CHECK(pc_host_alloc((size_t)EG_VOCAB * 4, (void **)&e->logits));
    e->rope_work = malloc((size_t)max_tokens * 64 * 4);
    return e->rope_work ? EG_OK : EG_FAILED;
}

int eg_prepare(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix) {
    if (count < 1 || count > e->max_tokens) return EG_REFUSED;
    e->plan = dp_replan(count, prefix, e->placed, (coax_v_bool){e->stream, 96}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->plan.status) return EG_REFUSED;
    if (!dp_embed((coax_v_s64){(int64_t *)tokens, count}, EG_WIDTH, e->model.embedding, (coax_v_f32){e->x, count * EG_WIDTH})) return EG_REFUSED;
    coax_t_QwResult r = qw_rope(0, count, (coax_v_f32){e->rope, count * 64}, (coax_v_f32){e->rope_work, count * 64}, 0);
    if (r.status) return EG_REFUSED;
    e->tokens = count;
    e->prefix = prefix;
    return EG_OK;
}

int eg_issue(EgEngine *e, int64_t i, PcStream compute) {
    coax_t_DpOp *op = &e->ops[i];
    if (op->kind == COAX_DpOpKind_copy) {
        CHECK(pc_wait(e->copy, e->released[op->slot]));
        CHECK(pc_upload(e->slots[op->slot], e->pinned[op->tensor], (size_t)e->bytes[op->tensor], e->copy));
        CHECK(pc_record(e->filled[op->slot], e->copy));
        return EG_OK;
    }
    if (op->kind == COAX_DpOpKind_wait) { CHECK(pc_wait(compute, e->filled[op->slot])); return EG_OK; }
    if (op->kind == COAX_DpOpKind_release) { CHECK(pc_record(e->released[op->slot], compute)); return EG_OK; }
    PcArg args[12];
    for (int64_t k = 0; k < op->count; k++) {
        coax_t_DpArg *a = &op->args.e[k];
        args[k] = (PcArg){(PcArgKind)a->kind, a->value, a->real, 0, a->count};
        if (a->kind == 4) {
            PcBuffer base = a->buffer == 0 ? e->workspace : a->buffer <= 2 ? e->slots[a->buffer - 1] : e->address[a->buffer - 3];
            if (!base) return EG_FAILED;
            args[k].data = base + (PcBuffer)a->offset;
        }
    }
    if (op->lanes == 32) CHECK(pc_launch_warps(e->kernels[op->kernel], op->items, (int)op->block, args, (int)op->count, compute));
    else CHECK(pc_launch(e->kernels[op->kernel], op->items, (int)op->block, args, (int)op->count, compute));
    return EG_OK;
}

int eg_layer_start(const EgEngine *e, int64_t i) {
    return dp_stop_point(e->ops[i]);
}

int eg_decide(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context) {
    int r = eg_prepare(e, tokens, count, prefix);
    if (r) return r;
    CHECK(pc_upload(e->workspace + (PcBuffer)dp_input_offset(count), e->x, (size_t)count * EG_WIDTH * 4, e->compute));
    CHECK(pc_upload(e->workspace + (PcBuffer)dp_rope_offset(count), e->rope, (size_t)count * 64 * 4, e->compute));
    for (int64_t i = 0; i < e->plan.ops; i++) {
        if (stop && eg_layer_start(e, i)) {
            /* Let the device catch up so the check sees time actually spent. */
            CHECK(pc_sync_stream(e->compute));
            if (stop(context)) {
                CHECK(pc_sync());
                return EG_STOPPED;
            }
        }
        r = eg_issue(e, i, e->compute);
        if (r) return r;
    }
    CHECK(pc_download(e->logits, e->workspace + (PcBuffer)dp_logits_offset(count), EG_VOCAB * 4, e->compute));
    CHECK(pc_sync());
    return EG_OK;
}
