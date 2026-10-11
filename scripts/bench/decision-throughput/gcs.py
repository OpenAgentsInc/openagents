"""Google credentials and Cloud Storage for the decision throughput harness,
stdlib only.

A token comes from, in order: $GOOGLE_OAUTH_ACCESS_TOKEN, the metadata
server (Cloud Run, GCE), or `gcloud auth print-access-token` (honours
CLOUDSDK_CONFIG, e.g. the automation account's isolated config). Tokens are
cached for 45 minutes and never printed.
"""
import json, os, subprocess, tempfile, threading, time
import urllib.error, urllib.parse, urllib.request

_lock = threading.Lock()
_cached = {"token": None, "at": 0.0}


def token():
    with _lock:
        if _cached["token"] and time.time() - _cached["at"] < 2700:
            return _cached["token"]
        t = os.environ.get("GOOGLE_OAUTH_ACCESS_TOKEN")
        if not t:
            try:
                req = urllib.request.Request(
                    "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token",
                    headers={"Metadata-Flavor": "Google"})
                with urllib.request.urlopen(req, timeout=2) as r:
                    t = json.loads(r.read())["access_token"]
            except Exception:
                t = None
        if not t:
            t = subprocess.run(["gcloud", "auth", "print-access-token"], capture_output=True, text=True,
                               check=True).stdout.strip()
        _cached.update(token=t, at=time.time())
        return t


def _split(uri):
    assert uri.startswith("gs://"), uri
    bucket, _, name = uri[5:].partition("/")
    return bucket, name


def get(uri):
    """(bytes, generation) or (None, 0) when the object does not exist."""
    bucket, name = _split(uri)
    url = (f"https://storage.googleapis.com/storage/v1/b/{bucket}/o/{urllib.parse.quote(name, safe='')}"
           "?alt=media")
    req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token()}"})
    try:
        with urllib.request.urlopen(req, timeout=60) as r:
            return r.read(), int(r.headers.get("x-goog-generation", "0"))
    except urllib.error.HTTPError as e:
        if e.code == 404:
            return None, 0
        raise


def put(uri, data, if_generation=None, content_type="application/x-ndjson"):
    bucket, name = _split(uri)
    q = {"uploadType": "media", "name": name}
    if if_generation is not None:
        q["ifGenerationMatch"] = str(if_generation)
    url = f"https://storage.googleapis.com/upload/storage/v1/b/{bucket}/o?" + urllib.parse.urlencode(q)
    req = urllib.request.Request(url, data=data, method="POST", headers={
        "Authorization": f"Bearer {token()}", "Content-Type": content_type})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())


def list_names(prefix_uri):
    bucket, prefix = _split(prefix_uri)
    names, page = [], None
    while True:
        q = {"prefix": prefix, "fields": "items(name),nextPageToken"}
        if page:
            q["pageToken"] = page
        url = f"https://storage.googleapis.com/storage/v1/b/{bucket}/o?" + urllib.parse.urlencode(q)
        req = urllib.request.Request(url, headers={"Authorization": f"Bearer {token()}"})
        with urllib.request.urlopen(req, timeout=60) as r:
            body = json.loads(r.read())
        names += [f"gs://{bucket}/{i['name']}" for i in body.get("items", [])]
        page = body.get("nextPageToken")
        if not page:
            return names


def append_jsonl(prefix_uri, line, day=None):
    """Append one JSON line to <prefix>/YYYY-MM-DD.jsonl (UTC), with a
    generation precondition so concurrent writers never lose a line."""
    day = day or time.strftime("%Y-%m-%d", time.gmtime())
    uri = prefix_uri.rstrip("/") + f"/{day}.jsonl"
    for attempt in range(8):
        old, gen = get(uri)
        try:
            put(uri, (old or b"") + (json.dumps(line) + "\n").encode(), if_generation=gen)
            return uri
        except urllib.error.HTTPError as e:
            if e.code != 412:
                raise
            time.sleep(0.2 * (attempt + 1))
    raise RuntimeError(f"could not append to {uri}")


def local_corpus(path):
    """A gs:// corpus directory is downloaded to a temporary directory."""
    if not path.startswith("gs://"):
        return path
    d = tempfile.mkdtemp(prefix="decision-corpus-")
    for name in ("dataset.json", "router.jsonl"):
        data, _ = get(path.rstrip("/") + "/" + name)
        if data is not None:
            open(os.path.join(d, name), "wb").write(data)
    return d
