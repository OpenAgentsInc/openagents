#!/usr/bin/env python3
"""Meter every inference request before forwarding through a private Unix socket."""
import argparse
import ctypes
import hashlib
import http.client
import http.server
import json
import os
import re
from pathlib import Path
import socketserver
import ssl
import threading
import time
import urllib.parse
import uuid

MAX_BODY = 16 * 1024 * 1024
MAX_EVENT = 4 * 1024 * 1024
MAX_RESPONSE = 64 * 1024 * 1024
REQUEST_HEADERS = {'content-type', 'anthropic-version', 'anthropic-beta', 'user-agent', 'accept', 'x-app'}
RESPONSE_HEADERS = {'content-type', 'request-id', 'anthropic-ratelimit-requests-remaining', 'retry-after'}
TOKEN_FIELDS = ('input_tokens', 'output_tokens', 'cache_creation_input_tokens', 'cache_read_input_tokens')
RATE_FIELDS = ('input', 'output', 'cache_write_5m', 'cache_write_1h', 'cache_read')


def admitted(method, target, body, models):
    """Admit only inference and token-count requests for the frozen models."""
    parsed = urllib.parse.urlsplit(target)
    if parsed.scheme or parsed.netloc or parsed.fragment or parsed.query not in ('', 'beta=true'):
        raise ValueError('Only a provider-relative path is allowed')
    if method != 'POST' or parsed.path not in ('/v1/messages', '/v1/messages/count_tokens'):
        raise ValueError('The provider path is not admitted')
    value = json.loads(body)
    if not isinstance(value, dict) or value.get('model') not in models:
        raise ValueError('The requested model is not admitted')
    if value.get('mcp_servers') or value.get('container'):
        raise ValueError('External provider tools are not admitted')
    for tool in value.get('tools',[]):
        if not isinstance(tool,dict) or tool.get('type','custom') not in ('custom','tool_search_tool_regex_20251119','tool_search_tool_bm25_20251119'):
            raise ValueError('External provider tools are not admitted')
    if value.get('speed') not in (None,'standard') or value.get('service_tier') not in (None,'auto','standard') or value.get('inference_geo') not in (None,'global'):
        raise ValueError('Only standard global inference is admitted')
    return target, value['model']


class Usage:
    """Read usage fields without retaining provider text or tool content."""
    def __init__(self, content_type):
        self.sse = 'text/event-stream' in content_type
        self.buffer = b''
        self.tokens = {}
        self.server_tool_use = {}
        self.service_tier = None
        self.served_model = None
        self.complete = False
        self.error = None
        self.size = 0

    def merge(self, value):
        if not isinstance(value, dict):
            raise ValueError('Invalid provider usage')
        if 'service_tier' in value:
            self.service_tier=value['service_tier']
        if 'server_tool_use' in value:
            uses=value['server_tool_use']
            if not isinstance(uses,dict) or any(type(n) is not int or n<0 for n in uses.values()):
                raise ValueError('Invalid server-tool usage')
            for key,count in uses.items():self.server_tool_use[key]=max(self.server_tool_use.get(key,0),count)
        for key in TOKEN_FIELDS:
            if key in value:
                count = value[key]
                if type(count) is not int or not 0 <= count <= 100_000_000:
                    raise ValueError('Invalid token count')
                # Streaming fields are cumulative, not increments.
                self.tokens[key] = max(self.tokens.get(key, 0), count)
        if 'cache_creation' in value:
            creation = value['cache_creation']
            if not isinstance(creation, dict):
                raise ValueError('Invalid cache usage')
            for source, key in [('ephemeral_5m_input_tokens', 'cache_write_5m_tokens'), ('ephemeral_1h_input_tokens', 'cache_write_1h_tokens')]:
                if source in creation:
                    count = creation[source]
                    if type(count) is not int or not 0 <= count <= 100_000_000:
                        raise ValueError('Invalid cache token count')
                    self.tokens[key] = max(self.tokens.get(key, 0), count)

    def event(self, item):
        if not isinstance(item, dict):
            raise ValueError('Invalid provider event')
        kind = item.get('type')
        if kind == 'message_start':
            message = item.get('message', {})
            self.served_model = message.get('model')
            self.merge(message.get('usage', {}))
        elif kind == 'message_delta':
            self.merge(item.get('usage', {}))
        elif kind == 'message_stop':
            self.complete = True
        elif kind == 'message':
            self.served_model = item.get('model')
            self.merge(item.get('usage', {}))
            self.complete = True
        elif kind == 'error':
            self.error = 'provider_error'

    def feed(self, data):
        self.size += len(data)
        if self.size > MAX_RESPONSE:
            raise ValueError('The provider response exceeded its bound')
        self.buffer += data
        if self.sse:
            while b'\n' in self.buffer:
                line, self.buffer = self.buffer.split(b'\n', 1)
                if len(line) > MAX_EVENT:
                    raise ValueError('The provider event exceeded its bound')
                if line.startswith(b'data:'):
                    raw = line[5:].strip()
                    if raw and raw != b'[DONE]':
                        self.event(json.loads(raw))
        if len(self.buffer) > MAX_EVENT:
            raise ValueError('The provider event exceeded its bound')

    def finish(self):
        if not self.sse and self.buffer:
            self.event(json.loads(self.buffer))
            self.buffer = b''
        # A missing message_stop, incomplete line, or absent usage remains unknown.
        return self.complete and not self.error and not self.buffer.strip() and all(k in self.tokens for k in ('input_tokens', 'output_tokens'))


