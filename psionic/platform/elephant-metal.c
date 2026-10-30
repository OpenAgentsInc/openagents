/* Elephant on an Apple GPU: device effects only (see elephant-metal.h). */
#include "elephant-metal.h"
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static char note_text[512];
static void eg_note(const char *fmt, ...) { va_list ap; va_start(ap, fmt); vsnprintf(note_text, sizeof note_text, fmt, ap); va_end(ap); }
const char *eg_log(void) { return note_text[0] ? note_text : pm_log(); }

static double seconds_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }
#define CHECK(x) do { PmStatus s_ = (x); if (s_ != PM_OK) { note_text[0] = 0; return s_ == PM_ABSENT ? EG_SKIP : EG_FAILED; } } while (0)

/* The session's view of weight id w, as in elephant-gpu.c. */
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
    PmStatus s = pm_open(&e->device);
    if (s == PM_ABSENT) return EG_SKIP;
    if (s) return EG_FAILED;
    static const char *names[4] = {"project.metal", "layer.metal", "warp.metal", "tile.metal"};
    for (int i = 0; i < 4; i++) {
        double seconds = 0;
        CHECK(pm_compile(src->text[i], src->length[i], names[i], &e->modules[i], &seconds));
        e->compile_seconds += seconds;
    }
    for (int k = 0; k < EG_KERNELS; k++) {
        coax_string name = dp_kernel_name(k);
        char text[64];
        snprintf(text, sizeof text, "%.*s", (int)name.count, name.data);
        CHECK(pm_kernel(e->modules[k < 3 ? 0 : k < 17 ? 1 : k < 19 ? 2 : k < 21 ? 1 : 3], text, &e->kernels[k]));
    }
    return EG_OK;
}

int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget) {
    e->model = model;
    e->max_tokens = max_tokens;
    CHECK(pm_memory(&e->device));
    e->budget = (int64_t)e->device.working_set_bytes - ((int64_t)4 << 30);
    if (budget > 0 && budget < e->budget) e->budget = budget;
    e->op_cap = 4096;
    e->ops = calloc((size_t)e->op_cap, sizeof *e->ops);
    if (!e->ops) return EG_FAILED;
    e->placed = dp_plan_metal(max_tokens, 0, e->budget, (coax_v_bool){e->stream, EG_STREAMABLE}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->placed.status) { eg_note("the plan refused %lld tokens in %lld bytes: status %u", (long long)max_tokens, (long long)e->budget, e->placed.status); return EG_REFUSED; }
    if (e->placed.streamed_tensors) { eg_note("the plan streams %lld weights; unified memory keeps every weight resident", (long long)e->placed.streamed_tensors); return EG_REFUSED; }
    double t0 = seconds_now();
    CHECK(pm_alloc((size_t)e->placed.workspace, &e->workspace));
    CHECK(pm_alloc((size_t)e->placed.resident, &e->arena));
    CHECK(pm_alloc((size_t)dp_fork_bytes(), &e->fork));
    uint8_t *arena = pm_contents(e->arena);
    int64_t at = 0;
    for (int64_t w = 0; w < EG_WEIGHTS; w++) {
        e->bytes[w] = dp_weight_bytes(w);
        if (!e->bytes[w]) continue;
        View v = weight_view(&e->model, w);
        int64_t element = w == EG_WEIGHTS - 2 ? 1 : w == EG_WEIGHTS - 1 ? 4 : dp_weight_bf16(w) ? 1 : 4;
        if (element * v.count != e->bytes[w]) { eg_note("weight %lld: the session has %lld bytes, the plan expects %lld", (long long)w, (long long)(element * v.count), (long long)e->bytes[w]); return EG_REFUSED; }
        if (at + e->bytes[w] > e->placed.resident) { eg_note("weight %lld does not fit the resident arena", (long long)w); return EG_REFUSED; }
        e->offset[w] = at;
        memcpy(arena + at, v.data, (size_t)e->bytes[w]);
        at += (e->bytes[w] + 255) / 256 * 256;
    }
    e->resident_bytes = at;
    e->upload_seconds = seconds_now() - t0;
    e->rope_work = malloc((size_t)max_tokens * 64 * 4);
    return e->rope_work ? EG_OK : EG_FAILED;
}

/* The current plan's inputs: embedding rows of tokens[0..count) and rotary
   coefficients for positions start..start+count, both computed by Coax. */
static int stage(EgEngine *e, const int64_t *tokens, int64_t count, int64_t start) {
    uint8_t *ws = pm_contents(e->workspace);
    float *x = (float *)(ws + dp_input_offset(count)), *rope = (float *)(ws + e->plan.rope);
    if (!dp_embed((coax_v_s64){(int64_t *)tokens, count}, EG_WIDTH, e->model.embedding, (coax_v_f32){x, count * EG_WIDTH})) return EG_REFUSED;
    coax_t_QwResult r = qw_rope(start, count, (coax_v_f32){rope, count * 64}, (coax_v_f32){e->rope_work, count * 64}, 0);
    return r.status ? EG_REFUSED : EG_OK;
}

