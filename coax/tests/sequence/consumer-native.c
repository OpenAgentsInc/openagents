/* Actual generated consumer procedures only; no host, model or GPU effects. */
#include <inttypes.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifndef SEQUENCE_INPUT_C
#error "SEQUENCE_INPUT_C must name the actual generated consumer C"
#endif
#ifndef SEQUENCE_FOREIGN_STUBS
#error "SEQUENCE_FOREIGN_STUBS must name the abort-on-call host stubs"
#endif
#ifndef SEQUENCE_CONSUMER_VERSE
#error "SEQUENCE_CONSUMER_VERSE must be 0 or 1"
#endif

static uint64_t sequence_consumer_calls;
static uint64_t sequence_consumer_allocations;
static bool sequence_consumer_active;

/* Count explicit generated allocator calls, not standard-library internals. */
static void *sequence_consumer_malloc(size_t size) {
    if (sequence_consumer_active) sequence_consumer_allocations++;
    return malloc(size);
}
static void *sequence_consumer_calloc(size_t count, size_t size) {
    if (sequence_consumer_active) sequence_consumer_allocations++;
    return calloc(count, size);
}
static void *sequence_consumer_realloc(void *pointer, size_t size) {
    if (sequence_consumer_active) sequence_consumer_allocations++;
    return realloc(pointer, size);
}

#define malloc sequence_consumer_malloc
#define calloc sequence_consumer_calloc
#define realloc sequence_consumer_realloc
#include SEQUENCE_INPUT_C
#include SEQUENCE_FOREIGN_STUBS
#undef malloc
#undef calloc
#undef realloc

static void sequence_consumer_fill(uint8_t *bytes, size_t count) {
    for (size_t i = 0; i < count; i++) bytes[i] = (uint8_t)(0xa5u ^ (37u * i));
}
static void sequence_consumer_fail(const char *message) {
    fprintf(stderr, "sequence consumer: %s\n", message);
    exit(1);
}

#if SEQUENCE_CONSUMER_VERSE

static void sequence_consumer_put(coax_t_Sp *state, int64_t value) {
    sequence_consumer_active = true;
    sequence_consumer_calls++;
    coax_p_put_int((coax_v_t_Sp){state, 1}, value);
    sequence_consumer_active = false;
}

/* A sign is appended separately. Bad is sticky, but does not suppress writes
 * that fit. Preserve this non-atomic behavior rather than imposing a new one.
 */
static void sequence_consumer_append(uint8_t *bytes, int64_t capacity,
                                     int64_t *used, bool *bad,
                                     const char *text, size_t length) {
    if (*used + (int64_t)length > capacity) {
        *bad = true;
        return;
    }
    memcpy(bytes + *used, text, length);
    *used += (int64_t)length;
}

static uint64_t sequence_consumer_verse(void) {
    static const int64_t values[] = {
        0, 1, -1, 9, -9, 10, -10, 99, -99, 100, -100, 255, -255,
        65535, -65535, 1000000, -1000000,
        INT64_C(922337203685477580), -INT64_C(922337203685477580),
        INT64_MAX, INT64_MIN + 1,
    };
    enum { GUARD = 16, MAX_CAPACITY = 40, BUFFER = 96 };
    uint64_t cases = 0;
    for (size_t v = 0; v < sizeof values / sizeof values[0]; v++) {
        char formatted[32];
        int length = snprintf(formatted, sizeof formatted, "%" PRId64, values[v]);
        if (length < 1 || (size_t)length >= sizeof formatted)
            sequence_consumer_fail("snprintf oracle did not fit");
        bool negative = values[v] < 0;
        const char *digits = formatted + (negative ? 1 : 0);
        size_t digit_count = (size_t)length - (negative ? 1u : 0u);
        for (int64_t capacity = 0; capacity <= MAX_CAPACITY; capacity++) {
            for (int64_t start = 0; start <= capacity + 1; start++) {
                for (int initial_bad = 0; initial_bad <= 1; initial_bad++) {
                    uint8_t actual[BUFFER], expected[BUFFER];
                    sequence_consumer_fill(actual, sizeof actual);
                    memcpy(expected, actual, sizeof expected);
                    int64_t used = start;
                    bool bad = initial_bad != 0;
                    if (negative)
                        sequence_consumer_append(expected + GUARD, capacity,
                                                 &used, &bad, "-", 1);
                    sequence_consumer_append(expected + GUARD, capacity,
                                             &used, &bad, digits, digit_count);
                    coax_t_Sp state = {
                        .buf = {actual + GUARD, capacity},
                        .n = start,
                        .bad = initial_bad != 0,
                    };
                    sequence_consumer_put(&state, values[v]);
                    if (state.n != used || state.bad != bad ||
                        state.buf.data != actual + GUARD || state.buf.count != capacity ||
                        memcmp(actual, expected, sizeof actual) != 0) {
                        fprintf(stderr, "value=%" PRId64 " capacity=%" PRId64
                                " start=%" PRId64 " initial_bad=%d\n",
                                values[v], capacity, start, initial_bad);
                        sequence_consumer_fail("Verse formatter differs from snprintf append oracle");
                    }
                    cases++;
                }
            }
        }
    }
    return cases;
}

