#!/bin/bash
set -e
python3 -c "import urllib.request; open('/app/answer.txt', 'w').write(urllib.request.urlopen('http://db:8080/ping').read().decode())"
