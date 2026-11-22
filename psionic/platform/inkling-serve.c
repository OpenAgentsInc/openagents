/* Native Psionic HTTP inference server for Inkling 2 Mini.
   Serves /health, /v1/models, /v1/chat/completions (OpenAI SSE & JSON with
   reasoning_content), and /v1/responses (Open Responses SSE & JSON) directly
   on Psionic's Coax-compiled kernels. It is written against the engine
   contract (inkling-engine.h) only, so the same source is the server on
   Metal (macOS) and on CUDA (Linux and Windows); the sockets, the clock and
   the stop signal below are the only code that differs by system. */

#ifndef _WIN32
#define _POSIX_C_SOURCE 200809L
#endif

#include "inkling-engine.h"
#include "json-schema.h"

#include <ctype.h>
#include <errno.h>
#include <math.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static volatile sig_atomic_t g_stop = 0;

#ifdef _WIN32
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0600
#endif
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#ifdef _MSC_VER
#include <BaseTsd.h>
typedef SSIZE_T ssize_t;
#define strncasecmp _strnicmp
#else
#include <strings.h>
#endif
typedef CRITICAL_SECTION InkMutex;
static void ink_mutex_init(InkMutex *m) { InitializeCriticalSection(m); }
static void ink_mutex_destroy(InkMutex *m) { DeleteCriticalSection(m); }
static void ink_mutex_lock(InkMutex *m) { EnterCriticalSection(m); }
static void ink_mutex_unlock(InkMutex *m) { LeaveCriticalSection(m); }
typedef void (*InkThreadFn)(void *);
typedef struct { InkThreadFn fn; void *arg; } InkWinThread;
static DWORD WINAPI ink_win_thread(LPVOID p) {
    InkWinThread *t = (InkWinThread *)p;
    InkThreadFn fn = t->fn; void *arg = t->arg;
    free(t);
    fn(arg);
    return 0;
}
static int ink_spawn_detached(InkThreadFn fn, void *arg) {
    InkWinThread *t = (InkWinThread *)malloc(sizeof *t);
    if (!t) return 1;
    t->fn = fn; t->arg = arg;
    HANDLE h = CreateThread(NULL, 2 * 1024 * 1024, ink_win_thread, t, 0, NULL);
    if (!h) { free(t); return 1; }
    CloseHandle(h);
    return 0;
}
/* A SOCKET is kept in an int: Windows hands out handles that fit in 32 bits. */
static int g_listen = -1;
static int sock_start(void) { WSADATA w; return WSAStartup(MAKEWORD(2, 2), &w) == 0 ? 0 : 1; }
static void sock_close(int fd) { closesocket((SOCKET)fd); }
static int sock_interrupted(void) { return WSAGetLastError() == WSAEINTR; }
static ssize_t sock_send(int fd, const char *buf, size_t len) { return send((SOCKET)fd, buf, (int)(len > 1 << 30 ? 1 << 30 : len), 0); }
static ssize_t sock_recv(int fd, char *buf, size_t len) { return recv((SOCKET)fd, buf, (int)len, 0); }
static void sock_error(const char *what) { fprintf(stderr, "%s: Windows socket error %d\n", what, WSAGetLastError()); }
static int sock_poll_read(int fd, int timeout_ms) {
    fd_set rfds;
    FD_ZERO(&rfds);
    FD_SET((SOCKET)fd, &rfds);
    struct timeval tv;
    tv.tv_sec = timeout_ms / 1000;
    tv.tv_usec = (timeout_ms % 1000) * 1000;
    return select(0, &rfds, NULL, NULL, &tv);
}
/* Ctrl+C, Ctrl+Break, a closed console, logoff and shutdown all stop the
   server: closing the listening socket ends the select/accept it is waiting in. */
static BOOL WINAPI on_console(DWORD event) {
    (void)event;
    g_stop = 1;
    if (g_listen >= 0) closesocket((SOCKET)g_listen);
    return TRUE;
}
static void stop_signals(int listen_fd) { g_listen = listen_fd; SetConsoleCtrlHandler(on_console, TRUE); }
static void sock_close_listener(int fd) { if (!g_stop) closesocket((SOCKET)fd); } /* the handler closed it otherwise */
static double now_sec(void) {
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart / (double)f.QuadPart;
}
#else
#include <arpa/inet.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <strings.h>
#include <sys/socket.h>
#include <unistd.h>
typedef pthread_mutex_t InkMutex;
static void ink_mutex_init(InkMutex *m) { pthread_mutex_init(m, NULL); }
static void ink_mutex_destroy(InkMutex *m) { pthread_mutex_destroy(m); }
static void ink_mutex_lock(InkMutex *m) { pthread_mutex_lock(m); }
static void ink_mutex_unlock(InkMutex *m) { pthread_mutex_unlock(m); }
typedef void (*InkThreadFn)(void *);
typedef struct { InkThreadFn fn; void *arg; } InkPosixThread;
static void *ink_posix_thread(void *p) {
    InkPosixThread *t = (InkPosixThread *)p;
    InkThreadFn fn = t->fn; void *arg = t->arg;
    free(t);
    fn(arg);
    return NULL;
}
static int ink_spawn_detached(InkThreadFn fn, void *arg) {
    InkPosixThread *t = (InkPosixThread *)malloc(sizeof *t);
    if (!t) return 1;
    t->fn = fn; t->arg = arg;
    pthread_t tid;
    pthread_attr_t attr;
    pthread_attr_init(&attr);
    pthread_attr_setdetachstate(&attr, PTHREAD_CREATE_DETACHED);
    pthread_attr_setstacksize(&attr, 2 * 1024 * 1024);
    int rc = pthread_create(&tid, &attr, ink_posix_thread, t);
    pthread_attr_destroy(&attr);
    if (rc != 0) { free(t); return 1; }
    return 0;
}
static void on_sig(int sig) { (void)sig; g_stop = 1; }
static int sock_start(void) { signal(SIGPIPE, SIG_IGN); return 0; }
static void sock_close(int fd) { close(fd); }
static int sock_interrupted(void) { return errno == EINTR; }
static ssize_t sock_send(int fd, const char *buf, size_t len) { return send(fd, buf, len, 0); }
static ssize_t sock_recv(int fd, char *buf, size_t len) { return recv(fd, buf, len, 0); }
static void sock_error(const char *what) { perror(what); }
static int sock_poll_read(int fd, int timeout_ms) {
    struct pollfd pfd;
    pfd.fd = fd;
    pfd.events = POLLIN;
    pfd.revents = 0;
    int pr = poll(&pfd, 1, timeout_ms);
    if (pr > 0 && !(pfd.revents & POLLIN)) return 0;
    return pr;
}
static void stop_signals(int listen_fd) {
    (void)listen_fd;
    struct sigaction sa;
    memset(&sa, 0, sizeof sa);
    sa.sa_handler = on_sig;
    sigemptyset(&sa.sa_mask);
    sa.sa_flags = 0; /* No SA_RESTART: poll()/accept() wake immediately on SIGTERM/SIGINT */
    sigaction(SIGINT, &sa, NULL);
    sigaction(SIGTERM, &sa, NULL);
}
static void sock_close_listener(int fd) { close(fd); }
static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}
#endif

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
        ssize_t w = sock_send(fd, buf + sent, len - sent);
        if (w < 0) {
            if (sock_interrupted()) continue;
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

static int j_raw_value(JScan *j, Buf *out) {
    j_ws(j);
    size_t start = j->pos;
    if (j_skip(j)) return 1;
    if (out && j->pos > start) buf_append(out, j->s + start, j->pos - start);
    return 0;
}

typedef struct {
    Buf id;
    Buf name;
    Buf args_json; /* JSON object string, e.g. {"path":"calc.py"} */
} ToolCall;

typedef struct {
    Buf       role;
    Buf       name;
    Buf       tool_call_id;
    Buf       content;
    Buf       reasoning;
    ToolCall *tcs;
    int       n_tcs;
    int       cap_tcs;
} ChatMsg;

typedef struct {
    ChatMsg *msgs;
    int      n_msgs;
    int      cap_msgs;
    Buf      tools_json;
    Buf      model;
    Buf      effort;
    int      json_mode; /* 0=off, 1=json_object, 2=json_schema */
    Buf      json_schema_name;
    Buf      json_schema_raw;
    int      stream;
    int      max_tokens;
    float    temperature;
    float    top_p;
    int      top_k;
    int      cache_prompt;
    int      resp_fmt;          /* 0=none, 1=json_object, 2=json_schema */
    Buf      resp_schema_json;  /* raw JSON schema object when resp_fmt==2 */
} Req;

static void tc_free(ToolCall *tc) {
    buf_free(&tc->id);
    buf_free(&tc->name);
    buf_free(&tc->args_json);
}

static ToolCall *msg_add_tc(ChatMsg *m) {
    if (m->n_tcs >= m->cap_tcs) {
        int nc = m->cap_tcs ? m->cap_tcs * 2 : 4;
        ToolCall *p = (ToolCall *)realloc(m->tcs, (size_t)nc * sizeof *p);
        if (!p) return NULL;
        m->tcs = p;
        m->cap_tcs = nc;
    }
    ToolCall *tc = &m->tcs[m->n_tcs++];
    memset(tc, 0, sizeof *tc);
    return tc;
}

static void req_free(Req *r) {
    for (int i = 0; i < r->n_msgs; i++) {
        buf_free(&r->msgs[i].role);
        buf_free(&r->msgs[i].name);
        buf_free(&r->msgs[i].tool_call_id);
        buf_free(&r->msgs[i].content);
        buf_free(&r->msgs[i].reasoning);
        for (int t = 0; t < r->msgs[i].n_tcs; t++) tc_free(&r->msgs[i].tcs[t]);
        free(r->msgs[i].tcs);
    }
    free(r->msgs);
    buf_free(&r->tools_json);
    buf_free(&r->model);
    buf_free(&r->effort);
    buf_free(&r->json_schema_name);
    buf_free(&r->json_schema_raw);
    buf_free(&r->resp_schema_json);
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

static int parse_tool_call_obj(JScan *j, ChatMsg *m) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '{') return j_skip(j);
    j->pos++;
    ToolCall *tc = msg_add_tc(m);
    if (!tc) return 1;
    j_ws(j);
    while (j->pos < j->len && j->s[j->pos] != '}') {
        Buf k = {0};
        if (j_string(j, &k)) { buf_free(&k); return 1; }
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); return 1; }
        j->pos++;
        const char *ks = k.data ? k.data : "";
        if (!strcmp(ks, "id")) {
            j_string(j, &tc->id);
        } else if (!strcmp(ks, "name")) {
            j_string(j, &tc->name);
        } else if (!strcmp(ks, "arguments")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &tc->args_json);
            else j_raw_value(j, &tc->args_json);
        } else if (!strcmp(ks, "function")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '{') {
                j->pos++;
                j_ws(j);
                while (j->pos < j->len && j->s[j->pos] != '}') {
                    Buf fk = {0};
                    if (j_string(j, &fk)) { buf_free(&fk); buf_free(&k); return 1; }
                    j_ws(j);
                    if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&fk); buf_free(&k); return 1; }
                    j->pos++;
                    const char *fks = fk.data ? fk.data : "";
                    if (!strcmp(fks, "name")) {
                        tc->name.len = 0;
                        j_string(j, &tc->name);
                    } else if (!strcmp(fks, "arguments")) {
                        tc->args_json.len = 0;
                        j_ws(j);
                        if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &tc->args_json);
                        else j_raw_value(j, &tc->args_json);
                    } else {
                        j_skip(j);
                    }
                    buf_free(&fk);
                    j_ws(j);
                    if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
                }
                if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
            } else {
                j_skip(j);
            }
        } else {
            j_skip(j);
        }
        buf_free(&k);
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
    if (tc->args_json.len == 0) buf_adds(&tc->args_json, "{}");
    return 0;
}

