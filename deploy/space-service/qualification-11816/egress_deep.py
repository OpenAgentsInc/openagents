import json, socket, ssl
R = []
def tls(label, ip, sni, port=443):
    try:
        raw = socket.create_connection((ip, port), timeout=3)
        ctx = ssl.create_default_context()
        with ctx.wrap_socket(raw, server_hostname=sni) as t:
            cert = t.getpeercert()
            subj = dict(x[0] for x in cert.get('subject', ())).get('commonName')
            t.sendall(f'HEAD / HTTP/1.1\r\nHost: {sni}\r\nConnection: close\r\n\r\n'.encode())
            first = t.recv(64).split(b'\r\n')[0].decode(errors='replace')
        R.append({'probe': label, 'target': f'{ip}:{port}', 'sni': sni, 'tls_established': True, 'peer_cn': subj, 'http_first_line': first, 'denied': False})
    except Exception as e:
        R.append({'probe': label, 'target': f'{ip}:{port}', 'sni': sni, 'tls_established': False, 'denied': True, 'error': f'{type(e).__name__}: {str(e)[:120]}'})
def plain(label, ip, host, port=80):
    try:
        s = socket.create_connection((ip, port), timeout=3); s.settimeout(3)
        s.sendall(f'GET / HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n'.encode()); data = s.recv(64); s.close()
        R.append({'probe': label, 'target': f'{ip}:{port}', 'host': host, 'reply_first_line': data.split(b'\r\n')[0].decode(errors='replace'), 'denied': not data})
    except Exception as e:
        R.append({'probe': label, 'target': f'{ip}:{port}', 'host': host, 'denied': True, 'error': f'{type(e).__name__}: {str(e)[:120]}'})
tls('public_ip_tls', '1.1.1.1', 'one.one.one.one')
tls('public_ip_tls', '140.82.112.3', 'github.com')
tls('private_google_access_tls', '199.36.153.8', 'storage.googleapis.com')
tls('private_google_access_tls', '199.36.153.4', 'secretmanager.googleapis.com')
plain('public_ip_http', '142.250.190.78', 'www.google.com')
plain('public_ip_http', '1.1.1.1', 'one.one.one.one')
print(json.dumps(R))
