/* Native Psionic Metal HTTP inference server for Inkling 2 Mini.
   Serves /health, /v1/models, /v1/chat/completions (OpenAI SSE & JSON with
   reasoning_content), and /v1/responses (Open Responses SSE & JSON) directly
   on Psionic's Coax-compiled Metal kernels (psionic/lib/inkling/kernels.coax). */

#define _POSIX_C_SOURCE 200809L

#include "inkling-metal.h"

#include <arpa/inet.h>
#include <ctype.h>
#include <errno.h>
#include <netinet/in.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <strings.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

static volatile sig_atomic_t g_stop = 0;
static void on_sig(int sig) { (void)sig; g_stop = 1; }

static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}

/* ---------------- Dynamic byte buffer ---------------- */

typedef struct {
    char  *data;
    size_t len;
    size_t cap;
} Buf;

static void buf_free(Buf *b) { free(b->data); b->data = NULL; b->len = b->cap = 0; }

static int buf_reserve(Buf *b, size_t extra) {
    if (b->len + extra + 1 <= b->cap) return 0;
    size_t nc = b->cap ? b->cap * 2 : 1024;
    while (nc < b->len + extra + 1) nc *= 2;
    char *p = (char *)realloc(b->data, nc);
    if (!p) return 1;
    b->data = p;
    b->cap = nc;
    return 0;
}

static int buf_append(Buf *b, const char *s, size_t n) {
    if (buf_reserve(b, n)) return 1;
    if (n > 0) memcpy(b->data + b->len, s, n);
    b->len += n;
    b->data[b->len] = '\0';
    return 0;
}

static int buf_adds(Buf *b, const char *s) { return buf_append(b, s, strlen(s)); }

static int buf_addc(Buf *b, char c) { return buf_append(b, &c, 1); }

static int buf_add_json_escaped(Buf *b, const char *s, size_t n) {
    for (size_t i = 0; i < n; i++) {
        unsigned char c = (unsigned char)s[i];
        if (c == '"') { if (buf_adds(b, "\\\"")) return 1; }
        else if (c == '\\') { if (buf_adds(b, "\\\\")) return 1; }
        else if (c == '\n') { if (buf_adds(b, "\\n")) return 1; }
        else if (c == '\r') { if (buf_adds(b, "\\r")) return 1; }
        else if (c == '\t') { if (buf_adds(b, "\\t")) return 1; }
        else if (c < 0x20) {
            char esc[8];
            snprintf(esc, sizeof esc, "\\u%04x", (unsigned)c);
            if (buf_adds(b, esc)) return 1;
        } else {
            if (buf_addc(b, (char)c)) return 1;
        }
    }
    return 0;
}

/* ---------------- Socket I/O helpers ---------------- */

static int send_all(int fd, const char *buf, size_t len) {
    size_t sent = 0;
    while (sent < len) {
        ssize_t w = send(fd, buf + sent, len - sent, 0);
        if (w < 0) {
            if (errno == EINTR) continue;
            return 1;
        }
        if (w == 0) return 1;
        sent += (size_t)w;
    }
    return 0;
}

static int send_http_json(int fd, int status, const char *reason, const char *body, size_t body_len) {
    char hdr[512];
    int n = snprintf(hdr, sizeof hdr,
                     "HTTP/1.1 %d %s\r\n"
                     "Content-Type: application/json\r\n"
                     "Content-Length: %zu\r\n"
                     "Connection: close\r\n"
                     "Access-Control-Allow-Origin: *\r\n"
                     "\r\n",
                     status, reason, body_len);
    if (send_all(fd, hdr, (size_t)n)) return 1;
    return body_len ? send_all(fd, body, body_len) : 0;
}

static int send_sse_headers(int fd) {
    const char *hdr =
        "HTTP/1.1 200 OK\r\n"
        "Content-Type: text/event-stream\r\n"
        "Cache-Control: no-cache\r\n"
        "Connection: close\r\n"
        "Access-Control-Allow-Origin: *\r\n"
        "\r\n";
    return send_all(fd, hdr, strlen(hdr));
}

static int send_sse_event(int fd, const char *event_name, const char *data, size_t data_len) {
    Buf out = {0};
    if (event_name && event_name[0]) {
        buf_adds(&out, "event: ");
        buf_adds(&out, event_name);
        buf_adds(&out, "\n");
    }
    buf_adds(&out, "data: ");
    buf_append(&out, data, data_len);
    buf_adds(&out, "\n\n");
    int rc = send_all(fd, out.data, out.len);
    buf_free(&out);
    return rc;
}

/* ---------------- Minimal JSON parser ---------------- */

typedef struct {
    const char *s;
    size_t      len;
    size_t      pos;
} JScan;

static void j_ws(JScan *j) {
    while (j->pos < j->len && (j->s[j->pos] == ' ' || j->s[j->pos] == '\t' ||
                               j->s[j->pos] == '\r' || j->s[j->pos] == '\n')) {
        j->pos++;
    }
}

