#!/usr/bin/env python3
"""Read a CAD page and pick its top face in an isolated test-account browser.

Run through `openagents browser run`. Requires websocket-client.
"""
import argparse
import base64
import json
import os
from pathlib import Path
import time
import urllib.parse
import urllib.request
import websocket


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--url', required=True)
    parser.add_argument('--capture', required=True, type=Path)
    args = parser.parse_args()
    target = urllib.parse.urlsplit(args.url)
    assert target.scheme == 'https' and target.hostname in ('openagents.com', 'staging.openagents.com')
    token = os.environ['OPENAGENTS_BROWSER_TOKEN']
    assert token.startswith(('sess_', 'oa_sk_')), 'Use a dedicated browser session'
    port = os.environ['OPENAGENTS_CHROME_PORT']
    request = urllib.request.Request('http://127.0.0.1:' + port + '/json/new?about:blank', method='PUT')
    tab = json.load(urllib.request.urlopen(request, timeout=10))
    connection = websocket.create_connection(tab['webSocketDebuggerUrl'], suppress_origin=True, timeout=20)
    serial = 0

    def call(method, params):
        nonlocal serial
        serial += 1
        connection.send(json.dumps({'id': serial, 'method': method, 'params': params}))
        while True:
            result = json.loads(connection.recv())
            if result.get('id') == serial:
                assert 'error' not in result, 'Browser command failed: ' + method
                return result.get('result', {})

    def evaluate(expression):
        result = call('Runtime.evaluate', {'expression': expression, 'returnByValue': True, 'awaitPromise': True})
        assert 'exceptionDetails' not in result, 'Browser check failed'
        return result.get('result', {}).get('value')

    try:
        call('Page.enable', {})
        call('Network.enable', {})
        call('Emulation.setDeviceMetricsOverride', {'width': 1280, 'height': 1000, 'deviceScaleFactor': 1, 'mobile': False})
        call('Network.setExtraHTTPHeaders', {'headers': {'X-OpenAgents-Test': '1'}})
        cookie = call('Network.setCookie', {'name': 'oa_cloud_session', 'value': token,
            'url': target.scheme + '://' + target.netloc + '/', 'secure': True, 'httpOnly': True, 'sameSite': 'Lax'})
        assert cookie.get('success'), 'The isolated browser could not sign in'
        call('Page.navigate', {'url': args.url})
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            ready = evaluate("""(() => {
                const page = document.querySelector('[data-cad-doc-page]');
                const canvas = page?.querySelector('canvas');
                return !!page && !!canvas && !canvas.hidden && !!page.querySelector('[data-cad-model]').__cadSetModel;
            })()""")
            if ready:
                break
            time.sleep(0.5)
        assert ready, 'The saved 3D model did not load'
        check = evaluate("""(() => {
            const page = document.querySelector('[data-cad-doc-page]');
            const viewer = page.querySelector('[data-cad-model]');
            viewer.addEventListener('cad-face-pick', e => { window.__cadTestPick = e.detail; });
            viewer.scrollIntoView({block:'center'});
            const r = viewer.querySelector('canvas').getBoundingClientRect();
            return {title:document.querySelector('#cad-doc-title').textContent,
                    version:Number(page.dataset.cadVersion), x:r.x,y:r.y,width:r.width,height:r.height};
        })()""")
        assert check['title'] == '40 mm fan mounting bracket' and check['version'] == 1
        picked = None
        for y in (0.35, 0.45, 0.55, 0.65, 0.25, 0.75):
            for x in (0.35, 0.45, 0.55, 0.65, 0.25, 0.75):
                at = {'x': check['x'] + x * check['width'], 'y': check['y'] + y * check['height']}
                call('Input.dispatchMouseEvent', {'type': 'mouseMoved', **at})
                call('Input.dispatchMouseEvent', {'type': 'mousePressed', 'button': 'left', 'buttons': 1, 'clickCount': 1, **at})
                call('Input.dispatchMouseEvent', {'type': 'mouseReleased', 'button': 'left', 'buttons': 0, 'clickCount': 1, **at})
                picked = evaluate('window.__cadTestPick || null')
                if picked and picked.get('face') == 'body:+z':
                    break
            if picked and picked.get('face') == 'body:+z':
                break
        assert picked and picked.get('face') == 'body:+z', 'The top face could not be picked'
        picture = call('Page.captureScreenshot', {'format': 'png', 'captureBeyondViewport': False})
        args.capture.parent.mkdir(parents=True, exist_ok=True)
        args.capture.write_bytes(base64.b64decode(picture['data']))
        print('PASS isolated browser: saved 3D model and picked top face', flush=True)
    finally:
        connection.close()


if __name__ == '__main__':
    main()
