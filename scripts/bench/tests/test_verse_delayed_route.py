"""Check byte order, injected delay, and bounded forwarding on scratch TCP."""
import asyncio
import json
import pathlib
import signal
import sys
import tempfile
import time
import unittest


class DelayedRoute(unittest.IsolatedAsyncioTestCase):
    async def exercise(self, profile):
        async def echo(reader, writer):
            try:
                while chunk := await reader.read(65536):
                    writer.write(chunk)
                    await writer.drain()
            finally:
                writer.close()
                await writer.wait_closed()

        server = await asyncio.start_server(echo, "127.0.0.1", 0)
        port = server.sockets[0].getsockname()[1]
        proxy = None
        writer = None
        with tempfile.TemporaryDirectory(prefix="verse-route-test-") as directory:
            ready = pathlib.Path(directory) / "ready.json"
            receipt = pathlib.Path(directory) / "receipt.json"
            command = [sys.executable, str(pathlib.Path(__file__).parents[1] / "verse-delayed-route.py"),
                       "--destination-port", str(port), "--seconds", "10",
                       "--delay-ms", "40", "--jitter-ms", "20",
                       "--ready", str(ready), "--receipt", str(receipt)]
            if profile != "scheduled":
                command += ["--delay-profile", profile]
            try:
                proxy = await asyncio.create_subprocess_exec(*command)
                async with asyncio.timeout(5):
                    while not ready.exists():
                        self.assertIsNone(proxy.returncode)
                        await asyncio.sleep(0.01)
                    address = json.loads(ready.read_text())["address"]
                    reader, writer = await asyncio.open_connection("127.0.0.1", int(address.rsplit(":", 1)[1]))
                    payload = bytes(range(256)) * (8192 if profile in ("pipeline", "scheduled") else 1) + b"ordered-tail"
                    began = time.monotonic()
                    writer.write(payload)
                    await writer.drain()
                    first = await reader.readexactly(1)
                    first_seconds = time.monotonic() - began
                    self.assertGreaterEqual(first_seconds, 0.075)
                    received = first + await reader.readexactly(len(payload) - 1)
                    self.assertEqual(received, payload)
                    writer.close()
                    await writer.wait_closed()
                    proxy.send_signal(signal.SIGTERM)
                    await proxy.wait()
                self.assertEqual(proxy.returncode, 0)
                stats = json.loads(receipt.read_text())
                self.assertEqual(stats["delay_profile"], profile)
                self.assertLessEqual(stats["queued_chunk_peak"], 32 if profile == "scheduled" else 8)
                if profile in ("pipeline", "scheduled"):
                    self.assertGreater(stats["queued_chunk_peak"], 1)
                    self.assertGreater(stats["forwarded_chunks"], 8)
                else:
                    self.assertEqual(stats["queued_chunk_peak"], 0)
                print(json.dumps({"profile": profile, "bytes_checked": len(payload),
                                  "first_byte_seconds": first_seconds,
                                  "queued_chunk_peak": stats["queued_chunk_peak"]}))
            finally:
                if writer:
                    writer.close()
                if proxy and proxy.returncode is None:
                    proxy.kill()
                    await proxy.wait()
                server.close()
                await server.wait_closed()

    async def test_pipeline_keeps_large_payload_order_and_per_chunk_delay(self):
        await self.exercise("pipeline")

    async def test_default_profile_retains_scheduled_delay(self):
        await self.exercise("scheduled")

    async def test_explicit_serial_profile_retains_serial_delay(self):
        await self.exercise("serial")


if __name__ == "__main__":
    unittest.main()
