#!/usr/bin/env python3
"""Provision and deploy coder-sync using the automation Google Cloud config."""
import argparse
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.parse
import urllib.request

PROJECT = "openagentsgemini"
REGION = "us-central1"
INSTANCE = "coder-sync-pg"
ACCOUNT = f"coder-sync@{PROJECT}.iam.gserviceaccount.com"
BUILD_ACCOUNT = f"projects/{PROJECT}/serviceAccounts/oa-mvp-automation@{PROJECT}.iam.gserviceaccount.com"
ROOT = Path(__file__).resolve().parents[2]
ARGS = None
os.environ.setdefault("CLOUDSDK_CONFIG", "/Users/christopherdavid/work/.secrets/gcloud-sa-config")
os.environ.update(BROWSER="/usr/bin/true", NO_OPEN_BROWSER="1")


def g(*args, optional=False):
    result = subprocess.run(["gcloud", *args, "--project", PROJECT, "--quiet"], text=True, capture_output=True)
    if result.returncode:
        if optional and ("NOT_FOUND" in result.stderr or "does not exist" in result.stderr or "not found" in result.stderr.lower() or "404" in result.stderr):
            return None
        # Arguments never include database passwords or account tokens.
        sys.stderr.write(result.stderr)
        raise RuntimeError("Google Cloud command failed: " + " ".join(args[:3]))
    return result.stdout.strip()


