/* Independent declarations exercise the published ABI2 layout. */
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint64_t bits; const char *label; int64_t label_count; } EnumValue;
typedef struct {
    const char *name; int64_t name_count;
    const char *type_name; int64_t type_name_count;
    int64_t kind, width, is_signed, first;
    uint64_t count;
    const EnumValue *values; int64_t value_count;
} Binding;
typedef struct {
    int64_t declaration_index;
    const char *name; int64_t name_count;
    const char *source; int64_t source_count;
    int64_t line, column;
    int64_t kind;
    const Binding *bindings; int64_t binding_count;
    uint64_t case_count;
    int64_t (*run_values)(const uint64_t *, int64_t);
} Check;
extern const Check *coax_verification_checks_v2(int64_t *count);

static void hex(const char *text, int64_t n) {
    if (n < 0 || n > 8192 || (!text && n)) exit(3);
    for (int64_t i = 0; i < n; ++i) printf("%02x", (unsigned char)text[i]);
}

int main(int argc, char **argv) {
    int64_t count = -1;
    const Check *checks = coax_verification_checks_v2(&count);
    if (!checks || count <= 0 || count > 4096 || argc < 2) return 2;
    if (!strcmp(argv[1], "metadata")) {
        for (int64_t i = 0; i < count; ++i) {
            const Check *c = checks + i;
            printf("C\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRIu64 "\t%" PRId64 "\t%" PRId64 "\t", i,
                   c->declaration_index, c->kind, c->case_count, c->line, c->column);
            hex(c->name, c->name_count); putchar('\t'); hex(c->source, c->source_count); putchar('\n');
            if (c->binding_count < 0 || c->binding_count > 64 || (!c->bindings && c->binding_count)) return 3;
            for (int64_t b = 0; b < c->binding_count; ++b) {
                const Binding *d = c->bindings + b;
                printf("B\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRIu64 "\t%" PRId64 "\t",
                       i, b, d->kind, d->width, d->is_signed, d->first, d->count, d->value_count);
                hex(d->name, d->name_count); putchar('\t'); hex(d->type_name, d->type_name_count); putchar('\n');
                for (int64_t e = 0; e < d->value_count; ++e) {
                    printf("E\t%" PRId64 "\t%" PRId64 "\t%" PRId64 "\t%" PRIu64 "\t", i, b, e, d->values[e].bits);
                    hex(d->values[e].label, d->values[e].label_count); putchar('\n');
                }
            }
        }
        return 0;
    }
    if (!strcmp(argv[1], "guards")) {
        uint64_t values[65] = {0};
        for (int64_t i = 0; i < count; ++i) {
            int64_t n = checks[i].binding_count;
            if (n < 0 || n > 64 || !checks[i].run_values) return 3;
            if (checks[i].run_values(values, n + 1) != -1) return 4;
            if (n && checks[i].run_values(NULL, n) != -1) return 5;
            if (n && checks[i].run_values(values, n - 1) != -1) return 6;
        }
        printf("guarded %" PRId64 " original wrappers\n", count);
        return 0;
    }
    if (!strcmp(argv[1], "vectors") && argc == 3) {
        FILE *file = fopen(argv[2], "r");
        if (!file) return 7;
        int64_t ordinal, arity, calls = 0;
        while (fscanf(file, "%" SCNd64 " %" SCNd64, &ordinal, &arity) == 2) {
            uint64_t values[64];
            if (ordinal < 0 || ordinal >= count || arity < 0 || arity > 64) return 8;
            for (int64_t a = 0; a < arity; ++a)
                if (fscanf(file, "%" SCNu64, values + a) != 1) return 9;
            if (checks[ordinal].run_values(values, arity) != 0) return 10;
            ++calls;
        }
        if (!feof(file)) return 11;
        fclose(file);
        printf("ran %" PRId64 " original body calls\n", calls);
        return 0;
    }
    return 2;
}