static int parse_tools_array(JScan *j, Buf *out) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '[') return j_skip(j);
    j->pos++;
    j_ws(j);
    buf_addc(out, '[');
    int n_out = 0;
    while (j->pos < j->len && j->s[j->pos] != ']') {
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != '{') { if (j_skip(j)) return 1; }
        else {
            j->pos++;
            Buf name = {0}, desc = {0}, params = {0}, type = {0};
            j_ws(j);
            while (j->pos < j->len && j->s[j->pos] != '}') {
                Buf k = {0};
                if (j_string(j, &k)) { buf_free(&k); buf_free(&name); buf_free(&desc); buf_free(&params); buf_free(&type); return 1; }
                j_ws(j);
                if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); buf_free(&name); buf_free(&desc); buf_free(&params); buf_free(&type); return 1; }
                j->pos++;
                const char *ks = k.data ? k.data : "";
                if (!strcmp(ks, "type")) { type.len = 0; j_string(j, &type); }
                else if (!strcmp(ks, "name")) { name.len = 0; j_string(j, &name); }
                else if (!strcmp(ks, "description")) { desc.len = 0; j_string(j, &desc); }
                else if (!strcmp(ks, "parameters")) { params.len = 0; j_raw_value(j, &params); }
                else if (!strcmp(ks, "function")) {
                    j_ws(j);
                    if (j->pos < j->len && j->s[j->pos] == '{') {
                        j->pos++;
                        j_ws(j);
                        while (j->pos < j->len && j->s[j->pos] != '}') {
                            Buf fk = {0};
                            if (j_string(j, &fk)) { buf_free(&fk); buf_free(&k); buf_free(&name); buf_free(&desc); buf_free(&params); buf_free(&type); return 1; }
                            j_ws(j);
                            if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&fk); buf_free(&k); buf_free(&name); buf_free(&desc); buf_free(&params); buf_free(&type); return 1; }
                            j->pos++;
                            const char *fks = fk.data ? fk.data : "";
                            if (!strcmp(fks, "name")) { name.len = 0; j_string(j, &name); }
                            else if (!strcmp(fks, "description")) { desc.len = 0; j_string(j, &desc); }
                            else if (!strcmp(fks, "parameters")) { params.len = 0; j_raw_value(j, &params); }
                            else j_skip(j);
                            buf_free(&fk);
                            j_ws(j);
                            if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
                        }
                        if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
                    } else j_skip(j);
                } else j_skip(j);
                buf_free(&k);
                j_ws(j);
                if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
            }
            if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
            if (name.len > 0) {
                if (n_out++ > 0) buf_addc(out, ',');
                buf_adds(out, "{\"description\":\"");
                if (desc.len) buf_add_json_escaped(out, desc.data, desc.len);
                buf_adds(out, "\",\"name\":\"");
                buf_add_json_escaped(out, name.data, name.len);
                buf_adds(out, "\",\"parameters\":");
                if (params.len) buf_append(out, params.data, params.len);
                else buf_adds(out, "{}");
                buf_adds(out, ",\"type\":\"");
                if (type.len) buf_add_json_escaped(out, type.data, type.len);
                else buf_adds(out, "function");
                buf_adds(out, "\"}");
            }
            buf_free(&name); buf_free(&desc); buf_free(&params); buf_free(&type);
        }
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == ']') j->pos++;
    buf_addc(out, ']');
    if (n_out == 0) out->len = 0;
    return 0;
}

static int parse_message_obj(JScan *j, Req *r) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '{') return j_skip(j);
    j->pos++;
    ChatMsg *m = req_add_msg(r);
    if (!m) return 1;
    Buf item_type = {0};
    Buf fc_args = {0};
    j_ws(j);
    while (j->pos < j->len && j->s[j->pos] != '}') {
        Buf k = {0};
        if (j_string(j, &k)) { buf_free(&k); buf_free(&item_type); buf_free(&fc_args); return 1; }
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); buf_free(&item_type); buf_free(&fc_args); return 1; }
        j->pos++;
        if (k.data && !strcmp(k.data, "type")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &item_type);
            else j_skip(j);
        } else if (k.data && !strcmp(k.data, "role")) {
            j_string(j, &m->role);
        } else if (k.data && !strcmp(k.data, "name")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &m->name);
            else j_skip(j);
        } else if (k.data && (!strcmp(k.data, "tool_call_id") || !strcmp(k.data, "call_id"))) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &m->tool_call_id);
            else j_skip(j);
        } else if (k.data && (!strcmp(k.data, "content") || !strcmp(k.data, "output"))) {
            parse_content_value(j, &m->content);
        } else if (k.data && !strcmp(k.data, "arguments")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &fc_args);
            else j_raw_value(j, &fc_args);
        } else if (k.data && (!strcmp(k.data, "reasoning_content") || !strcmp(k.data, "reasoning"))) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '"') j_string(j, &m->reasoning);
            else j_skip(j);
        } else if (k.data && !strcmp(k.data, "tool_calls")) {
            j_ws(j);
            if (j->pos < j->len && j->s[j->pos] == '[') {
                j->pos++;
                j_ws(j);
                while (j->pos < j->len && j->s[j->pos] != ']') {
                    if (parse_tool_call_obj(j, m)) { buf_free(&k); buf_free(&item_type); buf_free(&fc_args); return 1; }
                    j_ws(j);
                    if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
                }
                if (j->pos < j->len && j->s[j->pos] == ']') j->pos++;
            } else {
                j_skip(j);
            }
        } else {
            j_skip(j);
        }
        buf_free(&k);
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
    if (item_type.data && !strcmp(item_type.data, "function_call")) {
        m->role.len = 0;
        buf_adds(&m->role, "assistant");
        ToolCall *tc = msg_add_tc(m);
        if (tc) {
            if (m->tool_call_id.len) buf_append(&tc->id, m->tool_call_id.data, m->tool_call_id.len);
            if (m->name.len) buf_append(&tc->name, m->name.data, m->name.len);
            if (fc_args.len) buf_append(&tc->args_json, fc_args.data, fc_args.len);
            else buf_adds(&tc->args_json, "{}");
        }
        m->name.len = 0;
        m->tool_call_id.len = 0;
    } else if (item_type.data && !strcmp(item_type.data, "function_call_output")) {
        m->role.len = 0;
        buf_adds(&m->role, "tool");
    }
    buf_free(&item_type);
    buf_free(&fc_args);
    if (!m->role.len) buf_adds(&m->role, "user");
    return 0;
}

