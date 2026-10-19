import json, socket, struct, http.client, sys
G = sys.argv[1] if len(sys.argv) > 1 else 'all'
R = []
def tcp(label, host, port, fam=socket.AF_INET):
    try:
        s = socket.socket(fam, socket.SOCK_STREAM); s.settimeout(1.5); s.connect((host, port)); s.close()
        R.append({'probe': label, 'target': f'{host}:{port}', 'denied': False})
    except OSError as e:
        R.append({'probe': label, 'target': f'{host}:{port}', 'denied': True, 'error': type(e).__name__})
def dns_udp(label, server):
    q = struct.pack('>HHHHHH', 0x1186, 0x0100, 1, 0, 0, 0) + b''.join(bytes([len(p)]) + p.encode() for p in 'example.com'.split('.')) + b'\0' + struct.pack('>HH', 1, 1)
    try:
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM); s.settimeout(1.5); s.sendto(q, (server, 53)); data = s.recv(512); s.close()
        R.append({'probe': label, 'target': f'{server}:53/udp', 'denied': False, 'answer_bytes': len(data)})
    except OSError as e:
        R.append({'probe': label, 'target': f'{server}:53/udp', 'denied': True, 'error': type(e).__name__})
def resolve(label, name):
    try:
        addrs = sorted({a[4][0] for a in socket.getaddrinfo(name, 80)})
        R.append({'probe': label, 'target': name, 'resolved': addrs, 'denied': False})
        return addrs
    except OSError as e:
        R.append({'probe': label, 'target': name, 'denied': True, 'error': type(e).__name__}); return []
def http_get(label, host, path, headers, port=80):
    try:
        c = http.client.HTTPConnection(host, port, timeout=1.5); c.request('GET', path, headers=headers); r = c.getresponse()
        R.append({'probe': label, 'target': f'http://{host}{path}', 'denied': r.status in (401, 403), 'status': r.status, 'location': r.getheader('Location')}); c.close()
    except OSError as e:
        R.append({'probe': label, 'target': f'http://{host}{path}', 'denied': True, 'error': type(e).__name__})
# Group 1: metadata by IP and name, a redirect toward metadata, DNS bypass, public DNS.
if G in ('1', 'all'):
    http_get('metadata_ipv4', '169.254.169.254', '/computeMetadata/v1/instance/id', {'Metadata-Flavor': 'Google'})
    for a in resolve('metadata_dns', 'metadata.google.internal'): tcp('metadata_dns_connect', a, 80)
    http_get('redirect_to_metadata', 'httpbin.org', '/redirect-to?url=http%3A%2F%2F169.254.169.254%2FcomputeMetadata%2Fv1%2F', {})
    dns_udp('dns_bypass', '8.8.8.8'); dns_udp('dns_bypass', '1.1.1.1')
    for a in resolve('public_dns', 'example.com'): tcp('public_dns_connect', a, 443)
# Group 2: management and private service addresses, public IPs without DNS, IPv6.
if G in ('2', 'all'):
    for host, port in [('10.219.14.10', 6443), ('10.219.14.10', 8443), ('10.219.14.10', 8444), ('10.219.14.10', 9810), ('10.219.14.10', 10250),
                       ('10.219.14.11', 9800), ('10.219.14.11', 10250), ('10.220.1.1', 9800), ('10.220.0.1', 443), ('10.43.0.1', 443),
                       ('10.219.16.3', 5432), ('199.36.153.8', 443), ('199.36.153.4', 443)]:
        tcp('management', host, port)
    for host, port in [('1.1.1.1', 443), ('8.8.8.8', 443), ('142.250.190.78', 443), ('140.82.112.3', 443)]: tcp('public_ip', host, port)
    for host in ['2001:4860:4860::8888', '2606:4700:4700::1111', 'fd20::1']: tcp('ipv6', host, 443, socket.AF_INET6)
print(json.dumps(R))
raise SystemExit(0 if all(r['denied'] for r in R) else 1)
