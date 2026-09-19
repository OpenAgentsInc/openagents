const SYSTEMD_UNIT: &str = include_str!("../../../deploy/systemd/nostr-relay.service");
const BACKUP_UNIT: &str = include_str!("../../../deploy/backup/nostr-relay-backup.service");
const BACKUP_TIMER: &str = include_str!("../../../deploy/backup/nostr-relay-backup.timer");
const BACKUP_SCRIPT: &str = include_str!("../../../deploy/backup/nostr-relay-backup");
const CADDYFILE: &str = include_str!("../../../deploy/caddy/Caddyfile");
const NGINX: &str = include_str!("../../../deploy/nginx/nostr-relay.conf");
const DOCKERFILE: &str = include_str!("../../../Dockerfile");
const DEBIAN_RUNBOOK: &str = include_str!("../../../docs/deployment/runbook-debian-vps.md");

#[test]
fn github_workflows_are_forbidden() {
    assert!(!std::path::Path::new(".github/workflows").exists());
    assert!(include_str!("../../../AGENTS.md").contains("No GitHub workflows"));
}

#[test]
fn systemd_unit_is_fail_closed_and_sandboxed() {
    for required in [
        "Requires=postgresql.service",
        "EnvironmentFile=/etc/nostr-relay/nostr-relay.env",
        "ExecStart=/opt/nostr-relay/current/nostr-relay",
        "Restart=on-failure",
        "TimeoutStopSec=15",
        "IPAddressDeny=any",
        "IPAddressAllow=localhost",
        "SocketBindAllow=tcp:8080",
        "StateDirectory=nostr-relay",
        "ReadWritePaths=/var/lib/nostr-relay",
        "NoNewPrivileges=true",
        "ProtectSystem=strict",
        "ProtectHome=true",
        "PrivateDevices=true",
        "ProtectKernelTunables=true",
        "RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6",
        "RestrictNamespaces=true",
        "MemoryDenyWriteExecute=true",
        "SystemCallFilter=@system-service",
        "CapabilityBoundingSet=",
    ] {
        assert!(SYSTEMD_UNIT.contains(required), "missing {required}");
    }
    assert!(!SYSTEMD_UNIT.contains("DATABASE_URL="));
}

#[test]
fn proxy_templates_preserve_websocket_and_client_ip_contracts() {
    assert!(CADDYFILE.contains("reverse_proxy 127.0.0.1:8080"));
    for required in [
        "proxy_http_version 1.1",
        "proxy_set_header Upgrade $http_upgrade",
        "proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for",
        "proxy_set_header X-Real-IP $remote_addr",
        "proxy_buffering off",
        "client_max_body_size 10m",
        "proxy_request_buffering off",
    ] {
        assert!(NGINX.contains(required), "missing {required}");
    }
}

#[test]
fn backup_is_private_atomic_retained_and_scheduled() {
    for required in [
        "umask 077",
        "pg_dump --dbname=nostr_relay --format=custom",
        "--file=\"${temporary}\"",
        "mv -- \"${temporary}\" \"${destination}\"",
        "tar --create --file=\"${media_temporary}\"",
        "mv -- \"${media_temporary}\" \"${media_destination}\"",
        "-mtime \"+${retention_days}\" -delete",
    ] {
        assert!(BACKUP_SCRIPT.contains(required), "missing {required}");
    }
    assert!(BACKUP_UNIT.contains("User=postgres"));
    assert!(BACKUP_UNIT.contains("SupplementaryGroups=nostr-relay"));
    assert!(BACKUP_UNIT.contains("ReadWritePaths=/var/backups/nostr-relay"));
    assert!(BACKUP_UNIT.contains("ReadOnlyPaths=/var/lib/nostr-relay/media"));
    assert!(BACKUP_TIMER.contains("Persistent=true"));
    assert!(BACKUP_TIMER.contains("OnCalendar=*-*-* 03:30:00 UTC"));
}

#[test]
fn container_remains_one_unprivileged_binary() {
    assert!(DOCKERFILE.contains("cargo build --locked --release"));
    assert!(DOCKERFILE.contains("FROM debian:13-slim"));
    assert!(DOCKERFILE.contains("USER 10001:10001"));
    assert!(DOCKERFILE.contains("ENTRYPOINT [\"/usr/local/bin/nostr-relay\"]"));
    assert_eq!(DOCKERFILE.matches("ENTRYPOINT").count(), 1);
}

#[test]
fn restore_procedure_checks_the_real_schema() {
    assert!(DEBIAN_RUNBOOK.contains("SELECT count(*) FROM nostr_event;"));
    assert!(!DEBIAN_RUNBOOK.contains("SELECT count(*) FROM events;"));
}
