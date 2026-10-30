/* Elephant and Mammoth on a CUDA device: device effects only (see elephant-gpu.h). */
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
static View weight_view(coax_t_QxModel *m, int64_t weights, int64_t w) {
    if (w == weights - 2) return (View){m->head.data, m->head.count};
    if (w == weights - 1) return (View){m->final_norm.data, m->final_norm.count};
    coax_t_QeLayer *l = &m->layers.data[w / 16];
    const View *fields = l->kind == 0 ? (const View *)&l->linear : (const View *)&l->full;
    return fields[w % 16];
}

const void *eg_weight(EgEngine *e, int64_t w) { return weight_view(&e->model, e->weights, w).data; }

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
        CHECK(pc_kernel(e->modules[k < 3 ? 0 : k < 17 ? 1 : k < 19 ? 2 : 1], text, &e->kernels[k]));
    }
    return EG_OK;
}

int eg_place(EgEngine *e, coax_t_QxModel model, int64_t max_tokens, int64_t budget) {
    return eg_place_kind(e, 0, model, max_tokens, budget);
}

int eg_place_kind(EgEngine *e, int64_t kind, coax_t_QxModel model, int64_t max_tokens, int64_t budget) {
    coax_t_DpModel d = dp_model(kind);
    if (kind < 0 || kind > 1 || d.width != model.shape.linear.width || d.hidden != model.shape.linear.hidden ||
        d.layers != model.shape.state.linear_layers + model.shape.state.full_layers || d.value_heads != model.shape.linear.delta.value_heads ||
        d.heads != model.shape.full.attention.heads || d.vocab != model.vocab) {
        eg_note("the session's model is not plan kind %lld's shape", (long long)kind);
        return EG_REFUSED;
    }
    e->model = model;
    e->shape = d;
    e->kind = kind;
    e->weights = dp_weights(d);
    e->streamable = dp_streamable(d);
    e->width = d.width;
    e->fork_buffer = dp_fork_buffer(d);
    if (e->weights > EG_WEIGHTS || e->streamable > EG_STREAMABLE) return EG_FAILED;
    e->max_tokens = max_tokens;
    size_t free_bytes, total_bytes;
    CHECK(pc_memory(&free_bytes, &total_bytes));
    e->free_before = (int64_t)free_bytes;
    e->budget = (int64_t)free_bytes - ((int64_t)256 << 20) - dp_fork_bytes_of(d); /* left for the driver and for other processes, whose own plans are already placed, and the fork buffer */
    if (budget > 0 && budget < e->budget) e->budget = budget;
    e->op_cap = 8192;
    e->ops = calloc((size_t)e->op_cap, sizeof *e->ops);
    if (!e->ops) return EG_FAILED;
    /* Place for the largest decision; later decisions reuse the placement. */
    e->placed = dp_plan_model(kind, 0, max_tokens, 0, e->budget, (coax_v_bool){e->stream, e->streamable}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->placed.status) { eg_note("the plan refused %lld tokens in %lld bytes: status %u", (long long)max_tokens, (long long)e->budget, e->placed.status); return EG_REFUSED; }
    double t0 = seconds_now();
    CHECK(pc_alloc((size_t)e->placed.workspace, &e->workspace));
    if (e->placed.slot) { CHECK(pc_alloc((size_t)e->placed.slot, &e->slots[0])); CHECK(pc_alloc((size_t)e->placed.slot, &e->slots[1])); }
    CHECK(pc_alloc((size_t)e->placed.resident, &e->arena));
    CHECK(pc_alloc((size_t)dp_fork_bytes_of(d), &e->fork));
    CHECK(pc_stream(&e->compute));
    CHECK(pc_stream(&e->copy));
    int64_t at = 0;
    for (int64_t w = 0; w < e->weights; w++) {
        e->bytes[w] = dp_bytes(d, w);
        if (!e->bytes[w]) continue; /* no such tensor in this kind of layer */
        View v = weight_view(&e->model, e->weights, w);
        /* The head is bytes; the final norm and the other non-bf16 weights are f32. */
        int64_t element = w == e->weights - 2 ? 1 : w == e->weights - 1 ? 4 : dp_bf16(d, w) ? 1 : 4;
        if (element * v.count != e->bytes[w]) { eg_note("weight %lld: the session has %lld bytes, the plan expects %lld", (long long)w, (long long)(element * v.count), (long long)e->bytes[w]); return EG_REFUSED; }
        if (dp_streamed_of(d, w, (coax_v_bool){e->stream, e->streamable})) {
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
    CHECK(pc_host_alloc((size_t)max_tokens * e->width * 4, (void **)&e->x));
    CHECK(pc_host_alloc((size_t)max_tokens * 64 * 4, (void **)&e->rope));
    CHECK(pc_host_alloc((size_t)EG_VOCAB * 4, (void **)&e->logits));
    e->rope_work = malloc((size_t)max_tokens * 64 * 4);
    return e->rope_work ? EG_OK : EG_FAILED;
}

/* Embedding rows of tokens[0..count) and rotary coefficients for positions
   start..start+count into the pinned inputs, both computed by Coax. */
static int stage(EgEngine *e, const int64_t *tokens, int64_t count, int64_t start) {
    if (!dp_embed((coax_v_s64){(int64_t *)tokens, count}, e->width, e->model.embedding, (coax_v_f32){e->x, count * e->width})) return EG_REFUSED;
    coax_t_QwResult r = qw_rope(start, count, (coax_v_f32){e->rope, count * 64}, (coax_v_f32){e->rope_work, count * 64}, 0);
    return r.status ? EG_REFUSED : EG_OK;
}

/* The same at row `at` of the pinned inputs (a batch's question). */
static int stage_at(EgEngine *e, int64_t at, const int64_t *tokens, int64_t count, int64_t start) {
    if (!dp_embed((coax_v_s64){(int64_t *)tokens, count}, e->width, e->model.embedding, (coax_v_f32){e->x + at * e->width, count * e->width})) return EG_REFUSED;
    coax_t_QwResult r = qw_rope(start, count, (coax_v_f32){e->rope + at * 64, count * 64}, (coax_v_f32){e->rope_work, count * 64}, 0);
    return r.status ? EG_REFUSED : EG_OK;
}

int eg_prepare(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix) {
    if (count < 1 || count > e->max_tokens) return EG_REFUSED;
    e->plan = dp_replan(count, prefix, e->placed, (coax_v_bool){e->stream, e->streamable}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->plan.status) return EG_REFUSED;
    if (stage(e, tokens, count, 0)) return EG_REFUSED;
    e->tokens = count;
    e->prefix = prefix;
    return EG_OK;
}

int eg_issue(EgEngine *e, int64_t i, PcStream compute) {
    coax_t_DpOp *op = &e->ops[i];
    if (op->kind == COAX_DpOpKind_copy) {
        CHECK(pc_wait(e->copy, e->released[op->slot]));
        /* The copy's one argument names the host bytes: weight, offset, count. */
        coax_t_DpArg *from = &op->args.e[0];
        if (!e->pinned[op->tensor] || from->offset < 0 || from->count < 1 || from->offset + from->count > e->bytes[op->tensor] || from->count > e->placed.slot) return EG_FAILED;
        CHECK(pc_upload(e->slots[op->slot], (const char *)e->pinned[op->tensor] + from->offset, (size_t)from->count, e->copy));
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
            PcBuffer base = a->buffer == 0 ? e->workspace : a->buffer <= 2 ? e->slots[a->buffer - 1] : a->buffer == e->fork_buffer ? e->fork : e->address[a->buffer - 3];
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

/* Upload the staged inputs of `count` tokens and issue the current plan. */
static int run(EgEngine *e, int64_t count, int (*stop)(void *), void *context) {
    int r;
    CHECK(pc_upload(e->workspace + (PcBuffer)dp_input_offset(count), e->x, (size_t)count * e->width * 4, e->compute));
    CHECK(pc_upload(e->workspace + (PcBuffer)e->plan.rope, e->rope, (size_t)count * 64 * 4, e->compute));
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
    return EG_OK;
}

int eg_decide(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context) {
    int r = eg_prepare(e, tokens, count, prefix);
    if (r) return r;
    e->forked = 0;
    if ((r = run(e, count, stop, context))) return r;
    CHECK(pc_download(e->logits, e->workspace + (PcBuffer)e->plan.logits, EG_VOCAB * 4, e->compute));
    CHECK(pc_sync());
    return EG_OK;
}

int eg_decide_fork(EgEngine *e, const int64_t *tokens, int64_t count, int64_t prefix, int (*stop)(void *), void *context) {
    if (prefix < 1 || prefix >= count || count > e->max_tokens) return eg_decide(e, tokens, count, prefix, stop, context);
    coax_v_bool stream = {e->stream, e->streamable};
    coax_v_t_DpOp ops = {e->ops, e->op_cap};
    int r;
    if (e->fork_length != prefix || memcmp(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens)) {
        e->fork_length = 0; /* the buffer is about to change: a stopped pass leaves no fork */
        e->plan = dp_plan_prefix(prefix, e->placed, stream, ops);
        if (e->plan.status) return EG_REFUSED;
        if ((r = stage(e, tokens, prefix, 0)) || (r = run(e, prefix, stop, context))) return r;
        CHECK(pc_sync()); /* the pinned inputs are staged again next */
        memcpy(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens);
        e->fork_length = prefix;
        e->prefix_passes++;
    }
    e->plan = dp_plan_suffix(prefix, count - prefix, e->placed, stream, ops);
    if (e->plan.status) return EG_REFUSED;
    if ((r = stage(e, tokens + prefix, count - prefix, prefix)) || (r = run(e, count - prefix, stop, context))) return r;
    CHECK(pc_download(e->logits, e->workspace + (PcBuffer)e->plan.logits, EG_VOCAB * 4, e->compute));
    CHECK(pc_sync());
    e->tokens = count;
    e->prefix = prefix;
    e->forked = 1;
    return EG_OK;
}

/* The prefix pass of a fork, unless the fork buffer already holds these
   prefix tokens. */
static int keep_prefix(EgEngine *e, const int64_t *tokens, int64_t prefix, int (*stop)(void *), void *context) {
    if (e->fork_length == prefix && !memcmp(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens)) return EG_OK;
    int r;
    e->fork_length = 0;
    e->plan = dp_plan_prefix(prefix, e->placed, (coax_v_bool){e->stream, e->streamable}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->plan.status) return EG_REFUSED;
    if ((r = stage(e, tokens, prefix, 0)) || (r = run(e, prefix, stop, context))) return r;
    CHECK(pc_sync());
    memcpy(e->fork_tokens, tokens, (size_t)prefix * sizeof *tokens);
    e->fork_length = prefix;
    e->prefix_passes++;
    return EG_OK;
}

int eg_decide_batch(EgEngine *e, const int64_t *const *tokens, const int64_t *counts, int64_t questions, int64_t prefix, float *logits, int (*stop)(void *), void *context) {
    int64_t lengths[EG_BATCH], total = 0;
    int same = questions >= 1 && questions <= EG_BATCH && prefix >= 1;
    for (int64_t i = 0; same && i < questions; i++) {
        if (counts[i] <= prefix || counts[i] > e->max_tokens || memcmp(tokens[i], tokens[0], (size_t)prefix * sizeof **tokens)) same = 0;
        else { lengths[i] = counts[i] - prefix; total += lengths[i]; }
    }
    coax_v_s64 shape = {lengths, questions};
    if (!same || total > e->max_tokens || !dp_batch_admits(e->placed.metal, prefix, shape)) {
        /* One suffix (or whole decision) at a time. */
        e->batched = 0;
        for (int64_t i = 0; i < questions; i++) {
            int r = eg_decide_fork(e, tokens[i], counts[i], prefix, stop, context);
            if (r) return r;
            memcpy(logits + i * EG_VOCAB, e->logits, EG_VOCAB * 4);
        }
        return EG_OK;
    }
    int r = keep_prefix(e, tokens[0], prefix, stop, context);
    if (r) return r;
    e->plan = dp_plan_batch(prefix, shape, e->placed, (coax_v_bool){e->stream, e->streamable}, (coax_v_t_DpOp){e->ops, e->op_cap});
    if (e->plan.status) { eg_note("the batch plan refused: status %u", e->plan.status); return EG_REFUSED; }
    for (int64_t i = 0, at = 0; i < questions; at += lengths[i], i++)
        if ((r = stage_at(e, at, tokens[i] + prefix, lengths[i], prefix))) return r;
    if ((r = run(e, total, stop, context))) return r;
    for (int64_t i = 0; i < questions; i++)
        CHECK(pc_download(logits + i * EG_VOCAB, e->workspace + (PcBuffer)dp_batch_logits(e->kind, prefix, shape, i), EG_VOCAB * 4, e->compute));
    CHECK(pc_sync());
    e->tokens = total;
    e->prefix = prefix;
    e->forked = 1;
    e->batched = questions;
    return EG_OK;
}
