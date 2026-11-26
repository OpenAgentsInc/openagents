// Firm reason: 6 (owner-only 0600 AF_UNIX attach server ~/.openagents/desktop/browser.sock with getpeereid/SO_PEERCRED verification).
// Spec: docs/browser/desktop-browser-pane-spec.md (§6.3, §8.1, §9.5); Issue #14108.
#include "br_attach.h"

#if !defined(_WIN32)
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/un.h>
#include <unistd.h>

#define OA_BR_ATTACH_MAX_CLIENTS 8
#define OA_BR_ATTACH_BUF 65536

typedef struct {
    int fd;
    char in_buf[OA_BR_ATTACH_BUF];
    int in_len;
} OaBrAttachClient;

struct OaBrAttachServer {
    int listen_fd;
    char path[256];
    OaBrAttachHandler handler;
    void *ctx;
    OaBrAttachClient clients[OA_BR_ATTACH_MAX_CLIENTS];
};

static bool verify_peer_uid(int fd) {
    uid_t my_uid = geteuid();
#if defined(__APPLE__)
    uid_t peer_uid = (uid_t)-1;
    gid_t peer_gid = (gid_t)-1;
    if (getpeereid(fd, &peer_uid, &peer_gid) != 0) return false;
    return peer_uid == my_uid;
#elif defined(__linux__) && defined(SO_PEERCRED)
    struct ucred cred;
    socklen_t len = sizeof(cred);
    if (getsockopt(fd, SOL_SOCKET, SO_PEERCRED, &cred, &len) != 0) return false;
    return cred.uid == my_uid;
#else
    (void)fd;
    (void)my_uid;
    return true;
#endif
}

static void mkdir_0700(const char *path) {
    char tmp[256];
    snprintf(tmp, sizeof(tmp), "%s", path);
    for (char *p = tmp + 1; *p; p++) {
        if (*p == '/') {
            *p = '\0';
            mkdir(tmp, 0700);
            *p = '/';
        }
    }
    mkdir(tmp, 0700);
}

OaBrAttachServer *oa_br_attach_start(const char *sock_path, OaBrAttachHandler handler, void *ctx) {
    if (!handler) return NULL;
    OaBrAttachServer *srv = (OaBrAttachServer *)calloc(1, sizeof(OaBrAttachServer));
    if (!srv) return NULL;
    srv->listen_fd = -1;
    srv->handler = handler;
    srv->ctx = ctx;
    for (int i = 0; i < OA_BR_ATTACH_MAX_CLIENTS; i++) srv->clients[i].fd = -1;

    if (sock_path && sock_path[0]) {
        snprintf(srv->path, sizeof(srv->path), "%s", sock_path);
    } else {
        const char *home = getenv("HOME");
        if (!home || !home[0]) { free(srv); return NULL; }
        snprintf(srv->path, sizeof(srv->path), "%s/.openagents/desktop/browser.sock", home);
    }

    // Ensure parent directory exists with 0700 permissions
    char dir[256];
    snprintf(dir, sizeof(dir), "%s", srv->path);
    char *slash = strrchr(dir, '/');
    if (slash) {
        *slash = '\0';
        mkdir_0700(dir);
        chmod(dir, 0700);
    }

    unlink(srv->path);
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    if (fd < 0) { free(srv); return NULL; }
    int fl = fcntl(fd, F_GETFL, 0);
    if (fl >= 0) fcntl(fd, F_SETFL, fl | O_NONBLOCK);

    struct sockaddr_un addr;
    memset(&addr, 0, sizeof(addr));
    addr.sun_family = AF_UNIX;
    if (strlen(srv->path) >= sizeof(addr.sun_path)) {
        close(fd);
        free(srv);
        return NULL;
    }
    snprintf(addr.sun_path, sizeof(addr.sun_path), "%s", srv->path);

    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) != 0) {
        close(fd);
        free(srv);
        return NULL;
    }
    chmod(srv->path, 0600);
    if (listen(fd, 8) != 0) {
        close(fd);
        unlink(srv->path);
        free(srv);
        return NULL;
    }
    srv->listen_fd = fd;
    return srv;
}

static void write_all_nb(int fd, const char *buf, int len) {
    int off = 0;
    while (off < len) {
        ssize_t w = write(fd, buf + off, (size_t)(len - off));
        if (w <= 0) break;
        off += (int)w;
    }
}

