import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import threading
import unittest
import urllib.error
import urllib.request

from serial_pylon import recording_engine


class SerialPylonRecordingTests(unittest.TestCase):
    def test_preserves_final_bytes_and_records_upstream_errors(self):
        requests = []
        final = "\n\n\\boxed{2}\n"

        class Model(BaseHTTPRequestHandler):
            def log_message(self, *args):
                pass

            def do_POST(self):
                request = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
                requests.append(request)
                response = json.dumps({"choices": [{"message": {"content": final}}]}).encode()
                self.send_response(200 if request["max_tokens"] > 1 else 503)
                self.send_header("Content-Length", str(len(response)))
                self.end_headers()
                self.wfile.write(response)

        backend = ThreadingHTTPServer(("127.0.0.1", 0), Model)
        thread = threading.Thread(target=backend.serve_forever, daemon=True)
        thread.start()
        try:
            with recording_engine(f"http://127.0.0.1:{backend.server_port}") as (api, exchanges, state):
                for maximum in [2048, 1]:
                    payload = {"messages": [{"role": "assistant", "content": final}], "max_tokens": maximum}
                    request = urllib.request.Request(api + "/v1/chat/completions", data=json.dumps(payload).encode())
                    try:
                        response = urllib.request.urlopen(request)
                    except urllib.error.HTTPError as error:
                        response = error
                    with response:
                        self.assertEqual(json.load(response)["choices"][0]["message"]["content"], final)
                    self.assertEqual(exchanges[-1]["request"], payload)
                self.assertEqual([row["status"] for row in exchanges], [200, 503])
                self.assertEqual(state, {"active": 0, "maximum_active": 1})
                self.assertEqual(requests[0]["messages"][0]["content"], final)
        finally:
            backend.shutdown()
            backend.server_close()
            thread.join()


if __name__ == "__main__":
    unittest.main()