static int hex_val(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static void buf_add_utf8(Buf *b, uint32_t cp) {
    if (cp < 0x80) {
        buf_addc(b, (char)cp);
    } else if (cp < 0x800) {
        buf_addc(b, (char)(0xC0 | (cp >> 6)));
        buf_addc(b, (char)(0x80 | (cp & 0x3F)));
    } else if (cp < 0x10000) {
        buf_addc(b, (char)(0xE0 | (cp >> 12)));
        buf_addc(b, (char)(0x80 | ((cp >> 6) & 0x3F)));
        buf_addc(b, (char)(0x80 | (cp & 0x3F)));
    } else {
        buf_addc(b, (char)(0xF0 | (cp >> 18)));
        buf_addc(b, (char)(0x80 | ((cp >> 12) & 0x3F)));
        buf_addc(b, (char)(0x80 | ((cp >> 6) & 0x3F)));
        buf_addc(b, (char)(0x80 | (cp & 0x3F)));
    }
}

static int j_string(JScan *j, Buf *out) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '"') return 1;
    j->pos++;
    while (j->pos < j->len) {
        char c = j->s[j->pos++];
        if (c == '"') return 0;
        if (c == '\\') {
            if (j->pos >= j->len) return 1;
            char e = j->s[j->pos++];
            if (e == '"' || e == '\\' || e == '/') { if (out) buf_addc(out, e); }
            else if (e == 'b') { if (out) buf_addc(out, '\b'); }
            else if (e == 'f') { if (out) buf_addc(out, '\f'); }
            else if (e == 'n') { if (out) buf_addc(out, '\n'); }
            else if (e == 'r') { if (out) buf_addc(out, '\r'); }
            else if (e == 't') { if (out) buf_addc(out, '\t'); }
            else if (e == 'u' && j->pos + 4 <= j->len) {
                int h0 = hex_val(j->s[j->pos]), h1 = hex_val(j->s[j->pos + 1]);
                int h2 = hex_val(j->s[j->pos + 2]), h3 = hex_val(j->s[j->pos + 3]);
                if (h0 < 0 || h1 < 0 || h2 < 0 || h3 < 0) return 1;
                uint32_t cp = (uint32_t)((h0 << 12) | (h1 << 8) | (h2 << 4) | h3);
                j->pos += 4;
                if (cp >= 0xD800 && cp <= 0xDBFF && j->pos + 6 <= j->len &&
                    j->s[j->pos] == '\\' && j->s[j->pos + 1] == 'u') {
                    int l0 = hex_val(j->s[j->pos + 2]), l1 = hex_val(j->s[j->pos + 3]);
                    int l2 = hex_val(j->s[j->pos + 4]), l3 = hex_val(j->s[j->pos + 5]);
                    if (l0 >= 0 && l1 >= 0 && l2 >= 0 && l3 >= 0) {
                        uint32_t lo = (uint32_t)((l0 << 12) | (l1 << 8) | (l2 << 4) | l3);
                        if (lo >= 0xDC00 && lo <= 0xDFFF) {
                            cp = 0x10000 + (((cp - 0xD800) << 10) | (lo - 0xDC00));
                            j->pos += 6;
                        }
                    }
                }
                if (out) buf_add_utf8(out, cp);
            } else {
                if (out) buf_addc(out, e);
            }
        } else {
            if (out) buf_addc(out, c);
        }
    }
    return 1;
}

static int j_skip(JScan *j) {
    j_ws(j);
    if (j->pos >= j->len) return 1;
    char c = j->s[j->pos];
    if (c == '"') return j_string(j, NULL);
    if (c == '{') {
        j->pos++;
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == '}') { j->pos++; return 0; }
        while (j->pos < j->len) {
            if (j_string(j, NULL)) return 1;
            j_ws(j);
            if (j->pos >= j->len || j->s[j->pos] != ':') return 1;
            j->pos++;
            if (j_skip(j)) return 1;
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == ',') { j->pos++; continue; }
            if (j->pos < j->len && j->s[j->pos] == '}') { j->pos++; return 0; }
            return 1;
        }
        return 1;
    }
    if (c == '[') {
        j->pos++;
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ']') { j->pos++; return 0; }
        while (j->pos < j->len) {
            if (j_skip(j)) return 1;
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == ',') { j->pos++; continue; }
            if (j->pos < j->len && j->s[j->pos] == ']') { j->pos++; return 0; }
            return 1;
        }
        return 1;
    }
    while (j->pos < j->len) {
        char d = j->s[j->pos];
        if (d == ',' || d == '}' || d == ']' || d == ' ' || d == '\t' || d == '\r' || d == '\n') break;
        j->pos++;
    }
    return 0;
}

static int j_bool(JScan *j, int *out) {
    j_ws(j);
    if (j->pos + 4 <= j->len && memcmp(j->s + j->pos, "true", 4) == 0) {
        j->pos += 4; *out = 1; return 0;
    }
    if (j->pos + 5 <= j->len && memcmp(j->s + j->pos, "false", 5) == 0) {
        j->pos += 5; *out = 0; return 0;
    }
    return j_skip(j);
}

static int j_number(JScan *j, double *out) {
    j_ws(j);
    if (j->pos >= j->len) return 1;
    const char *start = j->s + j->pos;
    char *end = NULL;
    double v = strtod(start, &end);
    if (end == start) return j_skip(j);
    j->pos = (size_t)(end - j->s);
    *out = v;
    return 0;
}

/* ---------------- Request parsing & Inkling chat template ---------------- */

typedef struct {
    Buf role;
    Buf content;
    Buf reasoning;
} ChatMsg;

