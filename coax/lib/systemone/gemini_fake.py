# Firm reason: 2 (an independent oracle: a stand-in for Google's Decisions API that holds the exact request bytes our Coax client must send)
# A local stand-in for Google's Decisions API, for gemini-smoke.sh.
#   POST /ticket   the body must be the ticket request byte for byte; answers it
#   POST /image    the body must hold one low-resolution JPEG block whose
#                  bytes are GD_FAKE_IMAGE's; answers three questions
#   POST /invalid, /safety, /content_blocked, /not_found, /bad_key, /quota,
#        /server   Google's refusal bodies, with their statuses
#   POST /malformed  200 with an answer missing its questions
# Every request must carry x-goog-api-key equal to GD_FAKE_KEY and no
# Authorization header, and while curl runs no process's arguments may hold
# the key. Prints its port, then serves until killed.
import base64, http.server, json, os, subprocess, sys

KEY = os.environ["GD_FAKE_KEY"]
IMAGE = open(os.environ["GD_FAKE_IMAGE"], "rb").read()
TICKET = (b'{"model":"gemini-decision-1","input":[{"type":"text","text":"I was charged twice for my subscription. '
          b'Please refund the duplicate charge."}],"questions":{"route":{"type":"choice","instructions":"Which team should '
          b'handle this request?","criteria":{"billing":"Billing, payments, and refunds","technical_support":"Technical '
          b'troubleshooting"}},"refund":{"type":"noul","instructions":"Is the customer asking for a refund?"},"urgency":'
          b'{"type":"score","instructions":"How urgent is the request?","criteria":["Routine request","Needs attention '
          b'today","Immediate emergency"]}}}')
TICKET_ANSWER = {"answers": {
    "refund": {"confidence": 0.99, "noul": 0.99, "type": "noul"},
    "route": {"choice": "billing", "confidence": 0.99, "probabilities": {"billing": 0.99, "technical_support": 0.01}, "type": "choice"},
    "urgency": {"confidence": 0.62, "legend": {"0": "Routine request", "1": "Needs attention today", "2": "Immediate emergency"},
                "probabilities": {"0": 0.62, "1": 0.37, "2": 0.01}, "score": 0.39, "type": "score"}},
    "usage": {"total_tokens": 171, "total_input_tokens": 171, "input_tokens_by_modality": [{"modality": "text", "tokens": 171}], "total_output_tokens": 0}}
IMAGE_ANSWER = {"answers": {
    "cold": {"confidence": 0.9, "noul": 0.9, "type": "noul"},
    "spell": {"choice": "ray_of_frost", "confidence": 0.97, "probabilities": {"fire_bolt": 0.02, "ray_of_frost": 0.97, "thunderwave": 0.01}, "type": "choice"},
    "range": {"confidence": 0.8, "legend": {"0": "Under 30 feet", "1": "30 to 90 feet", "2": "Over 90 feet"},
              "probabilities": {"0": 0.1, "1": 0.8, "2": 0.1}, "score": 1.0, "type": "score"}},
    "usage": {"total_tokens": 400, "total_input_tokens": 400, "input_tokens_by_modality": [{"modality": "text", "tokens": 142}, {"modality": "image", "tokens": 258}], "total_output_tokens": 0}}
REFUSALS = {
    "/invalid": (400, {"error": {"message": "* CreateDecisionRequest.questions[k].choice.criteria: must have between 2 and 255 options\n", "code": "invalid_request"}}),
    "/safety": (400, {"error": {"code": "safety", "message": "Request blocked due to safety violations (harmful content). Please modify your input and retry."}}),
    "/content_blocked": (400, {"error": {"code": "content_blocked", "message": "blocked"}}),
    "/not_found": (404, {"error": {"message": "Requested model not found.", "code": "not_found"}}),
    "/bad_key": (400, {"error": {"code": 400, "message": "API key not valid. Please pass a valid API key.", "status": "INVALID_ARGUMENT",
                                 "details": [{"@type": "type.googleapis.com/google.rpc.ErrorInfo", "reason": "API_KEY_INVALID"}]}}),
    "/quota": (429, {"error": {"code": 429, "message": "Resource has been exhausted (e.g. check quota).", "status": "RESOURCE_EXHAUSTED"}}),
    "/server": (500, {"error": {"code": 500, "message": "An internal error has occurred.", "status": "INTERNAL"}}),
    "/malformed": (200, {"answers": {}, "usage": {"total_tokens": 1, "total_input_tokens": 1, "total_output_tokens": 0}}),
}
problems = []


def leaked():
    return KEY in subprocess.run(["ps", "-axww", "-o", "args="], capture_output=True, text=True).stdout


class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def log_message(self, *a):
        pass

    def reply(self, status, value):
        data = json.dumps(value).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def do_POST(self):
        body = self.rfile.read(int(self.headers["Content-Length"]))
        if self.headers.get("x-goog-api-key") != KEY or self.headers.get("Authorization") is not None:
            return self.reply(400, {"error": {"code": 400, "message": "the key header is wrong", "status": "INVALID_ARGUMENT",
                                              "details": [{"reason": "API_KEY_INVALID"}]}})
        if leaked():
            return self.reply(500, {"error": {"code": 500, "message": "the key is in a process's arguments"}})
        if self.path == "/ticket":
            if body != TICKET:
                return self.reply(400, {"error": {"code": "invalid_request", "message": "not the golden ticket bytes: " + body.decode(errors="replace")}})
            return self.reply(200, TICKET_ANSWER)
        if self.path == "/image":
            d = json.loads(body)
            blocks = [x for x in d["input"] if x["type"] == "image"]
            ok = (len(blocks) == 1 and blocks[0]["mime_type"] == "image/jpeg" and blocks[0]["resolution"] == "low"
                  and base64.b64decode(blocks[0]["data"]) == IMAGE and sorted(d["questions"]) == ["cold", "range", "spell"])
            if not ok:
                return self.reply(400, {"error": {"code": "invalid_request", "message": "not the image request"}})
            return self.reply(200, IMAGE_ANSWER)
        if self.path in REFUSALS:
            status, value = REFUSALS[self.path]
            return self.reply(status, value)
        self.reply(404, {"error": {"code": "not_found", "message": "no such path"}})


server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
print(server.server_address[1], flush=True)
server.serve_forever()