static void apply_effort_str(Req *r, const char *s) {
    if (!s) return;
    r->effort.len = 0;
    if (!strcmp(s, "none")) buf_adds(&r->effort, "0");
    else if (!strcmp(s, "minimal")) buf_adds(&r->effort, "0.1");
    else if (!strcmp(s, "low")) buf_adds(&r->effort, "0.2");
    else if (!strcmp(s, "medium")) buf_adds(&r->effort, "0.7");
    else if (!strcmp(s, "high")) buf_adds(&r->effort, "0.9");
    else if (!strcmp(s, "xhigh") || !strcmp(s, "max")) buf_adds(&r->effort, "0.99");
    else buf_adds(&r->effort, "0.7");
}

static void parse_json_schema_body(JScan *j, Req *r) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '{') { j_skip(j); return; }
    j->pos++;
    j_ws(j);
    while (j->pos < j->len && j->s[j->pos] != '}') {
        Buf k = {0};
        if (j_string(j, &k)) { buf_free(&k); return; }
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); return; }
        j->pos++;
        const char *ks = k.data ? k.data : "";
        if (!strcmp(ks, "name")) {
            r->json_schema_name.len = 0;
            j_string(j, &r->json_schema_name);
        } else if (!strcmp(ks, "schema")) {
            r->json_schema_raw.len = 0;
            j_raw_value(j, &r->json_schema_raw);
        } else {
            j_skip(j);
        }
        buf_free(&k);
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
}

static void parse_format_obj(JScan *j, Req *r) {
    j_ws(j);
    if (j->pos >= j->len || j->s[j->pos] != '{') { j_skip(j); return; }
    j->pos++;
    j_ws(j);
    while (j->pos < j->len && j->s[j->pos] != '}') {
        Buf k = {0};
        if (j_string(j, &k)) { buf_free(&k); return; }
        j_ws(j);
        if (j->pos >= j->len || j->s[j->pos] != ':') { buf_free(&k); return; }
        j->pos++;
        const char *ks = k.data ? k.data : "";
        if (!strcmp(ks, "type")) {
            Buf tv = {0};
            j_string(j, &tv);
            if (tv.data) {
                if (!strcmp(tv.data, "json_schema")) { r->json_mode = 2; r->resp_fmt = 2; }
                else if (!strcmp(tv.data, "json_object")) { r->json_mode = 1; r->resp_fmt = 1; }
                else if (!strcmp(tv.data, "text")) { r->json_mode = 0; r->resp_fmt = 0; }
            }
            buf_free(&tv);
        } else if (!strcmp(ks, "json_schema")) {
            parse_json_schema_body(j, r);
        } else if (!strcmp(ks, "name")) {
            r->json_schema_name.len = 0;
            j_string(j, &r->json_schema_name);
        } else if (!strcmp(ks, "schema")) {
            r->json_schema_raw.len = 0;
            j_raw_value(j, &r->json_schema_raw);
        } else {
            j_skip(j);
        }
        buf_free(&k);
        j_ws(j);
        if (j->pos < j->len && j->s[j->pos] == ',') j->pos++;
    }
    if (j->pos < j->len && j->s[j->pos] == '}') j->pos++;
    r->resp_schema_json.len = 0;
    if (r->resp_fmt == 2 && r->json_schema_raw.len > 0) {
        buf_append(&r->resp_schema_json, r->json_schema_raw.data, r->json_schema_raw.len);
    }
}

static int parse_request_json(const char *body, size_t body_len, Req *r) {
    memset(r, 0, sizeof *r);
    buf_adds(&r->model, "inkling-2-mini");
    buf_adds(&r->effort, "0.7");
    r->max_tokens = 2048;
    r->temperature = 0.0f;
    r->top_p = 0.95f;
    r->top_k = 40;
    r->cache_prompt = 1;

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
        } else if (!strcmp(ks, "cache_prompt")) {
            j_bool(&j, &r->cache_prompt);
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
                apply_effort_str(r, ev.data);
            } else {
                j_skip(&j);
            }
            buf_free(&ev);
        } else if (!strcmp(ks, "reasoning")) {
            j_ws(&j);
            if (j.pos < j.len && j.s[j.pos] == '{') {
                j.pos++; j_ws(&j);
                while (j.pos < j.len && j.s[j.pos] != '}') {
                    Buf rk = {0};
                    if (j_string(&j, &rk)) { buf_free(&rk); break; }
                    j_ws(&j);
                    if (j.pos >= j.len || j.s[j.pos] != ':') { buf_free(&rk); break; }
                    j.pos++;
                    if (rk.data && !strcmp(rk.data, "effort")) {
                        Buf ev = {0};
                        j_string(&j, &ev);
                        apply_effort_str(r, ev.data);
                        buf_free(&ev);
                    } else {
                        j_skip(&j);
                    }
                    buf_free(&rk);
                    j_ws(&j);
                    if (j.pos < j.len && j.s[j.pos] == ',') j.pos++;
                }
                if (j.pos < j.len && j.s[j.pos] == '}') j.pos++;
            } else {
                j_skip(&j);
            }
        } else if (!strcmp(ks, "response_format")) {
            parse_format_obj(&j, r);
        } else if (!strcmp(ks, "text")) {
            j_ws(&j);
            if (j.pos < j.len && j.s[j.pos] == '{') {
                j.pos++; j_ws(&j);
                while (j.pos < j.len && j.s[j.pos] != '}') {
                    Buf tk = {0};
                    if (j_string(&j, &tk)) { buf_free(&tk); break; }
                    j_ws(&j);
                    if (j.pos >= j.len || j.s[j.pos] != ':') { buf_free(&tk); break; }
                    j.pos++;
                    if (tk.data && !strcmp(tk.data, "format")) {
                        parse_format_obj(&j, r);
                    } else {
                        j_skip(&j);
                    }
                    buf_free(&tk);
                    j_ws(&j);
                    if (j.pos < j.len && j.s[j.pos] == ',') j.pos++;
                }
                if (j.pos < j.len && j.s[j.pos] == '}') j.pos++;
            } else {
                j_skip(&j);
            }
        } else if (!strcmp(ks, "tools")) {
            r->tools_json.len = 0;
            parse_tools_array(&j, &r->tools_json);
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

    if (r->tools_json.len > 0) {
        buf_adds(out, "<|message_system|>tool_declare<|content_xml|>");
        buf_append(out, r->tools_json.data, r->tools_json.len);
        buf_adds(out, "<|end_message|>");
    }
    if (r->json_mode > 0) {
        buf_adds(out, "<|message_system|><|content_text|>Output ONLY one valid JSON object in your final answer (no markdown fences, no prose outside the JSON object)");
        if (r->json_schema_raw.len > 0) {
            buf_adds(out, " conforming strictly to this JSON Schema: ");
            buf_append(out, r->json_schema_raw.data, r->json_schema_raw.len);
        }
        buf_adds(out, "<|end_message|>");
    }

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
        Buf rtok = {0};
        if (is_sys) buf_adds(&rtok, "<|message_system|>");
        else if (!strcmp(role, "assistant")) {
            buf_adds(&rtok, "<|message_model|>");
            if (m->name.len > 0) buf_append(&rtok, m->name.data, m->name.len);
        } else if (!strcmp(role, "tool")) {
            buf_adds(&rtok, "<|message_tool|>");
            if (m->name.len > 0) {
                buf_append(&rtok, m->name.data, m->name.len);
            } else if (m->tool_call_id.len > 0) {
                for (int pi = 0; pi < i; pi++) {
                    const ChatMsg *pm = &r->msgs[pi];
                    for (int t = 0; t < pm->n_tcs; t++) {
                        if (pm->tcs[t].id.len == m->tool_call_id.len &&
                            memcmp(pm->tcs[t].id.data, m->tool_call_id.data, m->tool_call_id.len) == 0 &&
                            pm->tcs[t].name.len > 0) {
                            buf_append(&rtok, pm->tcs[t].name.data, pm->tcs[t].name.len);
                            goto found_tname;
                        }
                    }
                }
found_tname:;
            }
        } else {
            buf_adds(&rtok, "<|message_user|>");
        }

        if (!strcmp(role, "assistant") && m->reasoning.len > 0) {
            buf_append(out, rtok.data, rtok.len);
            buf_adds(out, "<|content_thinking|>");
            buf_append(out, m->reasoning.data, m->reasoning.len);
            buf_adds(out, "<|end_message|>");
        }
        if (m->content.len > 0 || strcmp(role, "assistant") != 0 || (m->reasoning.len == 0 && m->n_tcs == 0)) {
            buf_append(out, rtok.data, rtok.len);
            buf_adds(out, "<|content_text|>");
            if (m->content.len > 0) buf_append(out, m->content.data, m->content.len);
            buf_adds(out, "<|end_message|>");
        }
        if (!strcmp(role, "assistant") && m->n_tcs > 0) {
            for (int t = 0; t < m->n_tcs; t++) {
                const ToolCall *tc = &m->tcs[t];
                buf_adds(out, "<|message_model|>");
                if (tc->name.len) buf_append(out, tc->name.data, tc->name.len);
                buf_adds(out, "<|content_invoke_tool_json|>{\"name\":\"");
                if (tc->name.len) buf_add_json_escaped(out, tc->name.data, tc->name.len);
                buf_adds(out, "\",\"args\":");
                if (tc->args_json.len) buf_append(out, tc->args_json.data, tc->args_json.len);
                else buf_adds(out, "{}");
                buf_adds(out, "}<|end_message|>");
            }
        }
        if (!strcmp(role, "assistant")) {
            buf_adds(out, "<|content_model_end_sampling|>");
        }
        buf_free(&rtok);
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
    InkSession     *sess;
    InkMutex        sess_mu;
    InkMutex        state_mu;
    const char     *engine;
    int             in_flight;
    double          last_active;
    int             idle_exit_s;
    int             max_ctx;
    int32_t         tok_model;
    int32_t         tok_thinking;
    int32_t         tok_text;
    int32_t         tok_invoke_tool;
    int32_t         tok_end_msg;
    int32_t         tok_eog;
    uint64_t        rng;
    int             req_seq;
    /* Pre-decoded vocabulary table for zero-alloc JSON Schema constrained decoding */
    int             n_vocab;
    char           *vocab_pool;
    uint32_t       *vocab_off;
    uint16_t       *vocab_len;
    uint8_t        *vocab_special;
} Server;

