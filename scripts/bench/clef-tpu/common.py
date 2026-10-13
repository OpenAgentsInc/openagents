"""Shared helpers: environment capture, timing, JSON output."""
import json, os, platform, socket, subprocess, time, functools
import jax

PRICES = {  # $/chip-hour, Cloud Billing catalog 2026-10-12 (docs/inference/clef-tpu-survey.md)
    # (spot, on-demand) by chip family; regional spot overrides below
    "v6e": (0.584, 2.70), "v5e": (0.359, 1.20), "v5p": (0.93, 4.20),
}
SPOT_REGION = {
    ("v6e", "us-east4"): 0.243, ("v6e", "us-south1"): 0.66, ("v6e", "us-east5"): 1.62,
    ("v5e", "us-east5"): 0.57, ("v5e", "us-south1"): 0.79, ("v5e", "us-east4"): 0.49,
    ("v5p", "us-east4"): 0.36,
}

def chip_family():
    kind = jax.devices()[0].device_kind.lower()
    if "v6" in kind: return "v6e"
    if "v5p" in kind or kind.strip() == "tpu v5": return "v5p"
    if "v5" in kind: return "v5e"
    return kind

def env(use_jax=True):
    def sh(c):
        try: return subprocess.check_output(c, shell=True, text=True, timeout=10).strip()
        except Exception as e: return f"err:{e}"
    md = lambda p: sh(f"curl -s -H 'Metadata-Flavor: Google' http://metadata.google.internal/computeMetadata/v1/{p}")
    try:
        import jaxlib; jaxlib_v = jaxlib.__version__
    except Exception: jaxlib_v = None
    try:
        import libtpu; libtpu_v = getattr(libtpu, "__version__", None)
    except Exception: libtpu_v = None
    if libtpu_v is None:
        libtpu_v = sh("pip show libtpu 2>/dev/null | grep Version || uv pip show libtpu 2>/dev/null | grep Version")
    zone = md("instance/zone").split("/")[-1]
    acc = md("instance/attributes/accelerator-type")
    if use_jax:
        fam = chip_family()
    else:
        fam = {"v6e": "v6e", "v5litepod": "v5e", "v5p": "v5p"}.get(acc.split("-")[0], acc)
    region = zone.rsplit("-", 1)[0] if zone else ""
    spot, od = PRICES.get(fam, (None, None))
    spot = SPOT_REGION.get((fam, region), spot)
    try:  # exact catalog prices per region (prices.json, Cloud Billing catalog 2026-10-13)
        pj = json.load(open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "prices.json")))
        spot = pj[fam][region].get("spot", spot); od = pj[fam][region].get("ondemand", od)
    except Exception:
        pass
    return {
        "host": socket.gethostname(), "zone": zone,
        "accelerator_type": md("instance/attributes/accelerator-type"),
        "device_kind": jax.devices()[0].device_kind if use_jax else None,
        "n_devices": jax.device_count() if use_jax else None,
        "chip": fam, "jax": jax.__version__, "jaxlib": jaxlib_v, "libtpu": libtpu_v,
        "python": platform.python_version(),
        "price_spot_chip_h": spot, "price_ondemand_chip_h": od,
        "ts": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }

def bench(fn, *args, min_time=0.3, warmup=2):
    """Median-of-3 seconds per call of an already-jitted fn (async dispatch, block at end)."""
    out = None
    for _ in range(warmup):
        out = fn(*args); jax.block_until_ready(out)
    t0 = time.perf_counter(); out = fn(*args); jax.block_until_ready(out)
    one = max(time.perf_counter() - t0, 1e-6)
    n = max(1, min(200, int(min_time / one)))
    res = []
    for _ in range(3):
        t0 = time.perf_counter()
        for _ in range(n): out = fn(*args)
        jax.block_until_ready(out)
        res.append((time.perf_counter() - t0) / n)
    res.sort()
    return res[1], out

def hbm():
    try:
        s = jax.devices()[0].memory_stats() or {}
        return {k: s.get(k) for k in ("bytes_in_use", "peak_bytes_in_use", "bytes_limit")}
    except Exception:
        return {}

def write(name, payload):
    d = os.environ.get("OUT", os.path.expanduser("~/out"))
    os.makedirs(d, exist_ok=True)
    cg = os.environ.get("CLEF_CHIPS")
    if cg: name = name.replace(".json", f"-chips{cg.replace(',', '')}.json")
    p = os.path.join(d, name)
    with open(p, "w") as f: json.dump(payload, f, indent=1, default=str)
    print("wrote", p)