int oa_br_attach_poll(OaBrAttachServer *srv) {
    if (!srv || srv->listen_fd < 0) return 0;
    int handled = 0;

    // 1. Accept any pending local connections and verify peer UID
    for (;;) {
        int cfd = accept(srv->listen_fd, NULL, NULL);
        if (cfd < 0) break;
        if (!verify_peer_uid(cfd)) {
            close(cfd);
            continue;
        }
        int fl = fcntl(cfd, F_GETFL, 0);
        if (fl >= 0) fcntl(cfd, F_SETFL, fl | O_NONBLOCK);
        int slot = -1;
        for (int i = 0; i < OA_BR_ATTACH_MAX_CLIENTS; i++) {
            if (srv->clients[i].fd < 0) { slot = i; break; }
        }
        if (slot < 0) {
            close(cfd);
            continue;
        }
        srv->clients[slot].fd = cfd;
        srv->clients[slot].in_len = 0;
    }

    // 2. Read newline-delimited JSON commands from verified peers
    static char resp[OA_BR_ATTACH_BUF];
    for (int i = 0; i < OA_BR_ATTACH_MAX_CLIENTS; i++) {
        OaBrAttachClient *cl = &srv->clients[i];
        if (cl->fd < 0) continue;
        ssize_t r = read(cl->fd, cl->in_buf + cl->in_len, (size_t)(OA_BR_ATTACH_BUF - 1 - cl->in_len));
        if (r == 0 || (r < 0 && errno != EAGAIN && errno != EWOULDBLOCK)) {
            close(cl->fd);
            cl->fd = -1;
            cl->in_len = 0;
            continue;
        }
        if (r > 0) cl->in_len += (int)r;
        cl->in_buf[cl->in_len] = '\0';

        char *nl = NULL;
        while ((nl = strchr(cl->in_buf, '\n')) != NULL) {
            *nl = '\0';
            if (cl->in_buf[0] != '\0') {
                int rn = srv->handler(srv->ctx, cl->in_buf, resp, OA_BR_ATTACH_BUF - 2);
                if (rn < 0) rn = snprintf(resp, OA_BR_ATTACH_BUF - 2, "{\"ok\":false,\"error\":\"internal\"}");
                if (rn > OA_BR_ATTACH_BUF - 2) rn = OA_BR_ATTACH_BUF - 2;
                resp[rn++] = '\n';
                write_all_nb(cl->fd, resp, rn);
                handled++;
            }
            int remain = cl->in_len - (int)((nl - cl->in_buf) + 1);
            if (remain > 0) memmove(cl->in_buf, nl + 1, (size_t)remain);
            cl->in_len = remain;
            cl->in_buf[cl->in_len] = '\0';
        }
    }
    return handled;
}

bool oa_br_attach_listening(const OaBrAttachServer *srv) {
    return srv && srv->listen_fd >= 0;
}

const char *oa_br_attach_path(const OaBrAttachServer *srv) {
    return srv ? srv->path : "";
}

void oa_br_attach_stop(OaBrAttachServer *srv) {
    if (!srv) return;
    for (int i = 0; i < OA_BR_ATTACH_MAX_CLIENTS; i++) {
        if (srv->clients[i].fd >= 0) close(srv->clients[i].fd);
    }
    if (srv->listen_fd >= 0) close(srv->listen_fd);
    if (srv->path[0]) unlink(srv->path);
    free(srv);
}

#else
// Windows stub (token-authenticated AF_UNIX implementation in windows/browser_webview2.c)
#include <stdlib.h>
struct OaBrAttachServer { int unused; };
OaBrAttachServer *oa_br_attach_start(const char *sock_path, OaBrAttachHandler handler, void *ctx) { (void)sock_path; (void)handler; (void)ctx; return NULL; }
int oa_br_attach_poll(OaBrAttachServer *srv) { (void)srv; return 0; }
bool oa_br_attach_listening(const OaBrAttachServer *srv) { (void)srv; return false; }
const char *oa_br_attach_path(const OaBrAttachServer *srv) { (void)srv; return ""; }
void oa_br_attach_stop(OaBrAttachServer *srv) { free(srv); }
#endif