def priced(tokens, rates):
    """Return reported-token cost, conservatively pricing unknown cache lifetimes."""
    total = tokens.get('cache_creation_input_tokens', 0)
    short = tokens.get('cache_write_5m_tokens', 0)
    long = tokens.get('cache_write_1h_tokens', 0)
    if short + long > total:
        raise ValueError('Cache usage is inconsistent')
    unknown = total - short - long
    dollars = (tokens['input_tokens'] * rates['input'] + tokens['output_tokens'] * rates['output'] + tokens.get('cache_read_input_tokens', 0) * rates['cache_read'] + short * rates['cache_write_5m'] + long * rates['cache_write_1h'] + unknown * max(rates['cache_write_5m'], rates['cache_write_1h'])) / 1_000_000
    return dollars, 'upper_bound_unknown_cache_ttl' if unknown else 'reported_tokens'


class Meter:
    """Reserve admission budget and append durable start and finish receipts."""
    def __init__(self, config, records):
        self.config = config
        self.run_id = config['run_id']
        if not isinstance(self.run_id, str) or not 1 <= len(self.run_id) <= 128:
            raise ValueError('Invalid run identity')
        self.models = config['models']
        if not self.models:
            raise ValueError('No models configured')
        for spec in self.models.values():
            if set(spec['usd_per_million']) != set(RATE_FIELDS) or any(type(v) not in (int, float) or not 0 <= v < 1_000_000 for v in spec['usd_per_million'].values()):
                raise ValueError('Explicit nonnegative token prices are required')
            if type(spec['max_requests']) is not int or not 1 <= spec['max_requests'] <= 1000:
                raise ValueError('Invalid per-model request cap')
        for key in ('max_requests', 'max_concurrent', 'max_output_tokens'):
            if type(config[key]) is not int or config[key] < 1:
                raise ValueError('Invalid request limit')
        if not 0 < config['admission_target_usd'] <= 100:
            raise ValueError('Invalid admission target')
        self.records = Path(records)
        # A new broker cannot silently reset a previous run's budget.
        self.handle = self.records.open('x', buffering=1)
        self.lock = threading.Lock()
        self.counts = {}
        self.active = {}
        self.spent = 0.0
        self.unknown_reservations = 0.0
        self.closed = False

    def write(self, row):
        self.handle.write(json.dumps(dict(schema='openagents.delegation.provider-call.v2', run_id=self.run_id, **row), sort_keys=True) + '\n')
        self.handle.flush()
        os.fsync(self.handle.fileno())

    def begin(self, model, path, body, request):
        with self.lock:
            generation = urllib.parse.urlsplit(path).path == '/v1/messages'
            maximum = request.get('max_tokens') if generation else 0
            if generation and (type(maximum) is not int or not 1 <= maximum <= self.config['max_output_tokens']):
                raise ValueError('The output-token limit is not admitted')
            if self.closed or sum(self.counts.values()) >= self.config['max_requests'] or self.counts.get(model, 0) >= self.models[model]['max_requests'] or len(self.active) >= self.config['max_concurrent']:
                raise ValueError('The run request limit has been reached')
            rates = self.models[model]['usd_per_million']
            # This is an admission reservation, not an attested tokenizer bound.
            input_reserve = len(body) * 2 + 16_384
            reserve = (input_reserve * max(rates['input'], rates['cache_write_5m'], rates['cache_write_1h']) + maximum * rates['output']) / 1_000_000 if generation else 0.0
            held = sum(self.active.values()) + self.unknown_reservations
            if self.spent + held + reserve > self.config['admission_target_usd']:
                raise ValueError('The run admission target has been reached')
            call_id = str(uuid.uuid4())
            self.counts[model] = self.counts.get(model, 0) + 1
            self.active[call_id] = reserve
            self.write(dict(phase='admitted', call_id=call_id, started_unix=time.time(), model=model, path=path, request_bytes=len(body), request_sha256=hashlib.sha256(body).hexdigest(), reservation_usd=reserve, reserved_input_tokens=input_reserve if generation else 0, requested_max_output_tokens=maximum))
            return call_id

    def end(self, call_id, row):
        with self.lock:
            reserve = self.active.pop(call_id)
            cost = row.get('cost_usd')
            if cost is None:
                self.unknown_reservations += reserve
                # Unknown usage blocks further inference instead of assuming zero.
                self.closed = True
            else:
                self.spent += cost
            self.write(dict(phase='finished', call_id=call_id, **row, accounted_usd=self.spent, unknown_reservations_usd=self.unknown_reservations))

    def refused(self, row):
        with self.lock:
            self.write(dict(phase='refused', **row))