int eg_prepare(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix) {
    if (count < 1 || count > e->max_tokens) return EG_REFUSED;
    e->plan = dp_replan(count, prefix, e->placed, (coax_v_bool){e->stream, EG_STREAMABLE}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->plan.status) return EG_REFUSED;
    int r = stage(e, tokens, count, 0);
    if (r) return r;
    e->tokens = count;
    e->prefix = prefix;
    return EG_OK;
}

int eg_encode(EgEngine *e, PmBatch batch, int64_t from, int64_t to) {
    for (int64_t i = from; i < to; i++) {
        coax_t_DpOp *op = &e->ops[i];
        if (op->kind != COAX_DpOpKind_launch) { eg_note("operation %lld is a copy, wait or release; nothing streams on Metal", (long long)i); return EG_FAILED; }
        PmArg args[12];
        for (int64_t k = 0; k < op->count; k++) {
            coax_t_DpArg *a = &op->args.e[k];
            args[k] = (PmArg){(PmArgKind)a->kind, a->value, a->real, NULL, 0, a->count};
            if (a->kind == 4) {
                if (a->buffer == 0) { args[k].buffer = e->workspace; args[k].offset = a->offset; }
                else if (a->buffer >= 3 && a->buffer - 3 < EG_WEIGHTS && e->bytes[a->buffer - 3]) { args[k].buffer = e->arena; args[k].offset = e->offset[a->buffer - 3] + a->offset; }
                else if (a->buffer == EG_FORK_BUFFER && e->fork) { args[k].buffer = e->fork; args[k].offset = a->offset; }
                else { eg_note("operation %lld names buffer %lld", (long long)i, (long long)a->buffer); return EG_FAILED; }
            }
        }
        PmStatus s = pm_launch(batch, e->kernels[op->kernel], op->items, op->lanes > 1 ? (int)op->lanes : 1, (int)op->block, args, (int)op->count);
        if (s) { note_text[0] = 0; return EG_FAILED; }
    }
    return EG_OK;
}

int eg_layer_start(const EgEngine *e, int64_t i) { return dp_stop_point(e->ops[i]); }

/* Run every operation of the current plan; one command buffer per layer when
   the caller may stop between layers, otherwise all in one. */
static int run(EgEngine *e, int (*stop)(void *), void *context) {
    int r;
    /* One command buffer per layer when the caller may stop between layers;
       otherwise the whole decision in one. */
    int64_t from = 0;
    while (from < e->plan.ops) {
        int64_t to = from + 1;
        if (stop) while (to < e->plan.ops && !eg_layer_start(e, to)) to++;
        else to = e->plan.ops;
        if (stop && eg_layer_start(e, from) && stop(context)) return EG_STOPPED;
        PmBatch batch;
        CHECK(pm_begin(&batch));
        r = eg_encode(e, batch, from, to);
        double seconds = 0;
        PmStatus s = pm_wait(batch, &seconds);
        if (r) return r;
        if (s) { note_text[0] = 0; return EG_FAILED; }
        e->gpu_seconds += seconds;
        from = to;
    }
    return EG_OK;
}

int eg_decide(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context) {
    int r = eg_prepare(e, tokens, count, prefix);
    if (r) return r;
    e->gpu_seconds = 0;
    e->forked = 0;
    if ((r = run(e, stop, context))) return r;
    memcpy(e->logits, (uint8_t *)pm_contents(e->workspace) + e->plan.logits, sizeof e->logits);
    return EG_OK;
}

int eg_decide_fork(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context) {
    if (prefix < 1 || prefix >= count || count > e->max_tokens) return eg_decide(e, tokens, count, prefix, stop, context);
    coax_v_bool stream = {e->stream, EG_STREAMABLE};
    coax_v_t_DpOp ops = {e->ops, e->op_cap};
    int r;
    e->gpu_seconds = 0;
    if (e->fork_length != prefix || memcmp(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens)) {
        e->fork_length = 0; /* the buffer is about to change: a stopped pass leaves no fork */
        e->plan = dp_plan_prefix(prefix, e->placed, stream, ops);
        if (e->plan.status) return EG_REFUSED;
        if ((r = stage(e, tokens, prefix, 0)) || (r = run(e, stop, context))) return r;
        memcpy(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens);
        e->fork_length = prefix;
        e->prefix_passes++;
    }
    e->plan = dp_plan_suffix(prefix, count - prefix, e->placed, stream, ops);
    if (e->plan.status) return EG_REFUSED;
    if ((r = stage(e, tokens + prefix, count - prefix, prefix)) || (r = run(e, stop, context))) return r;
    memcpy(e->logits, (uint8_t *)pm_contents(e->workspace) + e->plan.logits, sizeof e->logits);
    e->tokens = count;
    e->prefix = prefix;
    e->forked = 1;
    return EG_OK;
}