typedef struct {
    ChatMsg *msgs;
    int      n_msgs;
    int      cap_msgs;
    Buf      model;
    Buf      effort;
    int      stream;
    int      max_tokens;
    float    temperature;
    float    top_p;
    int      top_k;
} Req;

static void req_free(Req *r) {
    for (int i = 0; i < r->n_msgs; i++) {
        buf_free(&r->msgs[i].role);
        buf_free(&r->msgs[i].content);
        buf_free(&r->msgs[i].reasoning);
    }
    free(r->msgs);
    buf_free(&r->model);
    buf_free(&r->effort);
}

static ChatMsg *req_add_msg(Req *r) {
    if (r->n_msgs >= r->cap_msgs) {
        int nc = r->cap_msgs ? r->cap_msgs * 2 : 8;
        ChatMsg *p = (ChatMsg *)realloc(r->msgs, (size_t)nc * sizeof *p);
        if (!p) return NULL;
        r->msgs = p;
        r->cap_msgs = nc;
    }
    ChatMsg *m = &r->msgs[r->n_msgs++];
    memset(m, 0, sizeof *m);
    return m;
}

static int parse_content_value(JScan *j, Buf *out) {
    j_ws(j);
    if (j->pos >= j->len) return 1;
    if (j->s[j->pos] == '"') return j_string(j, out);
    if (j->s[j->pos] == '[') {
        j->pos++;
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ']') { j->pos++; return 0; }
        while (j->pos < j->len) {
            j_ws(j);
            if (j->s[j->pos] == '"') {
                if (j_string(j, out)) return 1;
            } else if (j->s[j->pos] == '{') {
                j->pos++;
                j_ws(j);
                while (j->pos < j->len && j->s[j->pos] != '}') {
                    Buf k = {0};
                    if (j_string(j, &k)) { buf_free(&k); return 1; }
                    j_ws(j);
                    if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); return 1; }
                    j->pos++;
                    if (k.data && (!strcmp(k.data, "text") || !strcmp(k.data, "input_text"))) {
                        j_ws(j);
                        if (j->pos < j->len && j->s[j->pos] == '"') {
                            if (j_string(j, out)) { buf_free(&k); return 1; }
                        } else if (j_skip(j)) { buf_free(&k); return 1; }
                    } else {
                        if (j_skip(j)) { buf_free(&k); return 1; }
                    }
                    buf_free(&k);
                    j_ws(j);
                    if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
                }
                if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
            } else {
                if (j_skip(j)) return 1;
            }
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == ',') { j->pos++; continue; }
            if (j->pos < j->len && j->s[j->pos] == ']') { j->pos++; return 0; }
            return 1;
        }
        return 1;
    }
    return j_skip(j);
}

static int parse_message_obj(JScan *j, Req *r) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '{') return j_skip(j);
    j->pos++;
    ChatMsg *m = req_add_msg(r);
    if (!m) return 1;
    j_ws(j);
    while (j->pos < j->len && j->s[j->pos] != '}') {
        Buf k = {0};
        if (j_string(j, &k)) { buf_free(&k); return 1; }
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); return 1; }
        j->pos++;
        if (k.data && !strcmp(k.data, "role")) {
            j_string(j, &m->role);
        } else if (k.data && !strcmp(k.data, "content")) {
            parse_content_value(j, &m->content);
        } else if (k.data && (!strcmp(k.data, "reasoning_content") || !strcmp(k.data, "reasoning"))) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &m->reasoning);
            else j_skip(j);
        } else {
            j_skip(j);
        }
        buf_free(&k);
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
    if (!m->role.len) buf_adds(&m->role, "user");
    return 0;
}

