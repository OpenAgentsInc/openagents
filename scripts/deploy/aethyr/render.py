#!/usr/bin/env python3
"""Prepare immutable editor and isolated preview container contexts."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
from urllib.parse import urlsplit

p=argparse.ArgumentParser()
for name in ['release','editor','preview','host','sdk','dist','sdk-directory','assets-manifest','output']:p.add_argument('--'+name,required=True)
p.add_argument("--protocol-version",type=int,choices=[1,2,3,4,5,6,7,8],default=1)
p.add_argument("--profile",choices=["world","universe"],default="world")
a=p.parse_args()
assert re.fullmatch(r'[a-z0-9-]{1,63}',a.release)
assert re.fullmatch(r'[a-f0-9]{64}',a.sdk)
for url in [a.editor,a.preview,a.host]:
    parsed=urlsplit(url)
    assert parsed.scheme=='https' and parsed.netloc and not parsed.username and not parsed.password and not parsed.query and not parsed.fragment and parsed.path in ['', '/']
assert len({urlsplit(u).netloc for u in [a.editor,a.preview,a.host]})==3
sdk_path='/sdk/'+a.sdk
manifest={'schema':'aethyrforge.browser-release.v1','release':a.release,'protocol_version':a.protocol_version,'engine_abi':'aethyrforge.browser.v1','project_format':1,'editor_url':a.editor+'/','preview_url':a.preview+'/preview.html','host_origin':a.host,'sdk_url':a.editor+sdk_path+'/aethyrforge_host.js','asset_set':json.loads(Path(a.assets_manifest).read_text())['sha256'],'files':{}}
base='nginx:stable-alpine@sha256:47928bf67eb62a1863cb08d9ecb52d3e704304ba614eaee17884204677702951'
for role in ['editor','preview']:
    root=Path(a.output)/role
    (root/'site/assets').mkdir(parents=True,exist_ok=True)
    dist=Path(a.dist)
    names=(['universe.html'] if role=='editor' else ['universe-preview.html']) if a.profile=='universe' else (['index.html','universe.html'] if role=='editor' else ['preview.html','universe-preview.html','kernel.wasm'])
    if role=='editor' and a.profile=='world' and a.protocol_version>=7:names.append('player.html')
    for file in (dist/'assets').iterdir():
        if role=='preview' and file.name.startswith('editor-'):continue
        if a.profile=='universe':
            if not (file.suffix == '.css' or file.name.startswith('modulepreload-') or (file.name.startswith('universe-') and (file.name.startswith('universe-preview-') == (role=='preview')))):continue
        names.append(str(file.relative_to(dist)))
    if (dist/'favicon.svg').exists():names.append('favicon.svg')
    for name in names:
        assert not name.endswith('.map')
        shutil.copyfile(dist/name,root/'site'/name)
    if role=='editor':
        sdk=root/'site'/sdk_path.lstrip('/')
        sdk.mkdir(parents=True,exist_ok=True)
        for name in ['aethyrforge_host.js','aethyrforge_host_bg.wasm'] + (['kiln.js','kiln_bg.wasm'] if a.protocol_version >= 3 else []):shutil.copyfile(Path(a.sdk_directory)/name,sdk/name)
        if a.protocol_version >= 3:
            snippets=Path(a.sdk_directory)/'snippets'
            if snippets.exists():
                assert all(file.suffix=='.js' for file in snippets.rglob('*') if file.is_file())
                shutil.copytree(snippets,sdk/'snippets',dirs_exist_ok=True)
    (root/'site/NOTICE.txt').write_text('Aethyrforge engine: OpenAgents software, published in the OpenAgentsInc/openagents repository; third-party components are identified in THIRD_PARTY_LICENSES.txt.\nOpenAgents host protocol SDK: open source.\nRuntime scene content is licensed for this interactive demo; source asset files are not distributed.\nThird-party runtime notices: THIRD_PARTY_LICENSES.txt.\n')
    # Vite retains dependency license notices in its generated bundles. Include
    # the actual licenses for runtime dependencies beside those bundles too.
    # This script lives in scripts/deploy/aethyr; the web package is apps/aethyr.
    repo=Path(__file__).resolve().parents[3]/'apps'/'aethyr'
    notices=[(repo/'LICENSE.shadcn').read_text()]
    for package in ['react','react-dom','scheduler','effect','@effect/atom-react','class-variance-authority','clsx','tailwind-merge','radix-ui']:
        for license_name in ['LICENSE','LICENSE.md','LICENSE.txt']:
            candidate=repo/'node_modules'/package/license_name
            if candidate.is_file():notices.append(package+'\n'+candidate.read_text());break
    notices.append('OpenAgents universe renderer (ported from apps/verse-selector): Apache-2.0\n'+(repo/'packages/universe/LICENSE').read_text())
    notices.append('OpenAgents Verse camera, controller, and character ports: Apache-2.0\n'
                   +(repo/'third_party/openagents-camera/LICENSE').read_text()
                   +'\n\n'+(repo/'third_party/openagents-camera/NOTICE').read_text()
                   +'\n\n'+(repo/'third_party/openagents-character/README.md').read_text())
    for name in ['base-license.txt','outfits-license.txt','animations-license.txt']:
        notices.append('Quaternius Universal character sources — '+name+'\n'
                       +(repo/'third_party/openagents-character'/name).read_text())
    notices.append('OpenAgents Grove destruction and Druid ability port\n'
                   +(repo/'third_party/openagents-gameplay/NOTICE').read_text())
    (root/'site/THIRD_PARTY_LICENSES.txt').write_text('\n\n'.join(notices))
    csp=(f"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; worker-src 'self'; frame-src {a.preview}; frame-ancestors {a.host}; base-uri 'none'; form-action 'none'; object-src 'none'" if role=='editor' else f"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'self'; worker-src 'self'; frame-src 'none'; frame-ancestors {a.editor} {a.host}; base-uri 'none'; form-action 'none'; object-src 'none'")
    # Each role has a separate root, origin, and service account. Only the
    # preview receives the read-only compiled-runtime asset volume.
    headers=f'''add_header Content-Security-Policy "{csp}" always;
add_header X-Content-Type-Options nosniff always;
add_header X-Robots-Tag "noindex, nofollow" always;
add_header Referrer-Policy no-referrer always;
add_header Permissions-Policy "camera=(), microphone=(), geolocation=(), payment=()" always;
add_header Cross-Origin-Resource-Policy cross-origin always;
add_header Access-Control-Allow-Origin "{a.host}" always;
add_header Vary Origin always;
add_header X-Aethyrforge-Release "{a.release}" always;
add_header X-Aethyrforge-Role "{role}" always;
add_header Cache-Control "public, max-age=31536000, immutable" always;
'''
    root_location='location = / { try_files /index.html =404; }\n' if role=='editor' else ''
    assets_location='location /private-assets/ { root /runtime; try_files $uri =404; }\n' if role=='preview' else ''
    conf=f'''server {{
listen 8080;
http2 on;
server_name _;
server_tokens off;
root /app;
autoindex off;
sendfile off;
gzip on;
gzip_vary on;
gzip_proxied any;
gzip_types application/json application/javascript application/wasm application/octet-stream text/css;
gzip_comp_level 4;
{headers}
location = /health {{ access_log off; return 200 'ok'; }}
location ~ (^|/)\\. {{ return 404; }}
location ~ \\.map$ {{ return 404; }}
{root_location}{assets_location}
location / {{ try_files $uri =404; }}
}}
'''
    (root/'default.conf').write_text(conf)
    (root/'Dockerfile').write_text('FROM '+base+'\nCOPY default.conf /etc/nginx/conf.d/default.conf\nCOPY site/ /app/\nEXPOSE 8080\n')
    manifest['files'][role]=[{"path":str(f.relative_to(root/'site')),"sha256":hashlib.sha256(f.read_bytes()).hexdigest(),"bytes":f.stat().st_size} for f in sorted((root/'site').rglob('*')) if f.is_file()]
(Path(a.output)/'release.json').write_text(json.dumps(manifest,indent=2)+'\n')
print(json.dumps({'release':a.release,'asset_set':manifest['asset_set'],'editor':a.editor,'preview':a.preview}))
