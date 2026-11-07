# Firm reason: 2 (independent delayed-transfer and GPU-frame fixture for CDP accounting)
import json
import pathlib
import socket
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[3]
with tempfile.TemporaryDirectory(prefix="first-image-oracle-") as tmp:
    directory = pathlib.Path(tmp)
    (directory / "big.bin").write_bytes(b"x" * 524288)
    (directory / "late.bin").write_bytes(b"y" * 16384)
    (directory / "index.html").write_text('''<!doctype html><canvas></canvas><script type="module">
      const adapter = await navigator.gpu.requestAdapter();
      const device = await adapter.requestDevice();
      const canvas = document.querySelector('canvas');
      const ctx = canvas.getContext('webgpu');
      ctx.configure({device, format:navigator.gpu.getPreferredCanvasFormat()});
      globalThis.coaxial = {sceneReady:false};
      fetch('/big.bin').then(r=>r.arrayBuffer());
      setTimeout(() => {
        const e = device.createCommandEncoder();
        const p = e.beginRenderPass({colorAttachments:[{view:ctx.getCurrentTexture().createView(),clearValue:[1,0,0,1],loadOp:'clear',storeOp:'store'}]});
        p.end(); device.queue.submit([e.finish()]);
        coaxial.sceneReady = true;
      }, 200);
      setTimeout(()=>fetch('/late.bin').then(r=>r.arrayBuffer()),700);
    </script>''')
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        port = s.getsockname()[1]
    server = subprocess.Popen([str(ROOT / "coax/web/build/serve"), tmp, "--static", "--host-dir", tmp,
                               "--listen", f"127.0.0.1:{port}", "--throttle-mbps", "2", "--latency-ms", "30"],
                              stdout=subprocess.DEVNULL)
    try:
        time.sleep(.1)
        result = subprocess.run(["coax/web/tool.sh", "first-image", f"http://127.0.0.1:{port}/", "30000"],
                                cwd=ROOT, text=True, capture_output=True, timeout=90)
        assert result.returncode == 0, result.stdout + result.stderr
        report = json.loads(result.stdout)
        assert 524288 <= report["encodedBytesTotal"] <= 560000, report
        assert 1000 < report["bodyBytesBeforeFrame"] < 200000, report
        assert report["encodedBytesBeforeFrame"] < report["encodedBytesTotal"] - 16384, report
        assert 200 <= report["firstFrameMs"] < report["networkQuietMs"] - 500, report
        assert not report["overflow"] and report["activeRequests"] == 0 and report["failedRequests"] == 0, report
        print("PASS in-flight first-frame accounting and later-transfer exclusion", report)
    finally:
        server.terminate()
        server.wait(timeout=5)