def sql_user(name, password):
    # Send the password in the HTTPS body, never argv, a log, or a source file.
    token = g("auth", "print-access-token")
    data = json.dumps({"name": name, "password": password}).encode()
    request = urllib.request.Request(
        f"https://sqladmin.googleapis.com/sql/v1beta4/projects/{PROJECT}/instances/{INSTANCE}/users",
        data=data, headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"}, method="POST")
    with urllib.request.urlopen(request, timeout=60) as response:
        operation = json.load(response)["name"]
    g("sql", "operations", "wait", operation, "--timeout=300")


def lifecycle():
    for suffix in ("-staging", ""):
        g("storage", "buckets", "update", f"gs://{PROJECT}-coder-sync{suffix}",
          f"--lifecycle-file={ROOT / 'deploy/coder-sync/lifecycle.json'}")
        print(f"Lifecycle ready: coder-sync{suffix}")


def provision():
    if g("iam", "service-accounts", "describe", ACCOUNT, "--format=value(email)", optional=True) is None:
        g("iam", "service-accounts", "create", "coder-sync", "--display-name=Coder sync")
    try:
        g("projects", "add-iam-policy-binding", PROJECT, f"--member=serviceAccount:{ACCOUNT}", "--role=roles/cloudsql.client", "--condition=None")
    except RuntimeError:
        print(f"Owner step: gcloud projects add-iam-policy-binding {PROJECT} --member=serviceAccount:{ACCOUNT} --role=roles/cloudsql.client --condition=None", flush=True)

    instance = g("sql", "instances", "describe", INSTANCE, "--format=value(state)", optional=True)
    if instance is None:
        g("sql", "instances", "create", INSTANCE, "--database-version=POSTGRES_17", "--edition=ENTERPRISE", "--tier=db-f1-micro",
          f"--region={REGION}", "--storage-size=10GB", "--storage-type=SSD", "--availability-type=ZONAL", "--async")
        print("Cloud SQL creation started. Run provision again after it becomes RUNNABLE.")
        return
    if instance != "RUNNABLE":
        raise RuntimeError("Cloud SQL is still starting; run provision again when it is RUNNABLE.")
    for staging in (True, False):
        suffix = "-staging" if staging else ""
        database = "coder_sync_staging" if staging else "coder_sync"
        bucket = f"{PROJECT}-coder-sync{suffix}"
        secret = f"coder-sync{suffix}-database-url"
        if g("sql", "databases", "describe", database, f"--instance={INSTANCE}", optional=True) is None:
            g("sql", "databases", "create", database, f"--instance={INSTANCE}")
        if g("storage", "buckets", "describe", f"gs://{bucket}", optional=True) is None:
            g("storage", "buckets", "create", f"gs://{bucket}", f"--location={REGION}", "--uniform-bucket-level-access", "--public-access-prevention")
        g("storage", "buckets", "add-iam-policy-binding", f"gs://{bucket}", f"--member=serviceAccount:{ACCOUNT}", "--role=roles/storage.objectAdmin")
        if g("secrets", "describe", secret, optional=True) is None:
            g("secrets", "create", secret, "--replication-policy=automatic")
        versions = json.loads(g("secrets", "versions", "list", secret, "--filter=state:ENABLED", "--format=json(name)"))
        if not versions:
            password = secrets.token_urlsafe(40)
            sql_user(database, password)
            dsn = f"host=/cloudsql/{PROJECT}:{REGION}:{INSTANCE} user={database} dbname={database} password={password}"
            subprocess.run(["gcloud", "secrets", "versions", "add", secret, "--data-file=-", "--project", PROJECT, "--quiet"],
                           input=dsn, text=True, stdout=subprocess.DEVNULL, check=True)
        g("secrets", "add-iam-policy-binding", secret, f"--member=serviceAccount:{ACCOUNT}", "--role=roles/secretmanager.secretAccessor")
        print(f"Ready: {database}, gs://{bucket}, {secret}")
    lifecycle()


def revision():
    if subprocess.check_output(["git", "status", "--porcelain"], cwd=ROOT, text=True).strip():
        raise RuntimeError("Commit changes before building or deploying.")
    subprocess.run(["git", "fetch", "origin", "main"], cwd=ROOT, check=True)
    if subprocess.run(["git", "merge-base", "--is-ancestor", "origin/main", "HEAD"], cwd=ROOT).returncode:
        raise RuntimeError("Rebase on origin/main before building or deploying.")
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


# The Dockerfile copies these exported source trees. Include the shared host
# client's dependencies as well as the server and app sync clients.
# questions/: jev embeds its decision questions (crates/jev include_str!).
BUILD_PATHS = ["Cargo.toml", "Cargo.lock", "nips/openagents/schemas", "questions", "deploy/coder-sync", "apps/openagents/Cargo.toml"] + [
    f"crates/{name}" for name in (
        'agent-contract',
        'agent-lab',
        'atif',
        'chat-home',
        'coder-access',
        'coder-boundary',
        'coder-connect',
        'coder-engine-status',
        'coder-history',
        'coder-host',
        'coder-host-wire',
        'coder-link',
        'coder-pty',
        'coder-reach',
        'coder-sync',
        'github-actions',
        'jev',
        'lab-kit',
        'markdown-stream',
        'model-access',
        'nostr',
        'nostr-transport',
        'oa-copy',
        'openagents-chat',
        'openagents-connect',
        'openagents-dashboard',
        'openagents-login',
        'openagents-sdk',
        'openui-lang',
        'paper-mono',
        'playtest',
        'push-gateway',
        'private-fs',
        'receipts',
        'robot-description',
        'robot-lab',
        'route-contract',
        'rust-native',
        'secret-screen',
        'user-home',
        'workbench',
)] + [
    f"apps/openagents/crates/{name}" for name in ("brand", "client", "doc", "orb", "proto", "rpc", "sync", "voice-session")]


def build():
    commit = revision()
    image = f"us-central1-docker.pkg.dev/{PROJECT}/openagents/coder-sync"
    published = g("artifacts", "docker", "images", "describe", f"{image}:{commit}", "--format=value(image_summary.digest)", optional=True)
    if published:
        print(f"Already published: {image}@{published}")
        return
    versions = json.loads(g("artifacts", "docker", "images", "list", image, "--include-tags", "--format=json"))
    builders = [v for v in versions if any(t == "builder" or t.startswith("builder-") for t in v.get("tags", []))]
    cache = "rust:1.98.1-slim-trixie"
    if builders:
        cache = f"{image}@{max(builders, key=lambda v: v['createTime'])['version']}"
    scratch = os.environ.get("OPENAGENTS_SCRATCH") or subprocess.check_output(["openagents", "scratch"], text=True).strip()
    context = Path(tempfile.mkdtemp(prefix="coder-sync-build-", dir=scratch))
    try:
        archive = subprocess.Popen(["git", "archive", commit, *BUILD_PATHS], cwd=ROOT, stdout=subprocess.PIPE)
        subprocess.run(["tar", "-x", "-C", str(context)], stdin=archive.stdout, check=True)
        archive.stdout.close()
        if archive.wait():
            raise RuntimeError("Could not export the committed build.")
        print(g("builds", "submit", str(context), "--config", str(context / "deploy/coder-sync/cloudbuild.yaml"),
                f"--service-account={BUILD_ACCOUNT}", f"--substitutions=_TAG={commit},_CACHE_IMAGE={cache}", "--async", "--format=value(id)"))
    finally:
        shutil.rmtree(context)


def deploy(staging):
    commit = revision()
    suffix = "-staging" if staging else ""
    service = f"coder-sync{suffix}"
    environment = {
        "CODER_SYNC_ENV": "staging" if staging else "production",
        "CODER_SYNC_AUTH": "openagents",
        "CODER_SYNC_BUCKET": f"{PROJECT}-coder-sync{suffix}",
        "CODER_SYNC_REVISION": commit,
        # One log line per request blocked the runtime on a backed-up stdout
        # under reconnect floods (#12120); warnings and errors only.
        "RUST_LOG": "warn",
    }
    public_origin = os.environ.get("CODER_SYNC_PUBLIC_ORIGIN", "").strip().rstrip("/")
    if public_origin:
        parsed = urllib.parse.urlsplit(public_origin)
        if (parsed.scheme != "https" or not parsed.hostname or parsed.username is not None
                or parsed.password is not None or parsed.path or parsed.query or parsed.fragment
                or "," in public_origin):
            raise RuntimeError("Set CODER_SYNC_PUBLIC_ORIGIN to an HTTPS origin without a path.")
        environment["CODER_SYNC_PUBLIC_ORIGIN"] = public_origin
    # One instance (#12130). Device rooms and live chat rows live in the
    # instance's memory, so a host and a client on different instances never
    # meet: the room answers host_offline and live updates go missing. The
    # #12120 mitigation ran up to three instances because a lone instance
    # kept being taken out of rotation; #12128 found why (answers to unread
    # multi-MB uploads reset the frontend's connection) and the server now
    # drains refused bodies. The service allows two during a rollout so the
    # previous revision's socket-held instance can't block the new one, then
    # drops back to one, which stops that instance and moves its sockets
    # over. Production serves from the single-writer VM (#12122, `vm-deploy`);
    # this service stays current as its rollback (`cutover run`).
    args = ["run", "deploy", service, f"--region={REGION}", f"--image=us-central1-docker.pkg.dev/{PROJECT}/openagents/coder-sync:{commit}",
            f"--service-account={ACCOUNT}", "--max=2", "--min=0", "--max-instances=1", "--min-instances=1", "--timeout=3600", "--concurrency=1000",
            # A frozen process gets restarted, not left answering "no available
            # instance" (#12120). /health reads the database through the pool.
            "--liveness-probe=httpGet.path=/health,httpGet.port=8080,periodSeconds=10,timeoutSeconds=3,failureThreshold=3", "--cpu=1", "--memory=512Mi", "--allow-unauthenticated",
            f"--add-cloudsql-instances={PROJECT}:{REGION}:{INSTANCE}",
            f"--update-secrets=CODER_SYNC_DATABASE_URL=coder-sync{suffix}-database-url:latest",
            "--update-env-vars=" + ",".join(f"{key}={value}" for key, value in environment.items()),
            "--args="]
    g(*args)
    # `run deploy` leaves a pinned traffic split alone: on 2026-10-18 traffic
    # stayed on 00018 through two later revisions and hid both (#12128).
    g("run", "services", "update-traffic", service, f"--region={REGION}", "--to-latest")
    status = json.loads(g("run", "services", "describe", service, f"--region={REGION}", "--format=json"))["status"]
    latest = status.get("latestReadyRevisionName")
    serving = {t.get("revisionName"): t.get("percent", 0) for t in status.get("traffic", [])}
    if latest != status.get("latestCreatedRevisionName") or serving.get(latest) != 100:
        raise RuntimeError(f"The new revision is not serving all traffic: {serving}.")
    g("run", "services", "update", service, f"--region={REGION}", "--max=1")
    print(f"{latest} serves 100% of traffic.")
    print(status["url"])


# The single-writer VM (#12122). One process holds every room, so a live row
# reaches every socket. A global external HTTPS load balancer fronts
# sync.openagents.com with two backends: the VM and, for rollback, the Cloud
# Run service through a serverless NEG. `cutover` picks the one it serves; a
# request with `x-coder-sync-backend: vm` or `run` reaches that backend
# whichever one serves, so either can be checked before traffic moves.
ZONE = f"{REGION}-a"
VM = "coder-sync-1"
VM_TAG = "coder-sync"
VM_MACHINE = "e2-standard-2"
VM_GROUP = "coder-sync-ig"
VM_BACKEND = "coder-sync-vm"
RUN_BACKEND = "coder-sync-run"
RUN_NEG = "coder-sync-run-neg"
HEALTH = "coder-sync-health"
URL_MAP = "coder-sync"
PROXY = "coder-sync-https"
RULE = "coder-sync-https"
LB_IP = "coder-sync-ip"
DOMAIN = "sync.openagents.com"
CERT = "coder-sync-cert"
CERT_MAP = "coder-sync-map"
DNS_AUTH = "coder-sync-dns"
IMAGE = f"us-central1-docker.pkg.dev/{PROJECT}/openagents/coder-sync"
# The load balancer closes a websocket after the backend timeout: one day.
SOCKET_SECONDS = 86400
CLOUD_INIT = ROOT / "deploy/coder-sync-vm/cloud-init.yaml"
LB_RANGES = "130.211.0.0/22,35.191.0.0/16"


def vm_environment(commit):
    environment = {
        "CODER_SYNC_ENV": "production",
        "CODER_SYNC_AUTH": "openagents",
        "CODER_SYNC_BUCKET": f"{PROJECT}-coder-sync",
        "CODER_SYNC_REVISION": commit,
        "CODER_SYNC_PUBLIC_ORIGIN": f"https://{DOMAIN}",
        "RUST_LOG": "warn",
    }
    return "\n".join(f"{key}={value}" for key, value in environment.items()) + "\n"


def proven(commit):
    """Staging first: the image must already serve a Cloud Run service."""
    for service in ("coder-sync-staging", "coder-sync"):
        image = g("run", "services", "describe", service, f"--region={REGION}",
                  "--format=value(spec.template.spec.containers[0].image)", optional=True) or ""
        if image.endswith(":" + commit):
            return service
    raise RuntimeError(f"Deploy {commit[:10]} to coder-sync-staging first (deploy --staging).")


def vm_metadata():
    found = json.loads(g("compute", "instances", "describe", VM, f"--zone={ZONE}", "--format=json(metadata.items)"))
    return {item["key"]: item["value"] for item in found.get("metadata", {}).get("items", [])}


def metadata_files(values, scratch):
    files = []
    for key, value in values.items():
        path = Path(scratch) / key
        path.write_text(value)
        files.append(f"{key}={path}")
    return "--metadata-from-file=" + ",".join(files)


def lb_address():
    return g("compute", "addresses", "describe", LB_IP, "--global", "--format=value(address)")


def backend_health(backend, path="/health"):
    """GET `path` from one backend through the load balancer, before or after DNS points at it."""
    result = subprocess.run(
        ["curl", "-fsS", "-m", "10", "--resolve", f"{DOMAIN}:443:{lb_address()}",
         "-H", f"x-coder-sync-backend: {backend}", f"https://{DOMAIN}{path}"],
        text=True, capture_output=True)
    if result.returncode:
        return None
    try:
        return json.loads(result.stdout)
    except ValueError:
        return None


def wait_vm(commit, seconds=600):
    deadline = time.time() + seconds
    while time.time() < deadline:
        health = backend_health("vm") or {}
        if health.get("ok") and health.get("revision") == commit:
            print(f"{VM} serves {commit[:10]}.")
            return
        time.sleep(5)
    raise RuntimeError(f"{VM} did not serve {commit[:10]} within {seconds} s; run vm-rollback.")


def url_map(active):
    services = {key: f"projects/{PROJECT}/global/backendServices/{name}" for key, name in (("vm", VM_BACKEND), ("run", RUN_BACKEND))}
    route = lambda priority, value, name: {
        "priority": priority, "service": services[name],
        "matchRules": [{"prefixMatch": "/", "headerMatches": [{"headerName": "x-coder-sync-backend", "exactMatch": value}]}]}
    return {
        "name": URL_MAP,
        "defaultService": services[active],
        "hostRules": [{"hosts": ["*"], "pathMatcher": "main"}],
        "pathMatchers": [{"name": "main", "defaultService": services[active],
                          "routeRules": [route(1, "vm", "vm"), route(2, "run", "run")]}],
    }


def import_url_map(active):
    with tempfile.TemporaryDirectory() as scratch:
        path = Path(scratch) / "url-map.json"
        path.write_text(json.dumps(url_map(active)))
        g("compute", "url-maps", "import", URL_MAP, "--global", f"--source={path}")


def serving():
    found = g("compute", "url-maps", "describe", URL_MAP, "--global", "--format=value(defaultService)")
    return "vm" if found.endswith("/" + VM_BACKEND) else "run"


def exists(*args):
    return g(*args, "--format=value(name)", optional=True) is not None


def vm_provision():
    """Create the VM, its backend, and the load balancer. Every step is idempotent."""
    commit = run_commit()
    # The VM pulls the image itself (Cloud Run used its service agent).
    g("artifacts", "repositories", "add-iam-policy-binding", "openagents", f"--location={REGION}",
      f"--member=serviceAccount:{ACCOUNT}", "--role=roles/artifactregistry.reader")
    for role in ("roles/logging.logWriter", "roles/monitoring.metricWriter"):
        try:
            g("projects", "add-iam-policy-binding", PROJECT, f"--member=serviceAccount:{ACCOUNT}", f"--role={role}", "--condition=None")
        except RuntimeError:
            # Without it the VM keeps its logs on disk only.
            print(f"Owner step: gcloud projects add-iam-policy-binding {PROJECT} --member=serviceAccount:{ACCOUNT} --role={role} --condition=None", flush=True)
    if not exists("compute", "firewall-rules", "describe", "coder-sync-lb"):
        g("compute", "firewall-rules", "create", "coder-sync-lb", "--network=default", "--direction=INGRESS",
          "--allow=tcp:8080", f"--source-ranges={LB_RANGES}", f"--target-tags={VM_TAG}")
    if not exists("compute", "instances", "describe", VM, f"--zone={ZONE}"):
        with tempfile.TemporaryDirectory() as scratch:
            values = {"coder-sync-image": f"{IMAGE}:{commit}", "coder-sync-env": vm_environment(commit),
                      "coder-sync-secret": "coder-sync-database-url", "coder-sync-sql": f"{PROJECT}:{REGION}:{INSTANCE}",
                      "user-data": CLOUD_INIT.read_text()}
            g("compute", "instances", "create", VM, f"--zone={ZONE}", f"--machine-type={VM_MACHINE}",
              "--image-family=cos-stable", "--image-project=cos-cloud", "--boot-disk-size=20GB", "--boot-disk-type=pd-balanced",
              f"--service-account={ACCOUNT}", "--scopes=cloud-platform", f"--tags={VM_TAG}",
              "--maintenance-policy=MIGRATE", "--restart-on-failure", "--shielded-secure-boot",
              "--metadata=google-logging-enabled=true,google-monitoring-enabled=true,enable-oslogin=TRUE",
              metadata_files(values, scratch))
    if not exists("compute", "instance-groups", "unmanaged", "describe", VM_GROUP, f"--zone={ZONE}"):
        g("compute", "instance-groups", "unmanaged", "create", VM_GROUP, f"--zone={ZONE}")
        g("compute", "instance-groups", "unmanaged", "add-instances", VM_GROUP, f"--zone={ZONE}", f"--instances={VM}")
    g("compute", "instance-groups", "unmanaged", "set-named-ports", VM_GROUP, f"--zone={ZONE}", "--named-ports=http:8080")
    if not exists("compute", "health-checks", "describe", HEALTH):
        g("compute", "health-checks", "create", "http", HEALTH, "--port=8080", "--request-path=/health",
          "--check-interval=5s", "--timeout=3s", "--healthy-threshold=1", "--unhealthy-threshold=3")
    if not exists("compute", "backend-services", "describe", VM_BACKEND, "--global"):
        g("compute", "backend-services", "create", VM_BACKEND, "--global", "--load-balancing-scheme=EXTERNAL_MANAGED",
          "--protocol=HTTP", "--port-name=http", f"--health-checks={HEALTH}", f"--timeout={SOCKET_SECONDS}s",
          "--connection-draining-timeout=30s", "--enable-logging", "--logging-sample-rate=0.1")
        g("compute", "backend-services", "add-backend", VM_BACKEND, "--global", f"--instance-group={VM_GROUP}",
          f"--instance-group-zone={ZONE}", "--balancing-mode=UTILIZATION", "--max-utilization=1.0", "--capacity-scaler=1.0")
    if not exists("compute", "network-endpoint-groups", "describe", RUN_NEG, f"--region={REGION}"):
        g("compute", "network-endpoint-groups", "create", RUN_NEG, f"--region={REGION}",
          "--network-endpoint-type=serverless", "--cloud-run-service=coder-sync")
    if not exists("compute", "backend-services", "describe", RUN_BACKEND, "--global"):
        g("compute", "backend-services", "create", RUN_BACKEND, "--global", "--load-balancing-scheme=EXTERNAL_MANAGED")
        g("compute", "backend-services", "add-backend", RUN_BACKEND, "--global", f"--network-endpoint-group={RUN_NEG}",
          f"--network-endpoint-group-region={REGION}")
    if not exists("compute", "url-maps", "describe", URL_MAP, "--global"):
        # Cloud Run keeps serving until `cutover vm`.
        import_url_map("run")
    if not exists("certificate-manager", "dns-authorizations", "describe", DNS_AUTH, "--location=global"):
        g("certificate-manager", "dns-authorizations", "create", DNS_AUTH, f"--domain={DOMAIN}", "--location=global")
    if not exists("certificate-manager", "certificates", "describe", CERT, "--location=global"):
        g("certificate-manager", "certificates", "create", CERT, f"--domains={DOMAIN}", f"--dns-authorizations={DNS_AUTH}", "--location=global")
    if not exists("certificate-manager", "maps", "describe", CERT_MAP, "--location=global"):
        g("certificate-manager", "maps", "create", CERT_MAP, "--location=global")
    if not exists("certificate-manager", "maps", "entries", "describe", "sync", f"--map={CERT_MAP}", "--location=global"):
        g("certificate-manager", "maps", "entries", "create", "sync", f"--map={CERT_MAP}", f"--hostname={DOMAIN}",
          f"--certificates={CERT}", "--location=global")
    if not exists("compute", "addresses", "describe", LB_IP, "--global"):
        g("compute", "addresses", "create", LB_IP, "--global", "--ip-version=IPV4")
    if not exists("compute", "target-https-proxies", "describe", PROXY, "--global"):
        g("compute", "target-https-proxies", "create", PROXY, "--global", f"--url-map={URL_MAP}",
          f"--certificate-map={CERT_MAP}", f"--http-keep-alive-timeout-sec=610")
    if not exists("compute", "forwarding-rules", "describe", RULE, "--global"):
        g("compute", "forwarding-rules", "create", RULE, "--global", "--load-balancing-scheme=EXTERNAL_MANAGED",
          f"--address={LB_IP}", f"--target-https-proxy={PROXY}", "--ports=443")
    record = g("certificate-manager", "dns-authorizations", "describe", DNS_AUTH, "--location=global",
               "--format=value(dnsResourceRecord.name,dnsResourceRecord.data)")
    print(f"Load balancer address: {lb_address()}")
    print(f"Certificate DNS record (CNAME): {record}")
    print("Next: dns cert; when vm-status shows the certificate ACTIVE and the VM healthy, cutover vm, then dns lb.")


def run_commit():
    """The commit to run: --commit, else the commit production Cloud Run serves."""
    if ARGS.commit:
        return ARGS.commit
    image = g("run", "services", "describe", "coder-sync", f"--region={REGION}",
              "--format=value(spec.template.spec.containers[0].image)")
    return image.rsplit(":", 1)[1]


def vm_deploy():
    commit = ARGS.commit or revision()
    print(f"Proven on {proven(commit)}.")
    if g("artifacts", "docker", "images", "describe", f"{IMAGE}:{commit}", "--format=value(image_summary.digest)", optional=True) is None:
        raise RuntimeError(f"No image for {commit[:10]}; run build first.")
    current = vm_metadata()
    with tempfile.TemporaryDirectory() as scratch:
        g("compute", "instances", "add-metadata", VM, f"--zone={ZONE}", metadata_files({
            "coder-sync-previous-image": current["coder-sync-image"],
            "coder-sync-previous-env": current["coder-sync-env"],
            "coder-sync-image": f"{IMAGE}:{commit}",
            "coder-sync-env": vm_environment(commit),
            "user-data": CLOUD_INIT.read_text()}, scratch))
    wait_vm(commit)


def vm_rollback():
    current = vm_metadata()
    previous = current.get("coder-sync-previous-image")
    if not previous:
        raise RuntimeError("No previous image is recorded on the VM.")
    with tempfile.TemporaryDirectory() as scratch:
        g("compute", "instances", "add-metadata", VM, f"--zone={ZONE}", metadata_files({
            "coder-sync-previous-image": current["coder-sync-image"],
            "coder-sync-previous-env": current["coder-sync-env"],
            "coder-sync-image": previous,
            "coder-sync-env": current["coder-sync-previous-env"]}, scratch))
    wait_vm(previous.rsplit(":", 1)[1])


def cutover(target):
    if target == "vm":
        health = backend_health("vm") or {}
        if not health.get("ok"):
            raise RuntimeError(f"{VM} is not healthy through the load balancer; traffic stays put.")
    import_url_map(target)
    print(f"The load balancer now serves {DOMAIN} from {'the VM' if target == 'vm' else 'Cloud Run'}.")


def vm_status():
    print("serving:", serving())
    print("certificate:", g("certificate-manager", "certificates", "describe", CERT, "--location=global",
                            "--format=value(managed.state)"))
    print("backend health:", g("compute", "backend-services", "get-health", VM_BACKEND, "--global",
                               "--format=value(status.healthStatus[0].healthState)"))
    for backend in ("vm", "run"):
        print(f"{backend} /health:", backend_health(backend))
    print("image:", vm_metadata().get("coder-sync-image"))


def dns(point):
    """Point sync.openagents.com at the load balancer or back at the Cloud Run domain mapping (Cloudflare)."""
    token = os.environ.get("CLOUDFLARE_API_TOKEN")
    if not token:
        raise RuntimeError("Set CLOUDFLARE_API_TOKEN (scripts read ~/work/.secrets/cloudflare-openagents.env).")

    def cf(method, path, body=None):
        request = urllib.request.Request(f"https://api.cloudflare.com/client/v4{path}", method=method,
                                         data=json.dumps(body).encode() if body else None,
                                         headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"})
        with urllib.request.urlopen(request, timeout=30) as response:
            return json.load(response)["result"]

    zone = cf("GET", "/zones?name=openagents.com")[0]["id"]

    def put(name, kind, content):
        found = cf("GET", f"/zones/{zone}/dns_records?name={name}")
        body = {"type": kind, "name": name, "content": content, "ttl": 60, "proxied": False}
        for record in found[1:]:
            cf("DELETE", f"/zones/{zone}/dns_records/{record['id']}")
        if found:
            cf("PUT", f"/zones/{zone}/dns_records/{found[0]['id']}", body)
        else:
            cf("POST", f"/zones/{zone}/dns_records", body)
        print(f"{name} {kind} {content}")

    if point == "cert":
        name, data = g("certificate-manager", "dns-authorizations", "describe", DNS_AUTH, "--location=global",
                       "--format=value(dnsResourceRecord.name,dnsResourceRecord.data)").split()
        put(name.rstrip("."), "CNAME", data.rstrip("."))
    elif point == "lb":
        put(DOMAIN, "A", lb_address())
    else:
        put(DOMAIN, "CNAME", "ghs.googlehosted.com")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["provision", "build", "deploy", "domain", "lifecycle",
                                            "vm-provision", "vm-deploy", "vm-rollback", "vm-status", "cutover", "dns"])
    parser.add_argument("target", nargs="?", help="cutover: vm or run; dns: cert, lb, or run")
    parser.add_argument("--staging", action="store_true")
    parser.add_argument("--commit", help="vm-deploy and vm-provision: the image commit (default: HEAD, or what Cloud Run serves)")
    args = parser.parse_args()
    global ARGS
    ARGS = args
    if args.command in ("cutover", "dns") and args.target not in {"cutover": ("vm", "run"), "dns": ("cert", "lb", "run")}[args.command]:
        parser.error("cutover takes vm or run; dns takes cert, lb, or run")
    if args.command == "vm-provision": vm_provision()
    elif args.command == "vm-deploy": vm_deploy()
    elif args.command == "vm-rollback": vm_rollback()
    elif args.command == "vm-status": vm_status()
    elif args.command == "cutover": cutover(args.target)
    elif args.command == "dns": dns(args.target)
    elif args.command == "lifecycle":
        lifecycle()
    elif args.command == "provision": provision()
    elif args.command == "build": build()
    elif args.command == "deploy": deploy(args.staging)
    else:
        print(g("beta", "run", "domain-mappings", "create", "--service=coder-sync", "--domain=sync.openagents.com", f"--region={REGION}"))


if __name__ == "__main__":
    try: main()
    except RuntimeError as error:
        print(f"Deployment stopped: {error}", file=sys.stderr)
        sys.exit(1)
    except Exception as error:
        # SQL and HTTP exceptions can contain response bodies; keep credentials out.
        print(f"Deployment stopped ({type(error).__name__}). Check the preceding command result.", file=sys.stderr)
        sys.exit(1)