static int vocab_table_build(Server *sv) {
    int n = ink_vocab_unpadded(sv->sess);
    if (n <= 0) return 1;
    sv->n_vocab = n;
    sv->vocab_off = (uint32_t *)calloc((size_t)n, sizeof(uint32_t));
    sv->vocab_len = (uint16_t *)calloc((size_t)n, sizeof(uint16_t));
    sv->vocab_special = (uint8_t *)calloc((size_t)n, sizeof(uint8_t));
    if (!sv->vocab_off || !sv->vocab_len || !sv->vocab_special) return 1;
    Buf pool = {0};
    char tmp[512];
    for (int i = 0; i < n; i++) {
        if (i >= 200000 || ink_is_eog(sv->sess, i)) {
            sv->vocab_special[i] = 1;
            continue;
        }
        int len = ink_decode_piece(sv->sess, i, tmp, (int)sizeof tmp);
        if (len > 0) {
            if (len > 65535) len = 65535;
            sv->vocab_off[i] = (uint32_t)pool.len;
            sv->vocab_len[i] = (uint16_t)len;
            if (buf_append(&pool, tmp, (size_t)len)) { buf_free(&pool); return 1; }
        }
    }
    sv->vocab_pool = pool.data;
    return 0;
}

static void vocab_table_free(Server *sv) {
    free(sv->vocab_pool); sv->vocab_pool = NULL;
    free(sv->vocab_off); sv->vocab_off = NULL;
    free(sv->vocab_len); sv->vocab_len = NULL;
    free(sv->vocab_special); sv->vocab_special = NULL;
    sv->n_vocab = 0;
}

typedef struct { int32_t id; float val; } Cand;

static int cmp_cand_desc(const void *a, const void *b) {
    float va = ((const Cand *)a)->val;
    float vb = ((const Cand *)b)->val;
    return (vb > va) - (vb < va);
}

static uint64_t sv_xorshift64(uint64_t *st) {
    uint64_t x = *st ? *st : 0x9E3779B97F4A7C15ULL;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *st = x;
    return x;
}

static int js_tok_allowed(const Server *sv, const coax_t_JsSchema *sc, const coax_t_JsState *st,
                          int32_t tok, int can_fin, coax_t_JsState *next_out) {
    if (tok == sv->tok_end_msg || tok == sv->tok_eog || ink_is_eog(sv->sess, tok)) {
        if (!can_fin) return 0;
        if (next_out) *next_out = *st;
        return 1;
    }
    if (tok < 0 || tok >= sv->n_vocab || sv->vocab_special[tok]) return 0;
    uint16_t len = sv->vocab_len[tok];
    if (len == 0) return 0;
    coax_t_JsState trial = *st;
    coax_v_t_JsSchema scv = { (coax_t_JsSchema *)sc, 1 };
    coax_v_t_JsState stv = { &trial, 1 };
    coax_string piece = { sv->vocab_pool + sv->vocab_off[tok], (int64_t)len };
    if (ink_js_step(scv, stv, piece) == 1) {
        if (next_out) *next_out = trial;
        return 1;
    }
    return 0;
}

static int32_t js_sample_constrained(Server *sv, const coax_t_JsSchema *sc, coax_t_JsState *st,
                                     const InkSampleParams *sp) {
    coax_v_t_JsSchema scv = { (coax_t_JsSchema *)sc, 1 };
    coax_v_t_JsState stv = { st, 1 };
    if (ink_js_is_done(stv) == 1) {
        return (sv->tok_end_msg >= 0) ? sv->tok_end_msg : sv->tok_eog;
    }
    int can_fin = (ink_js_can_finish(scv, stv) == 1);
    const float *logits = ink_logits(sv->sess);
    int n = sv->n_vocab;
    if (!logits || n <= 0) return -1;

    float temp = sp ? sp->temperature : 0.0f;
    if (temp <= 1e-6f) {
        /* Fast path 1: check unconstrained argmax first (zero heap/sort). */
        int32_t argmax = 0;
        float best_v = logits[0];
        for (int i = 1; i < n; i++) {
            if (logits[i] > best_v) { best_v = logits[i]; argmax = i; }
        }
        coax_t_JsState next_st;
        if (js_tok_allowed(sv, sc, st, argmax, can_fin, &next_st)) {
            *st = next_st;
            return argmax;
        }
        /* Fast path 2: top-64 logits pass. */
        enum { KFAST = 64 };
        Cand top[KFAST];
        int nt = 0;
        float min_v = -1e30f;
        int min_idx = 0;
        for (int i = 0; i < n; i++) {
            float v = logits[i];
            if (nt < KFAST) {
                top[nt] = (Cand){ i, v };
                if (nt == 0 || v < min_v) { min_v = v; min_idx = nt; }
                nt++;
            } else if (v > min_v) {
                top[min_idx] = (Cand){ i, v };
                min_v = top[0].val; min_idx = 0;
                for (int j = 1; j < KFAST; j++) {
                    if (top[j].val < min_v) { min_v = top[j].val; min_idx = j; }
                }
            }
        }
        qsort(top, (size_t)nt, sizeof(Cand), cmp_cand_desc);
        for (int j = 0; j < nt; j++) {
            if (js_tok_allowed(sv, sc, st, top[j].id, can_fin, &next_st)) {
                *st = next_st;
                return top[j].id;
            }
        }
        /* Exact full-vocab fallback: guaranteed to find the highest-logit schema-valid token. */
        int32_t win = -1;
        float win_v = -1e30f;
        coax_t_JsState win_st;
        for (int i = 0; i < n; i++) {
            float v = logits[i];
            if (win >= 0 && v <= win_v) continue;
            if (js_tok_allowed(sv, sc, st, i, can_fin, &next_st)) {
                win = i; win_v = v; win_st = next_st;
            }
        }
        if (win >= 0) { *st = win_st; return win; }
        return sv->tok_eog;
    }

    /* Stochastic sampling: top-K filtered by schema, then temperature + top_p. */
    int K = (sp && sp->top_k > 64) ? sp->top_k : 128;
    if (K > 256) K = 256;
    Cand top[256];
    int nt = 0;
    float min_v = -1e30f;
    int min_idx = 0;
    for (int i = 0; i < n; i++) {
        float v = logits[i];
        if (nt < K) {
            top[nt] = (Cand){ i, v };
            if (nt == 0 || v < min_v) { min_v = v; min_idx = nt; }
            nt++;
        } else if (v > min_v) {
            top[min_idx] = (Cand){ i, v };
            min_v = top[0].val; min_idx = 0;
            for (int j = 1; j < K; j++) {
                if (top[j].val < min_v) { min_v = top[j].val; min_idx = j; }
            }
        }
    }
    qsort(top, (size_t)nt, sizeof(Cand), cmp_cand_desc);
    Cand valid[256];
    int nv = 0;
    for (int j = 0; j < nt; j++) {
        if (js_tok_allowed(sv, sc, st, top[j].id, can_fin, NULL)) {
            valid[nv++] = top[j];
        }
    }
    coax_t_JsState next_st;
    if (nv == 0) {
        int32_t win = -1;
        float win_v = -1e30f;
        coax_t_JsState win_st;
        for (int i = 0; i < n; i++) {
            float v = logits[i];
            if (win >= 0 && v <= win_v) continue;
            if (js_tok_allowed(sv, sc, st, i, can_fin, &next_st)) {
                win = i; win_v = v; win_st = next_st;
            }
        }
        if (win >= 0) { *st = win_st; return win; }
        return sv->tok_eog;
    }
    float max_l = valid[0].val;
    float probs[256];
    float sum = 0.0f;
    for (int j = 0; j < nv; j++) {
        probs[j] = expf((valid[j].val - max_l) / temp);
        sum += probs[j];
    }
    float top_p = (sp && sp->top_p > 0.0f && sp->top_p < 1.0f) ? sp->top_p : 1.0f;
    float cum = 0.0f;
    int cut = nv;
    for (int j = 0; j < nv; j++) {
        probs[j] /= sum;
        cum += probs[j];
        if (cum >= top_p) { cut = j + 1; break; }
    }
    float csum = 0.0f;
    for (int j = 0; j < cut; j++) csum += probs[j];
    float u = (float)((sv_xorshift64(&sv->rng) >> 11) * (1.0 / 9007199254740992.0)) * csum;
    float acc = 0.0f;
    int chosen = cut - 1;
    for (int j = 0; j < cut; j++) {
        acc += probs[j];
        if (acc >= u) { chosen = j; break; }
    }
    int32_t tok = valid[chosen].id;
    if (js_tok_allowed(sv, sc, st, tok, can_fin, &next_st)) *st = next_st;
    return tok;
}