static int parse_request_json(const char *body, size_t body_len, Req *r) {
    memset(r, 0, sizeof *r);
    buf_adds(&r->model, "inkling-2-mini");
    buf_adds(&r->effort, "0.7");
    r->max_tokens = 2048;
    r->temperature = 0.0f;
    r->top_p = 0.95f;
    r->top_k = 40;

    JScan j = { body, body_len, 0 };
    j_ws(&j);
    if (j.pos >= j.len || j.s[j.pos] != '{') return 1;
    j.pos++;
    j_ws(&j);
    while (j.pos < j.len && j.s[j.pos] != '}') {
        Buf k = {0};
        if (j_string(&j, &k)) { buf_free(&k); return 1; }
        j_ws(&j);
        if (j.pos >= j.len || j.s[j.pos] != ':') { buf_free(&k); return 1; }
        j.pos++;
        const char *ks = k.data ? k.data : "";
        if (!strcmp(ks, "model")) {
            r->model.len = 0;
            j_string(&j, &r->model);
        } else if (!strcmp(ks, "stream")) {
            j_bool(&j, &r->stream);
        } else if (!strcmp(ks, "max_tokens") || !strcmp(ks, "max_completion_tokens") || !strcmp(ks, "max_output_tokens")) {
            double v = 0;
            if (!j_number(&j, &v) && v > 0) r->max_tokens = (int)v;
        } else if (!strcmp(ks, "temperature")) {
            double v = 0;
            if (!j_number(&j, &v) && v >= 0.0) r->temperature = (float)v;
        } else if (!strcmp(ks, "top_p")) {
            double v = 0.95;
            if (!j_number(&j, &v) && v > 0.0) r->top_p = (float)v;
        } else if (!strcmp(ks, "top_k")) {
            double v = 40;
            if (!j_number(&j, &v) && v >= 0) r->top_k = (int)v;
        } else if (!strcmp(ks, "reasoning_effort")) {
            Buf ev = {0};
            j_ws(&j);
            if (j.pos < j.len && j.s[j.pos] == '"') {
                j_string(&j, &ev);
                if (ev.data) {
                    r->effort.len = 0;
                    if (!strcmp(ev.data, "none")) buf_adds(&r->effort, "0");
                    else if (!strcmp(ev.data, "minimal")) buf_adds(&r->effort, "0.1");
                    else if (!strcmp(ev.data, "low")) buf_adds(&r->effort, "0.2");
                    else if (!strcmp(ev.data, "medium")) buf_adds(&r->effort, "0.7");
                    else if (!strcmp(ev.data, "high")) buf_adds(&r->effort, "0.9");
                    else if (!strcmp(ev.data, "xhigh") || !strcmp(ev.data, "max")) buf_adds(&r->effort, "0.99");
                    else buf_adds(&r->effort, "0.7");
                }
            } else {
                j_skip(&j);
            }
            buf_free(&ev);
        } else if (!strcmp(ks, "instructions")) {
            ChatMsg *m = req_add_msg(r);
            if (m) {
                buf_adds(&m->role, "system");
                parse_content_value(&j, &m->content);
            } else {
                j_skip(&j);
            }
        } else if (!strcmp(ks, "messages") || !strcmp(ks, "input")) {
            j_ws(&j);
            if (j.pos < j.len && j.s[j.pos] == '"') {
                ChatMsg *m = req_add_msg(r);
                if (m) {
                    buf_adds(&m->role, "user");
                    j_string(&j, &m->content);
                } else {
                    j_skip(&j);
                }
            } else if (j.pos < j.len && j.s[j.pos] == '[') {
                j.pos++;
                j_ws(&j);
                while (j.pos < j.len && j.s[j.pos] != ']') {
                    if (parse_message_obj(&j, r)) { buf_free(&k); return 1; }
                    j_ws(&j);
                    if (j.pos < j.len && j.s[j.pos] == ',') j.pos++;
                }
                if (j.pos < j.len && j.s[j.pos] == ']') j.pos++;
            } else {
                j_skip(&j);
            }
        } else {
            j_skip(&j);
        }
        buf_free(&k);
        j_ws(&j);
        if (j.pos < j.len && j.s[j.pos] == ',') j.pos++;
    }
    return 0;
}

static void format_inkling_prompt(const Req *r, Buf *out) {
    int effort_emitted = 0;
    const char *eff = (r->effort.data && r->effort.len) ? r->effort.data : "0.7";

    for (int i = 0; i < r->n_msgs; i++) {
        const ChatMsg *m = &r->msgs[i];
        const char *role = m->role.data ? m->role.data : "user";
        int is_sys = (!strcmp(role, "system") || !strcmp(role, "developer"));
        if (!effort_emitted && !is_sys) {
            buf_adds(out, "<|message_system|><|content_text|>Thinking effort level: ");
            buf_adds(out, eff);
            buf_adds(out, "<|end_message|>");
            effort_emitted = 1;
        }
        const char *rtok = "<|message_user|>";
        if (is_sys) rtok = "<|message_system|>";
        else if (!strcmp(role, "assistant")) rtok = "<|message_model|>";
        else if (!strcmp(role, "tool")) rtok = "<|message_tool|>";

        if (!strcmp(role, "assistant") && m->reasoning.len > 0) {
            buf_adds(out, rtok);
            buf_adds(out, "<|content_thinking|>");
            buf_append(out, m->reasoning.data, m->reasoning.len);
            buf_adds(out, "<|end_message|>");
        }
        if (m->content.len > 0 || strcmp(role, "assistant") != 0 || m->reasoning.len == 0) {
            buf_adds(out, rtok);
            buf_adds(out, "<|content_text|>");
            if (m->content.len > 0) buf_append(out, m->content.data, m->content.len);
            buf_adds(out, "<|end_message|>");
        }
        if (!strcmp(role, "assistant")) {
            buf_adds(out, "<|content_model_end_sampling|>");
        }
    }
    if (!effort_emitted) {
        buf_adds(out, "<|message_system|><|content_text|>Thinking effort level: ");
        buf_adds(out, eff);
        buf_adds(out, "<|end_message|>");
    }
}

/* ---------------- UTF-8 safe incremental emitter ---------------- */

/* Returns how many leading bytes of (buf, len) form complete UTF-8 characters. */
static size_t utf8_complete_prefix(const unsigned char *s, size_t n) {
    if (n == 0) return 0;
    size_t i = n;
    /* Walk back up to 3 continuation bytes (10xxxxxx). */
    while (i > 0 && (n - i) < 4 && (s[i - 1] & 0xC0) == 0x80) i--;
    if (i == 0) return n; /* stray continuation bytes: flush them */
    unsigned char lead = s[i - 1];
    size_t need = 1;
    if (lead < 0x80) need = 1;
    else if ((lead & 0xE0) == 0xC0) need = 2;
    else if ((lead & 0xF0) == 0xE0) need = 3;
    else if ((lead & 0xF8) == 0xF0) need = 4;
    else return n; /* invalid lead: flush */
    size_t have = n - (i - 1);
    return (have >= need) ? n : (i - 1);
}