#else

static void sequence_consumer_reverse(uint8_t *bytes, int64_t capacity,
                                     int64_t from, int64_t to) {
    sequence_consumer_active = true;
    sequence_consumer_calls++;
    coax_p_sv_reverse((coax_v_u8){bytes, capacity}, from, to);
    sequence_consumer_active = false;
}

static uint64_t sequence_consumer_psionic(void) {
    enum { GUARD = 16, PAYLOAD = 160, BUFFER = 208 };
    uint64_t cases = 0;
    for (int64_t from = 0; from <= 32; from++) {
        for (int64_t to = 0; to <= 32; to++) {
            uint8_t actual[BUFFER], before[BUFFER], expected[BUFFER];
            sequence_consumer_fill(actual, sizeof actual);
            memcpy(before, actual, sizeof before);
            memcpy(expected, actual, sizeof expected);
            /* Direct coordinate mapping, independent of pair swapping. */
            for (int64_t at = 0; at < 32; at++) {
                if (at >= from && at < to)
                    expected[GUARD + at] = before[GUARD + from + to - 1 - at];
            }
            sequence_consumer_reverse(actual + GUARD, 32, from, to);
            if (memcmp(actual, expected, sizeof actual) != 0) {
                fprintf(stderr, "from=%" PRId64 " to=%" PRId64 "\n", from, to);
                sequence_consumer_fail("Psionic reversal differs from coordinate oracle");
            }
            cases++;
        }
    }

    static const int codes[] = {200, 400, 503};
    static const char *const reasons[] = {"OK", "Bad Request", "Service Unavailable"};
    for (size_t status = 0; status < sizeof codes / sizeof codes[0]; status++) {
        for (int64_t body_count = 0; body_count <= 64; body_count++) {
            char header[96];
            int header_count = snprintf(header, sizeof header,
                                        "HTTP/1.1 %d %s\r\nContent-Length: %" PRId64 "\r\n\r\n",
                                        codes[status], reasons[status], body_count);
            if (header_count < 1 || (size_t)header_count >= sizeof header ||
                body_count + header_count > PAYLOAD)
                sequence_consumer_fail("HTTP fixture exceeds its bounded payload");
            uint8_t body[64], actual[BUFFER], expected[BUFFER];
            for (int64_t i = 0; i < body_count; i++) body[i] = (uint8_t)('a' + i % 26);
            sequence_consumer_fill(actual, sizeof actual);
            memcpy(actual + GUARD, body, (size_t)body_count);
            memcpy(actual + GUARD + body_count, header, (size_t)header_count);
            memcpy(expected, actual, sizeof expected);
            /* Desired response is direct header + body concatenation. */
            memcpy(expected + GUARD, header, (size_t)header_count);
            memcpy(expected + GUARD + header_count, body, (size_t)body_count);
            sequence_consumer_reverse(actual + GUARD, PAYLOAD, 0, body_count);
            sequence_consumer_reverse(actual + GUARD, PAYLOAD, body_count,
                                      body_count + header_count);
            sequence_consumer_reverse(actual + GUARD, PAYLOAD, 0,
                                      body_count + header_count);
            if (memcmp(actual, expected, sizeof actual) != 0)
                sequence_consumer_fail("HTTP rotation differs from direct concatenation");
            cases++;
        }
    }
    return cases;
}

#endif

int main(int argc, char **argv) {
    (void)&sequence_consumer_malloc;
    (void)&sequence_consumer_calloc;
    (void)&sequence_consumer_realloc;
#if SEQUENCE_CONSUMER_VERSE
    if (argc == 2 && strcmp(argv[1], "min") == 0) {
        uint8_t bytes[32] = {0};
        coax_t_Sp state = {.buf = {bytes, sizeof bytes}, .n = 0, .bad = false};
        sequence_consumer_put(&state, INT64_MIN);
        sequence_consumer_fail("checked INT64_MIN formatting returned instead of trapping");
    }
#endif
    if (argc != 1) sequence_consumer_fail("unexpected command arguments");
    (void)argv;
    clock_t started = clock();
#if SEQUENCE_CONSUMER_VERSE
    uint64_t cases = sequence_consumer_verse();
    const char *kind = "verse-put-int";
#else
    uint64_t cases = sequence_consumer_psionic();
    const char *kind = "psionic-reverse";
#endif
    double seconds = (double)(clock() - started) / CLOCKS_PER_SEC;
    if (sequence_consumer_allocations != 0)
        sequence_consumer_fail("target made an explicit heap allocation");
    printf("{\"kind\":\"%s\",\"cases\":%" PRIu64 ",\"calls\":%" PRIu64
           ",\"allocations\":%" PRIu64 ",\"seconds\":%.9f}\n",
           kind, cases, sequence_consumer_calls, sequence_consumer_allocations, seconds);
    return 0;
}