static int32_t js_sample_no_eog(Server *sv, const InkSampleParams *sp, int prefer_text) {
    int32_t tok = ink_sample(sv->sess, sp, &sv->rng);
    if (tok >= 0 && tok != sv->tok_eog && !ink_is_eog(sv->sess, tok)) return tok;
    if (prefer_text && sv->tok_text >= 0) return sv->tok_text;
    const float *logits = ink_logits(sv->sess);
    int n = sv->n_vocab;
    if (!logits || n <= 0) return tok;
    int32_t best = -1;
    float best_v = -1e30f;
    for (int i = 0; i < n; i++) {
        if (i == sv->tok_eog || ink_is_eog(sv->sess, i)) continue;
        if (best < 0 || logits[i] > best_v) { best = i; best_v = logits[i]; }
    }
    return best >= 0 ? best : tok;
}

static void parse_invoked_tool_json(const char *s, size_t n, const Buf *hdr_name, Buf *out_name, Buf *out_args) {
    JScan j = { s, n, 0 };
    j_ws(&j);
    if (j.pos < j.len && j.s[j.pos] == '{') {
        j.pos++;
        j_ws(&j);
        while (j.pos < j.len && j.s[j.pos] != '}') {
            Buf k = {0};
            if (j_string(&j, &k)) { buf_free(&k); break; }
            j_ws(&j);
            if (j.pos >= j.len || j.s[j.pos] != ':') { buf_free(&k); break; }
            j.pos++;
            const char *ks = k.data ? k.data : "";
            if (!strcmp(ks, "name")) {
                out_name->len = 0;
                j_string(&j, out_name);
            } else if (!strcmp(ks, "args") || !strcmp(ks, "arguments")) {
                out_args->len = 0;
                j_ws(&j);
                if (j.pos < j.len && j.s[j.pos] == '"') j_string(&j, out_args);
                else j_raw_value(&j, out_args);
            } else {
                j_skip(&j);
            }
            buf_free(&k);
            j_ws(&j);
            if (j.pos < j.len && j.s[j.pos] == ',') j.pos++;
        }
    }
    if (out_name->len == 0 && hdr_name && hdr_name->len > 0) {
        buf_append(out_name, hdr_name->data, hdr_name->len);
    }
    if (out_args->len == 0) buf_adds(out_args, "{}");
}

static void emit_chat_tool_call(int fd, int64_t created, const char *model, int idx,
                                const char *id, const char *name, size_t name_len,
                                const char *args, size_t args_len) {
    Buf ev = {0};
    char head[256];
    snprintf(head, sizeof head,
             "{\"id\":\"chatcmpl-psionic\",\"object\":\"chat.completion.chunk\",\"created\":%lld,\"model\":\"",
             (long long)created);
    buf_adds(&ev, head);
    buf_add_json_escaped(&ev, model, strlen(model));
    char mid[160];
    snprintf(mid, sizeof mid,
             "\",\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":%d,\"id\":\"%s\",\"type\":\"function\",\"function\":{\"name\":\"",
             idx, id);
    buf_adds(&ev, mid);
    buf_add_json_escaped(&ev, name, name_len);
    buf_adds(&ev, "\",\"arguments\":\"");
    buf_add_json_escaped(&ev, args, args_len);
    buf_adds(&ev, "\"}}]},\"finish_reason\":null}]}");
    send_sse_event(fd, NULL, ev.data, ev.len);
    buf_free(&ev);
}

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

static void emit_responses_tool_call(int fd, int idx, const char *id,
                                     const char *name, size_t name_len,
                                     const char *args, size_t args_len) {
    Buf ev = {0};
    char head[160];
    snprintf(head, sizeof head, "{\"type\":\"response.output_item.added\",\"output_index\":%d,\"item\":{\"type\":\"function_call\",\"id\":\"fc_%s\",\"call_id\":\"%s\",\"name\":\"", idx, id, id);
    buf_adds(&ev, head);
    buf_add_json_escaped(&ev, name, name_len);
    buf_adds(&ev, "\",\"arguments\":\"\",\"status\":\"in_progress\"}}");
    send_sse_event(fd, "response.output_item.added", ev.data, ev.len);
    ev.len = 0;

    snprintf(head, sizeof head, "{\"type\":\"response.function_call_arguments.delta\",\"item_id\":\"fc_%s\",\"output_index\":%d,\"delta\":\"", id, idx);
    buf_adds(&ev, head);
    buf_add_json_escaped(&ev, args, args_len);
    buf_adds(&ev, "\"}");
    send_sse_event(fd, "response.function_call_arguments.delta", ev.data, ev.len);
    ev.len = 0;

    snprintf(head, sizeof head, "{\"type\":\"response.output_item.done\",\"output_index\":%d,\"item\":{\"type\":\"function_call\",\"id\":\"fc_%s\",\"call_id\":\"%s\",\"name\":\"", idx, id, id);
    buf_adds(&ev, head);
    buf_add_json_escaped(&ev, name, name_len);
    buf_adds(&ev, "\",\"arguments\":\"");
    buf_add_json_escaped(&ev, args, args_len);
    buf_adds(&ev, "\",\"status\":\"completed\"}}");
    send_sse_event(fd, "response.output_item.done", ev.data, ev.len);
    buf_free(&ev);
}