class UnixServer(socketserver.ThreadingMixIn, socketserver.UnixStreamServer):
    daemon_threads = True


class Forward(http.server.BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.0'

    def setup(self):
        super().setup()
        self.connection.settimeout(310)

    def log_message(self, *_):
        pass

    def error(self, code, message):
        data = json.dumps({'type':'error','error':{'type':'invalid_request_error','message':message}}).encode()
        try:
            self.send_response(code)
            self.send_header('Content-Type','application/json')
            self.send_header('Content-Length',str(len(data)))
            self.end_headers()
            self.wfile.write(data)
        except (OSError, ValueError):
            pass

    def do_POST(self):
        began = time.monotonic()
        row = {'status':'incomplete', 'cost_usd':None, 'usage_status':'unknown', 'client_disconnected':False}
        connection = None
        call_id = None
        usage = None
        try:
            if self.headers.get('Transfer-Encoding'):
                raise ValueError('Chunked request bodies are not admitted')
            length = int(self.headers.get('Content-Length','-1'))
            if not 0 < length <= MAX_BODY:
                raise ValueError('Request size is outside the allowed bound')
            body = self.rfile.read(length)
            if len(body) != length:
                raise ValueError('The request body is incomplete')
            request = json.loads(body)
            requested = request.get('model') if isinstance(request, dict) else None
            if isinstance(requested, str) and re.fullmatch(r'claude-[a-z0-9-]{1,80}', requested):
                row['requested_model'] = requested
            row.update(request_bytes=length, requested_max_output_tokens=request.get('max_tokens') if isinstance(request,dict) and type(request.get('max_tokens')) is int else None)
            path, model = admitted(self.command, self.path, body, self.server.meter.models)
            call_id = self.server.meter.begin(model, path, body, json.loads(body))
            row.update(requested_model=model, path=path)
            headers = {k:v for k,v in self.headers.items() if k.lower() in REQUEST_HEADERS}
            headers.update(Authorization='Bearer ' + self.server.token, **{'Content-Length':str(length), 'Connection':'close', 'Accept-Encoding':'identity'})
            connection = self.server.connection_factory()
            connection.request('POST',path,body,headers)
            response = connection.getresponse()
            row.update(http_status=response.status, first_response_s=time.monotonic()-began)
            usage = Usage(response.getheader('content-type', ''))
            if response.getheader('content-encoding', 'identity') not in ('', 'identity'):
                raise ValueError('The provider response has unsupported compression')
            try:
                self.send_response(response.status)
                for name,value in response.getheaders():
                    if name.lower() in RESPONSE_HEADERS:
                        self.send_header(name,value)
                self.send_header('Connection','close')
                self.end_headers()
            except OSError:
                row['client_disconnected'] = True
            while chunk := response.read1(65536):
                usage.feed(chunk)
                if not row['client_disconnected']:
                    try:
                        self.wfile.write(chunk)
                        self.wfile.flush()
                    except OSError:
                        # Finish reading usage even after the executor disconnects.
                        row['client_disconnected'] = True
            row.update(status='complete', response_bytes=usage.size, served_model=usage.served_model, usage=usage.tokens)
            if urllib.parse.urlsplit(path).path.endswith('/count_tokens'):
                # The provider's token-count endpoint is not a generation call.
                value = json.loads(usage.buffer)
                if response.status == 200 and type(value.get('input_tokens')) is int:
                    row.update(usage_status='token_count_only', counted_input_tokens=value['input_tokens'], cost_usd=0.0, cost_basis='non_generation_endpoint')
            elif usage.finish() and response.status == 200 and usage.served_model == model and not any(usage.server_tool_use.values()) and usage.service_tier in (None,'standard'):
                cost, basis = priced(usage.tokens, self.server.meter.models[model]['usd_per_million'])
                row.update(usage_status='reported', cost_usd=cost, cost_basis=basis)
        except (ValueError, json.JSONDecodeError) as error:
            row.update(status='refused' if call_id is None else 'invalid_response', error_type=type(error).__name__)
            if call_id is None:
                if type(error) is ValueError:
                    row['refusal_reason']=str(error)
                self.error(403,'The benchmark broker refused this request')
        except Exception as error:
            # Exception messages can contain provider response text or headers.
            row.update(status='transport_error',error_type=type(error).__name__)
            if 'http_status' not in row:
                self.error(502,'The benchmark provider connection failed')
        finally:
            if connection:
                connection.close()
            row['wall_s'] = time.monotonic()-began
            if usage is not None:
                row.update(response_bytes=usage.size, usage=usage.tokens, served_model=usage.served_model, server_tool_use=usage.server_tool_use, service_tier=usage.service_tier)
            if call_id:
                self.server.meter.end(call_id, row)
            else:
                self.server.meter.refused(row)

    def do_GET(self):
        self.server.meter.refused({'status':'refused','reason':'unsupported_method'})
        self.error(403,'The benchmark broker admits inference POST requests only')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--socket',required=True)
    parser.add_argument('--credential-file',required=True)
    parser.add_argument('--records',required=True)
    parser.add_argument('--config',required=True)
    args = parser.parse_args()
    os.umask(0o077)
    # Refuse to hold the provider credential in a dumpable process.
    if ctypes.CDLL(None).prctl(4, 0, 0, 0, 0) != 0:
        raise SystemExit('Could not disable provider process dumps')
    meter = Meter(json.loads(Path(args.config).read_text()), args.records)
    credential = Path(args.credential_file)
    if credential.is_symlink():
        raise SystemExit('The provider transfer file must not be a symlink')
    credential.chmod(0o600)
    token = credential.read_text().strip()
    if not token or any(c.isspace() for c in token):
        raise SystemExit('The provider credential is malformed')
    Path(args.credential_file).unlink()
    path = Path(args.socket)
    path.parent.mkdir(parents=True,exist_ok=True)
    if path.exists() or path.is_symlink():
        raise SystemExit('The broker socket already exists')
    with UnixServer(str(path),Forward) as server:
        server.token = token
        server.meter = meter
        server.connection_factory = lambda: http.client.HTTPSConnection('api.anthropic.com',443,timeout=300,context=ssl.create_default_context())
        server.serve_forever()


if __name__ == '__main__':
    main()