/* ---------------- Generation & HTTP handlers ---------------- */

typedef struct {
    InkSession *sess;
    int32_t     tok_model;
    int32_t     tok_thinking;
    int32_t     tok_text;
    int32_t     tok_end_msg;
    int32_t     tok_eog;
    uint64_t    rng;
    int         req_seq;
} Server;

static void emit_chat_delta(int fd, int64_t created, const char *model, int is_thinking, const char *s, size_t n) {
    if (n == 0) return;
    Buf ev = {0};
    char head[256];
    snprintf(head, sizeof head,
             "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion.chunk\",\"created\":%lld,\"model\":\"",
             (long long)created);
    buf_adds(&ev, head);
    buf_add_json_escaped(&ev, model, strlen(model));
    buf_adds(&ev, "\",\"choices\":[{\"index\":0,\"delta\":{\"");
    buf_adds(&ev, is_thinking ? "reasoning_content" : "content");
    buf_adds(&ev, "\":\"");
    buf_add_json_escaped(&ev, s, n);
    buf_adds(&ev, "\"},\"finish_reason\":null}]}");
    send_sse_event(fd, NULL, ev.data, ev.len);
    buf_free(&ev);
}

static void emit_responses_delta(int fd, int is_thinking, const char *s, size_t n) {
    if (n == 0) return;
    Buf ev = {0};
    const char *etype = is_thinking ? "response.reasoning_summary_text.delta" : "response.output_text.delta";
    buf_adds(&ev, "{\"type\":\"");
    buf_adds(&ev, etype);
    buf_adds(&ev, "\",\"delta\":\"");
    buf_add_json_escaped(&ev, s, n);
    buf_adds(&ev, "\"}");
    send_sse_event(fd, etype, ev.data, ev.len);
    buf_free(&ev);
}