static void handle_inference(Server *sv, int fd, int is_responses, const char *body, size_t body_len) {
    Req r;
    if (parse_request_json(body, body_len, &r) != 0) {
        const char *e = "{\"error\":{\"message\":\"invalid JSON request\",\"type\":\"invalid_request_error\"}}";
        send_http_json(fd, 400, "Bad Request", e, strlen(e));
        req_free(&r);
        return;
    }

    int use_js = 0;
    coax_t_JsSchema *js_sc = NULL;
    coax_t_JsState js_st;
    memset(&js_st, 0, sizeof js_st);
    if (r.resp_fmt == 1 || r.resp_fmt == 2) {
        js_sc = (coax_t_JsSchema *)calloc(1, sizeof *js_sc);
        if (!js_sc) {
            const char *e = "{\"error\":{\"message\":\"out of memory\"}}";
            send_http_json(fd, 500, "Internal Server Error", e, strlen(e));
            req_free(&r);
            return;
        }
        coax_v_t_JsSchema scv = { js_sc, 1 };
        int64_t ok = 0;
        if (r.resp_fmt == 1) {
            ok = ink_js_init_object(scv);
        } else if (r.resp_schema_json.data && r.resp_schema_json.len > 0) {
            coax_string sjs = { r.resp_schema_json.data, (int64_t)r.resp_schema_json.len };
            ok = ink_js_compile_schema(scv, sjs);
        }
        if (ok != 1 || ink_js_reset(scv, (coax_v_t_JsState){ &js_st, 1 }) != 1) {
            const char *e = "{\"error\":{\"message\":\"invalid or unsupported response_format json_schema\",\"type\":\"invalid_request_error\"}}";
            send_http_json(fd, 400, "Bad Request", e, strlen(e));
            free(js_sc);
            req_free(&r);
            return;
        }
        use_js = 1;
    }

    Buf prompt = {0};
    format_inkling_prompt(&r, &prompt);

    ink_mutex_lock(&sv->state_mu);
    sv->in_flight++;
    sv->last_active = now_sec();
    ink_mutex_unlock(&sv->state_mu);

    ink_mutex_lock(&sv->sess_mu);

    int max_ctx = ink_max_ctx(sv->sess);
    int32_t *toks = (int32_t *)malloc((size_t)(max_ctx + 1) * sizeof(int32_t));
    if (!toks) {
        const char *e = "{\"error\":{\"message\":\"out of memory\"}}";
        send_http_json(fd, 500, "Internal Server Error", e, strlen(e));
        ink_mutex_unlock(&sv->sess_mu);
        ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
        buf_free(&prompt); free(js_sc); req_free(&r);
        return;
    }
    int n_prompt = ink_encode(sv->sess, prompt.data ? prompt.data : "", prompt.len, toks, max_ctx, 1);
    if (n_prompt <= 0) {
        const char *e = "{\"error\":{\"message\":\"empty prompt\",\"type\":\"invalid_request_error\"}}";
        send_http_json(fd, 400, "Bad Request", e, strlen(e));
        ink_mutex_unlock(&sv->sess_mu);
        ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
        free(toks); buf_free(&prompt); free(js_sc); req_free(&r);
        return;
    }
    if (n_prompt >= max_ctx - 1) {
        char ebuf[320];
        snprintf(ebuf, sizeof ebuf,
                 "{\"error\":{\"message\":\"This model's maximum context length is %d tokens. However, your messages exceeded the context length.\",\"type\":\"invalid_request_error\",\"code\":\"context_length_exceeded\"}}",
                 max_ctx);
        send_http_json(fd, 400, "Bad Request", ebuf, strlen(ebuf));
        ink_mutex_unlock(&sv->sess_mu);
        ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
        free(toks); buf_free(&prompt); free(js_sc); req_free(&r);
        return;
    }

    int64_t created = (int64_t)time(NULL);
    const char *model = (r.model.data && r.model.len) ? r.model.data : "inkling-2-mini";
    sv->req_seq++;
    fprintf(stderr, "[psionic-inkling-serve] req#%d %s stream=%d fmt=%d prompt=%d starting\n",
            sv->req_seq, is_responses ? "/v1/responses" : "/v1/chat/completions", r.stream, r.resp_fmt, n_prompt);
    fflush(stderr);

    if (r.stream) {
        if (send_sse_headers(fd) != 0) {
            ink_mutex_unlock(&sv->sess_mu);
            ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
            free(toks); buf_free(&prompt); free(js_sc); req_free(&r);
            return;
        }
    }

    double t0 = now_sec();
    int cached = 0;
    if (r.cache_prompt) {
        cached = ink_prefix_restore(sv->sess, toks, n_prompt);
    } else {
        ink_reset(sv->sess);
    }
    for (int i = cached; i < n_prompt && !g_stop; ) {
        int chunk = n_prompt - i;
        if (chunk > 3072) chunk = 3072;
        int need_logits = (i + chunk == n_prompt);
        if (ink_prefill(sv->sess, toks + i, chunk, need_logits) != 0) {
            if (!r.stream) {
                char ebuf[512];
                snprintf(ebuf, sizeof ebuf, "{\"error\":{\"message\":\"prefill failed: %s\"}}", ink_error(sv->sess));
                send_http_json(fd, 500, "Internal Server Error", ebuf, strlen(ebuf));
            }
            ink_mutex_unlock(&sv->sess_mu);
            ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
            free(toks); buf_free(&prompt); free(js_sc); req_free(&r);
            return;
        }
        i += chunk;
        if (r.stream && i < n_prompt) {
            if (send_all(fd, ": prefilling\n\n", 14) != 0) {
                ink_mutex_unlock(&sv->sess_mu);
                ink_mutex_lock(&sv->state_mu); sv->in_flight--; sv->last_active = now_sec(); ink_mutex_unlock(&sv->state_mu);
                free(toks); buf_free(&prompt); free(js_sc); req_free(&r);
                return;
            }
        }
    }
    if (r.cache_prompt && !g_stop && cached < n_prompt) {
        ink_prefix_save(sv->sess, toks, n_prompt, 1);
    }
    double t_prefill = now_sec() - t0;
    fprintf(stderr, "[psionic-inkling-serve] req#%d prefill=%d cached=%d new=%d in %.3fs\n",
            sv->req_seq, n_prompt, cached, n_prompt - cached, t_prefill);
    fflush(stderr);
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

    /* Channel state machine: 0=none, 1=thinking, 2=text, 3=invoke_tool_json */
    int channel = 0;
    int in_header = 0;
    int js_started = 0;
    Buf header_buf = {0};
    Buf tool_json = {0};
    Buf pending = {0};
    Buf full_text = {0};
    Buf full_think = {0};
    ToolCall *emitted_tcs = NULL;
    int n_emitted_tcs = 0, cap_emitted_tcs = 0;
    InkSampleParams sp = { .temperature = r.temperature, .top_p = r.top_p, .top_k = r.top_k };

    int max_gen = r.max_tokens;
    int avail = max_ctx - n_prompt - 1;
    if (avail < 1) avail = 1;
    if (max_gen > avail) max_gen = avail;

    int n_gen = 0;
    const char *finish_reason = "length";
    double td0 = now_sec();

    for (int i = 0; i < max_gen && !g_stop; i++) {
        int32_t tok;
        if (use_js && channel == 2) {
            js_started = 1;
            tok = js_sample_constrained(sv, js_sc, &js_st, &sp);
        } else if (use_js && !js_started) {
            tok = js_sample_no_eog(sv, &sp, (channel == 0 && !in_header));
            /* If the model emits a non-special token directly from channel 0 (implicit channel 2),
             * enforce the JSON Schema grammar on it too. */
            if (channel == 0 && !in_header && tok >= 0 && tok < 200000 &&
                tok != sv->tok_model && tok != sv->tok_thinking && tok != sv->tok_text &&
                tok != sv->tok_end_msg && tok != sv->tok_eog && !ink_is_eog(sv->sess, tok)) {
                channel = 2;
                js_started = 1;
                coax_t_JsState next_st;
                if (js_tok_allowed(sv, js_sc, &js_st, tok, 0, &next_st)) {
                    js_st = next_st;
                } else {
                    tok = js_sample_constrained(sv, js_sc, &js_st, &sp);
                }
            }
        } else {
            tok = ink_sample(sv->sess, &sp, &sv->rng);
        }
        if (tok < 0) break;
        n_gen++;

        if (tok == sv->tok_eog || ink_is_eog(sv->sess, tok)) {
            finish_reason = "stop";
            break;
        }

        if (tok == sv->tok_model || tok == sv->tok_end_msg) {
            int was_js_text = (use_js && channel == 2);
            if (channel == 3 && tool_json.len > 0) {
                if (n_emitted_tcs >= cap_emitted_tcs) {
                    int nc = cap_emitted_tcs ? cap_emitted_tcs * 2 : 4;
                    ToolCall *np = (ToolCall *)realloc(emitted_tcs, (size_t)nc * sizeof *np);
                    if (np) { emitted_tcs = np; cap_emitted_tcs = nc; }
                }
                if (n_emitted_tcs < cap_emitted_tcs) {
                    ToolCall *tc = &emitted_tcs[n_emitted_tcs];
                    memset(tc, 0, sizeof *tc);
                    char idbuf[64];
                    snprintf(idbuf, sizeof idbuf, "call_psionic_%d_%d", sv->req_seq, n_emitted_tcs);
                    buf_adds(&tc->id, idbuf);
                    parse_invoked_tool_json(tool_json.data, tool_json.len, &header_buf, &tc->name, &tc->args_json);
                    if (r.stream) {
                        if (is_responses) {
                            emit_responses_tool_call(fd, n_emitted_tcs, tc->id.data,
                                                     tc->name.data ? tc->name.data : "", tc->name.len,
                                                     tc->args_json.data ? tc->args_json.data : "{}", tc->args_json.len);
                        } else {
                            emit_chat_tool_call(fd, created, model, n_emitted_tcs, tc->id.data,
                                                tc->name.data ? tc->name.data : "", tc->name.len,
                                                tc->args_json.data ? tc->args_json.data : "{}", tc->args_json.len);
                        }
                    }
                    n_emitted_tcs++;
                }
                tool_json.len = 0;
            } else if (pending.len > 0 && (channel == 1 || channel == 2)) {
                if (channel == 1) buf_append(&full_think, pending.data, pending.len);
                else buf_append(&full_text, pending.data, pending.len);
                if (r.stream) {
                    if (is_responses) emit_responses_delta(fd, channel == 1, pending.data, pending.len);
                    else emit_chat_delta(fd, created, model, channel == 1, pending.data, pending.len);
                }
                pending.len = 0;
            }
            channel = 0;
            if (was_js_text) {
                finish_reason = "stop";
                break;
            }
            if (tok == sv->tok_model) {
                in_header = 1;
                header_buf.len = 0;
            } else {
                in_header = 0;
            }
        } else if (tok == sv->tok_thinking) {
            channel = 1;
            in_header = 0;
            pending.len = 0;
        } else if (tok == sv->tok_text) {
            channel = 2;
            in_header = 0;
            pending.len = 0;
        } else if (sv->tok_invoke_tool >= 0 && tok == sv->tok_invoke_tool) {
            channel = 3;
            in_header = 0;
            pending.len = 0;
            tool_json.len = 0;
        } else if (tok >= 200000) {
            /* Other special control token: ignore in user-visible output */
        } else {
            char piece[512];
            int plen = ink_decode_piece(sv->sess, tok, piece, sizeof piece);
            if (plen > 0) {
                if (in_header) {
                    buf_append(&header_buf, piece, (size_t)plen);
                } else if (channel == 3) {
                    buf_append(&tool_json, piece, (size_t)plen);
                } else {
                    if (channel == 0) channel = 2;
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
            if (use_js && channel == 2 && ink_js_is_done((coax_v_t_JsState){ &js_st, 1 }) == 1) {
                if (pending.len > 0) {
                    buf_append(&full_text, pending.data, pending.len);
                    if (r.stream) {
                        if (is_responses) emit_responses_delta(fd, 0, pending.data, pending.len);
                        else emit_chat_delta(fd, created, model, 0, pending.data, pending.len);
                    }
                    pending.len = 0;
                }
                finish_reason = "stop";
                break;
            }
        }

        if (i + 1 < max_gen) {
            if (ink_step(sv->sess, tok, 1) != 0) {
                fprintf(stderr, "[psionic-inkling-serve] step error: %s\n", ink_error(sv->sess));
                break;
            }
        }
    }
    if (channel == 3 && tool_json.len > 0) {
        if (n_emitted_tcs >= cap_emitted_tcs) {
            int nc = cap_emitted_tcs ? cap_emitted_tcs * 2 : 4;
            ToolCall *np = (ToolCall *)realloc(emitted_tcs, (size_t)nc * sizeof *np);
            if (np) { emitted_tcs = np; cap_emitted_tcs = nc; }
        }
        if (n_emitted_tcs < cap_emitted_tcs) {
            ToolCall *tc = &emitted_tcs[n_emitted_tcs];
            memset(tc, 0, sizeof *tc);
            char idbuf[64];
            snprintf(idbuf, sizeof idbuf, "call_psionic_%d_%d", sv->req_seq, n_emitted_tcs);
            buf_adds(&tc->id, idbuf);
            parse_invoked_tool_json(tool_json.data, tool_json.len, &header_buf, &tc->name, &tc->args_json);
            if (r.stream) {
                if (is_responses) {
                    emit_responses_tool_call(fd, n_emitted_tcs, tc->id.data,
                                             tc->name.data ? tc->name.data : "", tc->name.len,
                                             tc->args_json.data ? tc->args_json.data : "{}", tc->args_json.len);
                } else {
                    emit_chat_tool_call(fd, created, model, n_emitted_tcs, tc->id.data,
                                        tc->name.data ? tc->name.data : "", tc->name.len,
                                        tc->args_json.data ? tc->args_json.data : "{}", tc->args_json.len);
                }
            }
            n_emitted_tcs++;
        }
    } else if (pending.len > 0 && (channel == 1 || channel == 2)) {
        if (channel == 1) buf_append(&full_think, pending.data, pending.len);
        else buf_append(&full_text, pending.data, pending.len);
        if (r.stream) {
            if (is_responses) emit_responses_delta(fd, channel == 1, pending.data, pending.len);
            else emit_chat_delta(fd, created, model, channel == 1, pending.data, pending.len);
        }
    }
    if (n_emitted_tcs > 0 && !strcmp(finish_reason, "stop")) {
        finish_reason = "tool_calls";
    }
    double t_decode = now_sec() - td0;
    fprintf(stderr, "[psionic-inkling-serve] req#%d %s stream=%d prompt=%d (%.2fs) gen=%d (%.2fs, %.1f tok/s) finish=%s\n",
            sv->req_seq, is_responses ? "/v1/responses" : "/v1/chat/completions", r.stream,
            n_prompt, t_prefill, n_gen, t_decode, n_gen / (t_decode + 1e-9), finish_reason);

    if (r.stream) {
        if (is_responses) {
            Buf done = {0};
            char u[320];
            long long lc = ink_hparams(sv->sess) ? (long long)ink_hparams(sv->sess)->loop_count : 3;
            snprintf(u, sizeof u,
                     "{\"type\":\"response.completed\",\"response\":{\"id\":\"resp-psionic-%d\",\"status\":\"completed\",\"usage\":{\"input_tokens\":%d,\"output_tokens\":%d,\"total_tokens\":%d,\"loop_count\":%lld}}}",
                     sv->req_seq, n_prompt, n_gen, n_prompt + n_gen, lc);
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
        buf_adds(&out, "\",\"output\":[");
        int out_n = 0;
        if (full_think.len) {
            buf_adds(&out, "{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"");
            buf_add_json_escaped(&out, full_think.data, full_think.len);
            buf_adds(&out, "\"}]}");
            out_n++;
        }
        if (full_text.len || n_emitted_tcs == 0) {
            if (out_n++) buf_adds(&out, ",");
            buf_adds(&out, "{\"type\":\"message\",\"role\":\"assistant\",\"content\":[{\"type\":\"output_text\",\"text\":\"");
            if (full_text.len) buf_add_json_escaped(&out, full_text.data, full_text.len);
            buf_adds(&out, "\"}]}");
        }
        for (int t = 0; t < n_emitted_tcs; t++) {
            const ToolCall *tc = &emitted_tcs[t];
            if (out_n++) buf_adds(&out, ",");
            buf_adds(&out, "{\"type\":\"function_call\",\"id\":\"fc_");
            if (tc->id.len) buf_add_json_escaped(&out, tc->id.data, tc->id.len);
            buf_adds(&out, "\",\"call_id\":\"");
            if (tc->id.len) buf_add_json_escaped(&out, tc->id.data, tc->id.len);
            buf_adds(&out, "\",\"name\":\"");
            if (tc->name.len) buf_add_json_escaped(&out, tc->name.data, tc->name.len);
            buf_adds(&out, "\",\"arguments\":\"");
            if (tc->args_json.len) buf_add_json_escaped(&out, tc->args_json.data, tc->args_json.len);
            else buf_adds(&out, "{}");
            buf_adds(&out, "\",\"status\":\"completed\"}");
        }
        char utail[200];
        long long lc = ink_hparams(sv->sess) ? (long long)ink_hparams(sv->sess)->loop_count : 3;
        snprintf(utail, sizeof utail,
                 "],\"usage\":{\"input_tokens\":%d,\"output_tokens\":%d,\"total_tokens\":%d,\"loop_count\":%lld}}",
                 n_prompt, n_gen, n_prompt + n_gen, lc);
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
        if (n_emitted_tcs > 0) {
            buf_adds(&out, ",\"tool_calls\":[");
            for (int t = 0; t < n_emitted_tcs; t++) {
                if (t > 0) buf_addc(&out, ',');
                buf_adds(&out, "{\"id\":\"");
                buf_add_json_escaped(&out, emitted_tcs[t].id.data, emitted_tcs[t].id.len);
                buf_adds(&out, "\",\"type\":\"function\",\"function\":{\"name\":\"");
                if (emitted_tcs[t].name.len) buf_add_json_escaped(&out, emitted_tcs[t].name.data, emitted_tcs[t].name.len);
                buf_adds(&out, "\",\"arguments\":\"");
                if (emitted_tcs[t].args_json.len) buf_add_json_escaped(&out, emitted_tcs[t].args_json.data, emitted_tcs[t].args_json.len);
                else buf_adds(&out, "{}");
                buf_adds(&out, "\"}}");
            }
            buf_addc(&out, ']');
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

    for (int t = 0; t < n_emitted_tcs; t++) tc_free(&emitted_tcs[t]);
    free(emitted_tcs);
    buf_free(&header_buf);
    buf_free(&tool_json);
    buf_free(&pending);
    buf_free(&full_text);
    buf_free(&full_think);
    ink_mutex_unlock(&sv->sess_mu);
    ink_mutex_lock(&sv->state_mu);
    sv->in_flight--;
    sv->last_active = now_sec();
    ink_mutex_unlock(&sv->state_mu);
    free(js_sc);
    req_free(&r);
}

static int read_http_request(int fd, char *method, size_t mcap, char *path, size_t pcap, Buf *body) {
    Buf raw = {0};
    char tmp[8192];
    size_t hdr_end = 0;
    size_t content_len = 0;
    int have_hdr = 0;

    while (raw.len < 4 * 1024 * 1024) {
        ssize_t n = sock_recv(fd, tmp, sizeof tmp);
        if (n < 0) { if (sock_interrupted()) continue; buf_free(&raw); return 1; }
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

typedef struct {
    Server *sv;
    int     cfd;
} ConnArg;

static void conn_thread(void *arg) {
    ConnArg *ca = (ConnArg *)arg;
    Server *sv = ca->sv;
    int cfd = ca->cfd;
    free(ca);

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
            ink_mutex_lock(&sv->state_mu);
            int busy = sv->in_flight > 0;
            ink_mutex_unlock(&sv->state_mu);
            char ok[192];
            snprintf(ok, sizeof ok,
                     "{\"status\":\"ok\",\"engine\":\"%s\",\"ctx\":%d,\"busy\":%s}",
                     sv->engine, sv->max_ctx, busy ? "true" : "false");
            send_http_json(cfd, 200, "OK", ok, strlen(ok));
        } else if (!strcmp(method, "GET") && !strcmp(path, "/v1/models")) {
            char ms[224];
            snprintf(ms, sizeof ms,
                     "{\"object\":\"list\",\"data\":[{\"id\":\"inkling-2-mini\",\"object\":\"model\",\"owned_by\":\"%s\"}]}",
                     sv->engine);
            send_http_json(cfd, 200, "OK", ms, strlen(ms));
        } else if (!strcmp(method, "POST") && !strcmp(path, "/v1/chat/completions")) {
            handle_inference(sv, cfd, 0, body.data ? body.data : "", body.len);
        } else if (!strcmp(method, "POST") && !strcmp(path, "/v1/responses")) {
            handle_inference(sv, cfd, 1, body.data ? body.data : "", body.len);
        } else {
            const char *nf = "{\"error\":{\"message\":\"not found\"}}";
            send_http_json(cfd, 404, "Not Found", nf, strlen(nf));
        }
    }
    buf_free(&body);
    sock_close(cfd);
}

int main(int argc, char **argv) {
    const char *gguf = NULL;
    const char *metal = NULL;
    const char *host = "127.0.0.1";
    int port = 18794;
    int max_ctx = 32768;
    int idle_exit_s = 0;

    for (int i = 1; i < argc; i++) {
        if ((!strcmp(argv[i], "--model") || !strcmp(argv[i], "-m")) && i + 1 < argc) gguf = argv[++i];
        else if ((!strcmp(argv[i], "--kernels") || !strcmp(argv[i], "--metal")) && i + 1 < argc) metal = argv[++i];
        else if (!strcmp(argv[i], "--host") && i + 1 < argc) host = argv[++i];
        else if (!strcmp(argv[i], "--port") && i + 1 < argc) port = atoi(argv[++i]);
        else if ((!strcmp(argv[i], "--ctx") || !strcmp(argv[i], "--ctx-size")) && i + 1 < argc) max_ctx = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--idle-exit-s") && i + 1 < argc) idle_exit_s = atoi(argv[++i]);
    }
    if (!gguf || !metal) {
        fprintf(stderr, "usage: %s --model PATH.gguf --kernels PATH [--host 127.0.0.1] [--port 18794] [--ctx 32768] [--idle-exit-s 0]\n"
                        "  --kernels  the engine's generated kernel source: inkling-kernels.metal or inkling-kernels-cuda.cu (--metal is the same option)\n", argv[0]);
        return 2;
    }

    if (sock_start() != 0) { fprintf(stderr, "[psionic-inkling-serve] cannot start sockets\n"); return 1; }

    double t0 = now_sec();
    Server sv;
    memset(&sv, 0, sizeof sv);
    ink_mutex_init(&sv.sess_mu);
    ink_mutex_init(&sv.state_mu);
    sv.last_active = t0;
    sv.idle_exit_s = idle_exit_s;
    sv.max_ctx = max_ctx;
    sv.rng = (uint64_t)time(NULL) ^ 0x9E3779B97F4A7C15ULL;
    char err[512];
    if (ink_open(gguf, metal, max_ctx, &sv.sess, err, sizeof err) != 0) {
        fprintf(stderr, "[psionic-inkling-serve] ink_open failed: %s\n", err);
        ink_mutex_destroy(&sv.sess_mu);
        ink_mutex_destroy(&sv.state_mu);
        return 1;
    }
    if (vocab_table_build(&sv) != 0) {
        fprintf(stderr, "[psionic-inkling-serve] failed to build vocab table\n");
        ink_close(sv.sess);
        ink_mutex_destroy(&sv.sess_mu);
        ink_mutex_destroy(&sv.state_mu);
        return 1;
    }
    sv.engine          = ink_backend_name(sv.sess);
    sv.max_ctx         = ink_max_ctx(sv.sess);
    sv.tok_model       = ink_find_token(sv.sess, "<|message_model|>");
    sv.tok_thinking    = ink_find_token(sv.sess, "<|content_thinking|>");
    sv.tok_text        = ink_find_token(sv.sess, "<|content_text|>");
    sv.tok_invoke_tool = ink_find_token(sv.sess, "<|content_invoke_tool_json|>");
    sv.tok_end_msg     = ink_find_token(sv.sess, "<|end_message|>");
    sv.tok_eog         = ink_eos_token(sv.sess);
    sv.last_active     = now_sec();
    fprintf(stderr, "[psionic-inkling-serve] loaded %s on %s in %.2fs (ctx=%d, vocab=%d, eog=%d, idle_exit=%ds)\n",
            gguf, sv.engine, now_sec() - t0, sv.max_ctx, sv.n_vocab, sv.tok_eog, idle_exit_s);

    int srv = (int)socket(AF_INET, SOCK_STREAM, 0);
    if (srv < 0) { sock_error("socket"); vocab_table_free(&sv); ink_close(sv.sess); ink_mutex_destroy(&sv.sess_mu); ink_mutex_destroy(&sv.state_mu); return 1; }
    int one = 1;
    setsockopt(srv, SOL_SOCKET, SO_REUSEADDR, (const char *)&one, sizeof one);

    struct sockaddr_in addr;
    memset(&addr, 0, sizeof addr);
    addr.sin_family = AF_INET;
    addr.sin_port = htons((uint16_t)port);
    if (inet_pton(AF_INET, host, &addr.sin_addr) != 1) {
        fprintf(stderr, "[psionic-inkling-serve] invalid host %s\n", host);
        sock_close(srv); vocab_table_free(&sv); ink_close(sv.sess); ink_mutex_destroy(&sv.sess_mu); ink_mutex_destroy(&sv.state_mu); return 1;
    }
    if (bind(srv, (struct sockaddr *)&addr, sizeof addr) != 0) {
        sock_error("bind"); sock_close(srv); vocab_table_free(&sv); ink_close(sv.sess); ink_mutex_destroy(&sv.sess_mu); ink_mutex_destroy(&sv.state_mu); return 1;
    }
    if (listen(srv, 32) != 0) {
        sock_error("listen"); sock_close(srv); vocab_table_free(&sv); ink_close(sv.sess); ink_mutex_destroy(&sv.sess_mu); ink_mutex_destroy(&sv.state_mu); return 1;
    }
    stop_signals(srv);
    fprintf(stderr, "[psionic-inkling-serve] listening on http://%s:%d (%s, ctx=%d)\n", host, port, sv.engine, sv.max_ctx);
    fflush(stderr);

    while (!g_stop) {
        int pr = sock_poll_read(srv, 500);
        if (pr < 0) {
            if (!g_stop && sock_interrupted()) continue;
            break;
        }
        if (pr == 0) {
            if (sv.idle_exit_s > 0) {
                ink_mutex_lock(&sv.state_mu);
                int busy = sv.in_flight > 0;
                double idle = now_sec() - sv.last_active;
                ink_mutex_unlock(&sv.state_mu);
                if (!busy && idle >= (double)sv.idle_exit_s) {
                    fprintf(stderr, "[psionic-inkling-serve] idle %.0fs >= %ds, exiting cleanly\n",
                            idle, sv.idle_exit_s);
                    fflush(stderr);
                    break;
                }
            }
            continue;
        }
        struct sockaddr_in cli;
        socklen_t clen = sizeof cli;
        int cfd = (int)accept(srv, (struct sockaddr *)&cli, &clen);
        if (cfd < 0) {
            if (!g_stop && sock_interrupted()) continue;
            break;
        }
        ConnArg *ca = (ConnArg *)malloc(sizeof *ca);
        if (!ca) { sock_close(cfd); continue; }
        ca->sv = &sv;
        ca->cfd = cfd;
        if (ink_spawn_detached(conn_thread, ca) != 0) {
            free(ca);
            sock_close(cfd);
        }
    }

    sock_close_listener(srv);
    /* Wait for any in-flight inference thread (already winding down because g_stop=1) before freeing the engine. */
    ink_mutex_lock(&sv.sess_mu);
    vocab_table_free(&sv);
    ink_close(sv.sess);
    sv.sess = NULL;
    ink_mutex_unlock(&sv.sess_mu);
    ink_mutex_destroy(&sv.sess_mu);
    ink_mutex_destroy(&sv.state_mu);
    return 0;
}