static void handle_inference(Server *sv, int fd, int is_responses, const char *body, size_t body_len) {
    Req r;
    if (parse_request_json(body, body_len, &r) != 0) {
        const char *e = "{\"error\":{\"message\":\"invalid JSON request\"}}";
        send_http_json(fd, 400, "Bad Request", e, strlen(e));
        req_free(&r);
        return;
    }

    Buf prompt = {0};
    format_inkling_prompt(&r, &prompt);

    int max_ctx = ink_max_ctx(sv->sess);
    int32_t *toks = (int32_t *)malloc((size_t)max_ctx * sizeof(int32_t));
    if (!toks) {
        const char *e = "{\"error\":{\"message\":\"out of memory\"}}";
        send_http_json(fd, 500, "Internal Server Error", e, strlen(e));
        buf_free(&prompt);
        req_free(&r);
        return;
    }
    int n_prompt = ink_encode(sv->sess, prompt.data ? prompt.data : "", prompt.len, toks, max_ctx - 1, 1);
    if (n_prompt <= 0) {
        const char *e = "{\"error\":{\"message\":\"empty prompt\"}}";
        send_http_json(fd, 400, "Bad Request", e, strlen(e));
        free(toks); buf_free(&prompt); req_free(&r);
        return;
    }

    int64_t created = (int64_t)time(NULL);
    const char *model = (r.model.data && r.model.len) ? r.model.data : "inkling-2-mini";
    sv->req_seq++;
    fprintf(stderr, "[psionic-inkling-serve] req#%d %s stream=%d prompt=%d starting\n",
            sv->req_seq, is_responses ? "/v1/responses" : "/v1/chat/completions", r.stream, n_prompt);
    fflush(stderr);

    if (r.stream) {
        if (send_sse_headers(fd) != 0) {
            free(toks); buf_free(&prompt); req_free(&r);
            return;
        }
    }

    ink_reset(sv->sess);
    double t0 = now_sec();
    for (int i = 0; i < n_prompt && !g_stop; ) {
        int chunk = n_prompt - i;
        if (chunk > 3072) chunk = 3072;
        int need_logits = (i + chunk == n_prompt);
        if (ink_prefill(sv->sess, toks + i, chunk, need_logits) != 0) {
            if (!r.stream) {
                char ebuf[512];
                snprintf(ebuf, sizeof ebuf, "{\"error\":{\"message\":\"prefill failed: %s\"}}", ink_error(sv->sess));
                send_http_json(fd, 500, "Internal Server Error", ebuf, strlen(ebuf));
            }
            free(toks); buf_free(&prompt); req_free(&r);
            return;
        }
        i += chunk;
        if (r.stream && i < n_prompt) {
            if (send_all(fd, ": prefilling\n\n", 14) != 0) {
                free(toks); buf_free(&prompt); req_free(&r);
                return;
            }
        }
    }
    double t_prefill = now_sec() - t0;
    free(toks);
    buf_free(&prompt);

    if (r.stream) {
        if (!is_responses) {
            Buf init = {0};
            char head[256];
            snprintf(head, sizeof head,
                     "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion.chunk\",\"created\":%lld,\"model\":\"",
                     (long long)created);
            buf_adds(&init, head);
            buf_add_json_escaped(&init, model, strlen(model));
            buf_adds(&init, "\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\"},\"finish_reason\":null}]}");
            send_sse_event(fd, NULL, init.data, init.len);
            buf_free(&init);
        }
    }

    /* Channel state machine: 0=none, 1=thinking, 2=text */
    int channel = 0;
    Buf pending = {0};
    Buf full_text = {0};
    Buf full_think = {0};
    InkSampleParams sp = { .temperature = r.temperature, .top_p = r.top_p, .top_k = r.top_k };

    int max_gen = r.max_tokens;
    int avail = max_ctx - n_prompt - 1;
    if (avail < 1) avail = 1;
    if (max_gen > avail) max_gen = avail;

    int n_gen = 0;
    const char *finish_reason = "length";
    double td0 = now_sec();

    for (int i = 0; i < max_gen && !g_stop; i++) {
        int32_t tok = ink_sample(sv->sess, &sp, &sv->rng);
        if (tok < 0) break;
        n_gen++;

        if (tok == sv->tok_eog || ink_is_eog(sv->sess, tok)) {
            finish_reason = "stop";
            break;
        }

        if (tok == sv->tok_model || tok == sv->tok_end_msg) {
            if (pending.len > 0 && channel != 0) {
                if (channel == 1) buf_append(&full_think, pending.data, pending.len);
                else buf_append(&full_text, pending.data, pending.len);
                if (r.stream) {
                    if (is_responses) emit_responses_delta(fd, channel == 1, pending.data, pending.len);
                    else emit_chat_delta(fd, created, model, channel == 1, pending.data, pending.len);
                }
                pending.len = 0;
            }
            channel = 0;
        } else if (tok == sv->tok_thinking) {
            channel = 1;
            pending.len = 0;
        } else if (tok == sv->tok_text) {
            channel = 2;
            pending.len = 0;
        } else if (tok >= 200000) {
            /* Other special control token: ignore in user-visible output */
        } else {
            if (channel == 0) channel = 2; /* fallback if model emits text directly */
            char piece[512];
            int plen = ink_decode_piece(sv->sess, tok, piece, sizeof piece);
            if (plen > 0) {
                buf_append(&pending, piece, (size_t)plen);
                size_t ready = utf8_complete_prefix((const unsigned char *)pending.data, pending.len);
                if (ready > 0) {
                    if (channel == 1) buf_append(&full_think, pending.data, ready);
                    else buf_append(&full_text, pending.data, ready);
                    if (r.stream) {
                        if (is_responses) emit_responses_delta(fd, channel == 1, pending.data, ready);
                        else emit_chat_delta(fd, created, model, channel == 1, pending.data, ready);
                    }
                    if (ready < pending.len) {
                        memmove(pending.data, pending.data + ready, pending.len - ready);
                    }
                    pending.len -= ready;
                }
            }
        }

        if (i + 1 < max_gen) {
            if (ink_step(sv->sess, tok, 1) != 0) {
                fprintf(stderr, "[psionic-inkling-serve] step error: %s\n", ink_error(sv->sess));
                break;
            }
        }
    }
    if (pending.len > 0 && channel != 0) {
        if (channel == 1) buf_append(&full_think, pending.data, pending.len);
        else buf_append(&full_text, pending.data, pending.len);
        if (r.stream) {
            if (is_responses) emit_responses_delta(fd, channel == 1, pending.data, pending.len);
            else emit_chat_delta(fd, created, model, channel == 1, pending.data, pending.len);
        }
    }
    double t_decode = now_sec() - td0;
    fprintf(stderr, "[psionic-inkling-serve] req#%d %s stream=%d prompt=%d (%.2fs) gen=%d (%.2fs, %.1f tok/s) finish=%s\n",
            sv->req_seq, is_responses ? "/v1/responses" : "/v1/chat/completions", r.stream,
            n_prompt, t_prefill, n_gen, t_decode, n_gen / (t_decode + 1e-9), finish_reason);

    if (r.stream) {
        if (is_responses) {
            Buf done = {0};
            char u[256];
            snprintf(u, sizeof u,
                     "{\"type\":\"response.completed\",\"response\":{\"id\":\"resp-psionic-%d\",\"status\":\"completed\",\"usage\":{\"input_tokens\":%d,\"output_tokens\":%d,\"total_tokens\":%d}}}",
                     sv->req_seq, n_prompt, n_gen, n_prompt + n_gen);
            buf_adds(&done, u);
            send_sse_event(fd, "response.completed", done.data, done.len);
            buf_free(&done);
        } else {
            Buf fin = {0};
            char head[256];
            snprintf(head, sizeof head,
                     "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion.chunk\",\"created\":%lld,\"model\":\"",
                     (long long)created);
            buf_adds(&fin, head);
            buf_add_json_escaped(&fin, model, strlen(model));
            buf_adds(&fin, "\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"");
            buf_adds(&fin, finish_reason);
            buf_adds(&fin, "\"}]}");
            send_sse_event(fd, NULL, fin.data, fin.len);
            buf_free(&fin);

            Buf usg = {0};
            snprintf(head, sizeof head,
                     "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion.chunk\",\"created\":%lld,\"model\":\"",
                     (long long)created);
            buf_adds(&usg, head);
            buf_add_json_escaped(&usg, model, strlen(model));
            char utail[160];
            snprintf(utail, sizeof utail,
                     "\",\"choices\":[],\"usage\":{\"prompt_tokens\":%d,\"completion_tokens\":%d,\"total_tokens\":%d}}",
                     n_prompt, n_gen, n_prompt + n_gen);
            buf_adds(&usg, utail);
            send_sse_event(fd, NULL, usg.data, usg.len);
            buf_free(&usg);

            send_sse_event(fd, NULL, "[DONE]", 6);
        }
    } else if (is_responses) {
        Buf out = {0};
        char head[256];
        snprintf(head, sizeof head,
                 "{\"id\":\"resp-psionic-%d\",\"object\":\"response\",\"status\":\"completed\",\"model\":\"",
                 sv->req_seq);
        buf_adds(&out, head);
        buf_add_json_escaped(&out, model, strlen(model));
        buf_adds(&out, "\",\"output_text\":\"");
        if (full_text.len) buf_add_json_escaped(&out, full_text.data, full_text.len);
        buf_adds(&out, "\",\"output\":[{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"");
        if (full_text.len) buf_add_json_escaped(&out, full_text.data, full_text.len);
        char utail[160];
        snprintf(utail, sizeof utail,
                 "\"}]}],\"usage\":{\"input_tokens\":%d,\"output_tokens\":%d,\"total_tokens\":%d}}",
                 n_prompt, n_gen, n_prompt + n_gen);
        buf_adds(&out, utail);
        send_http_json(fd, 200, "OK", out.data, out.len);
        buf_free(&out);
    } else {
        Buf out = {0};
        char head[256];
        snprintf(head, sizeof head,
                 "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion\",\"created\":%lld,\"model\":\"",
                 (long long)created);
        buf_adds(&out, head);
        buf_add_json_escaped(&out, model, strlen(model));
        buf_adds(&out, "\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"");
        if (full_text.len) buf_add_json_escaped(&out, full_text.data, full_text.len);
        buf_adds(&out, "\"");
        if (full_think.len) {
            buf_adds(&out, ",\"reasoning_content\":\"");
            buf_add_json_escaped(&out, full_think.data, full_think.len);
            buf_adds(&out, "\"");
        }
        buf_adds(&out, "},\"finish_reason\":\"");
        buf_adds(&out, finish_reason);
        char utail[160];
        snprintf(utail, sizeof utail,
                 "\"}],\"usage\":{\"prompt_tokens\":%d,\"completion_tokens\":%d,\"total_tokens\":%d}}",
                 n_prompt, n_gen, n_prompt + n_gen);
        buf_adds(&out, utail);
        send_http_json(fd, 200, "OK", out.data, out.len);
        buf_free(&out);
    }

    buf_free(&pending);
    buf_free(&full_text);
    buf_free(&full_think);
    req_free(&r);
}

static int read_http_request(int fd, char *method, size_t mcap, char *path, size_t pcap, Buf *body) {
    Buf raw = {0};
    char tmp[8192];
    size_t hdr_end = 0;
    size_t content_len = 0;
    int have_hdr = 0;

    while (raw.len < 4 * 1024 * 1024) {
        ssize_t n = recv(fd, tmp, sizeof tmp, 0);
        if (n < 0) { if (errno == EINTR) continue; buf_free(&raw); return 1; }
        if (n == 0) break;
        if (buf_append(&raw, tmp, (size_t)n)) { buf_free(&raw); return 1; }
        if (!have_hdr) {
            char *p = strstr(raw.data, "\r\n\r\n");
            if (p) {
                have_hdr = 1;
                hdr_end = (size_t)(p - raw.data) + 4;
                /* Parse Content-Length and Expect: 100-continue case-insensitively */
                int expect_100 = 0;
                const char *line = raw.data;
                while (line < p) {
                    const char *eol = strstr(line, "\r\n");
                    if (!eol || eol > p) break;
                    if ((size_t)(eol - line) > 15 && strncasecmp(line, "Content-Length:", 15) == 0) {
                        content_len = (size_t)strtoull(line + 15, NULL, 10);
                    } else if ((size_t)(eol - line) >= 7 && strncasecmp(line, "Expect:", 7) == 0) {
                        const char *v = line + 7;
                        while (v < eol && (*v == ' ' || *v == '\t')) v++;
                        if ((size_t)(eol - v) >= 12 && strncasecmp(v, "100-continue", 12) == 0) {
                            expect_100 = 1;
                        }
                    }
                    line = eol + 2;
                }
                if (expect_100) {
                    send_all(fd, "HTTP/1.1 100 Continue\r\n\r\n", 25);
                }
            }
        }
        if (have_hdr && raw.len >= hdr_end + content_len) break;
    }
    if (!have_hdr) { buf_free(&raw); return 1; }

    /* Request line: METHOD PATH HTTP/1.x */
    const char *sp1 = strchr(raw.data, ' ');
    if (!sp1 || (size_t)(sp1 - raw.data) >= mcap) { buf_free(&raw); return 1; }
    memcpy(method, raw.data, (size_t)(sp1 - raw.data));
    method[sp1 - raw.data] = '\0';
    const char *pstart = sp1 + 1;
    const char *sp2 = strchr(pstart, ' ');
    if (!sp2) { buf_free(&raw); return 1; }
    size_t plen = (size_t)(sp2 - pstart);
    if (plen >= pcap) plen = pcap - 1;
    memcpy(path, pstart, plen);
    path[plen] = '\0';
    char *q = strchr(path, '?');
    if (q) *q = '\0';

    size_t blen = raw.len > hdr_end ? raw.len - hdr_end : 0;
    if (content_len > 0 && blen > content_len) blen = content_len;
    if (blen > 0) buf_append(body, raw.data + hdr_end, blen);
    buf_free(&raw);
    return 0;
}

int main(int argc, char **argv) {
    const char *gguf = NULL;
    const char *metal = NULL;
    const char *host = "127.0.0.1";
    int port = 18794;
    int max_ctx = 8192;

    for (int i = 1; i < argc; i++) {
        if ((!strcmp(argv[i], "--model") || !strcmp(argv[i], "-m")) && i + 1 < argc) gguf = argv[++i];
        else if (!strcmp(argv[i], "--metal") && i + 1 < argc) metal = argv[++i];
        else if (!strcmp(argv[i], "--host") && i + 1 < argc) host = argv[++i];
        else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
        else if ((!strcmp(argv[i], "--ctx") || !strcmp(argv[i], "--ctx-size")) && i + 1 < argc) max_ctx = atoi(argv[++i]);
    }
    if (!gguf || !metal) {
        fprintf(stderr, "usage: %s --model PATH.gguf --metal PATH.metal [--host 127.0.0.1] [--port 18794] [--ctx 8192]\n", argv[0]);
        return 2;
    }

    signal(SIGPIPE, SIG_IGN);
    signal(SIGINT, on_sig);
    signal(SIGTERM, on_sig);

    double t0 = now_sec();
    Server sv;
    memset(&sv, 0, sizeof sv);
    sv.rng = (uint64_t)time(NULL) ^ 0x9E3779B97F4A7C15ULL;
    char err[512];
    if (ink_open(gguf, metal, max_ctx, &sv.sess, err, sizeof err) != 0) {
        fprintf(stderr, "[psionic-inkling-serve] ink_open failed: %s\n", err);
        return 1;
    }
    sv.tok_model    = ink_find_token(sv.sess, "<|message_model|>");
    sv.tok_thinking = ink_find_token(sv.sess, "<|content_thinking|>");
    sv.tok_text     = ink_find_token(sv.sess, "<|content_text|>");
    sv.tok_end_msg  = ink_find_token(sv.sess, "<|end_message|>");
    sv.tok_eog      = ink_eos_token(sv.sess);
    fprintf(stderr, "[psionic-inkling-serve] loaded %s on Psionic Metal in %.2fs (ctx=%d, eog=%d)\n",
            gguf, now_sec() - t0, max_ctx, sv.tok_eog);

    int srv = socket(AF_INET, SOCK_STREAM, 0);
    if (srv < 0) { perror("socket"); ink_close(sv.sess); return 1; }
    int one = 1;
    setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, &one, sizeof one);

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof addr);
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &addr.sin_addr) != 1) {
        fprintf(stderr, "[psionic-inkling-serve] invalid host %s\n", host);
        close(srv); ink_close(sv.sess); return 1;
    }
    if (bind(srv, (struct sockaddr *)&addr, sizeof addr) != 0) {
        perror("bind"); close(srv); ink_close(sv.sess); return 1;
    }
    if (listen(srv, 16) != 0) {
        perror("listen"); close(srv); ink_close(sv.sess); return 1;
    }
    fprintf(stderr, "[psionic-inkling-serve] listening on http://%s:%d (Psionic Metal)\n", host, port);
    fflush(stderr);

    while (!g_stop) {
        struct sockaddr_in cli;
        socklen_t clen = sizeof cli;
        int cfd = accept(srv, (struct sockaddr *)&cli, &clen);
        if (cfd < 0) {
            if (errno == EINTR) continue;
            break;
        }
        char method[16] = {0}, path[256] = {0};
        Buf body = {0};
        if (read_http_request(cfd, method, sizeof method, path, sizeof path, &body) == 0) {
            if (!strcmp(method, "OPTIONS")) {
                const char *cors =
                    "HTTP/1.1 204 No Content\r\n"
                    "Access-Control-Allow-Origin: *\r\n"
                    "Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n"
                    "Access-Control-Allow-Headers: Content-Type, Authorization\r\n"
                    "Connection: close\r\n\r\n";
                send_all(cfd, cors, strlen(cors));
            } else if (!strcmp(method, "GET") && !strcmp(path, "/health")) {
                const char *ok = "{\"status\":\"ok\",\"engine\":\"psionic-metal\"}";
                send_http_json(cfd, 200, "OK", ok, strlen(ok));
            } else if (!strcmp(method, "GET") && !strcmp(path, "/v1/models")) {
                const char *ms = "{\"object\":\"list\",\"data\":[{\"id\":\"inkling-2-mini\",\"object\":\"model\",\"owned_by\":\"psionic-metal\"}]}";
                send_http_json(cfd, 200, "OK", ms, strlen(ms));
            } else if (!strcmp(method, "POST") && !strcmp(path, "/v1/chat/completions")) {
                handle_inference(&sv, cfd, 0, body.data ? body.data : "", body.len);
            } else if (!strcmp(method, "POST") && !strcmp(path, "/v1/responses")) {
                handle_inference(&sv, cfd, 1, body.data ? body.data : "", body.len);
            } else {
                const char *nf = "{\"error\":{\"message\":\"not found\"}}";
                send_http_json(cfd, 404, "Not Found", nf, strlen(nf));
            }
        }
        buf_free(&body);
        close(cfd);
    }

    close(srv);
    ink_close(sv.sess);
    return 0;
}
