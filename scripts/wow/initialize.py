#!/usr/bin/env python3
"""Initialize a fresh private realm; refuse to overwrite an existing database."""
import getpass
import hashlib
import json
import os
from pathlib import Path
import secrets
import subprocess
import time

root = Path(os.environ.get('WOW_GYM_ROOT', str(Path.home() / 'wow-gym'))).resolve()
os.umask(0o077)
if not (root / 'install/etc/mangosd.conf.dist').is_file():
    raise SystemExit('build and install the pinned realm before initialization')
if (root / 'mysql').exists():
    raise SystemExit('database already exists; use the realm runbook for recovery')
root.mkdir(parents=True, exist_ok=True)
(root / 'logs').mkdir(exist_ok=True)
(root / 'tmp').mkdir(exist_ok=True)
archive = root / 'db-4641790.zip'
if not archive.exists():
    subprocess.run(['curl', '-fL', 'https://github.com/vmangos/core/releases/download/db_latest/db-4641790.zip', '-o', str(archive)], check=True)
if hashlib.sha256(archive.read_bytes()).hexdigest() != '9e178f1d70363e92a7b8c259000102dc9739333d79ca00c99ea32dd1fb43e904':
    raise SystemExit('database archive digest mismatch')
subprocess.run(['7z', 'x', str(archive), '-o' + str(root), '-y'], check=True, stdout=subprocess.DEVNULL)
subprocess.run(['mariadb-install-db', '--no-defaults', '--datadir=' + str(root / 'mysql'), '--auth-root-authentication-method=socket'], check=True, stdout=subprocess.DEVNULL)
log = (root / 'logs/mysql.log').open('ab')
subprocess.Popen(['mariadbd', '--no-defaults', '--datadir=' + str(root / 'mysql'), '--socket=' + str(root / 'mysql.sock'), '--pid-file=' + str(root / 'mysql.pid'), '--bind-address=127.0.0.1', '--port=13306', '--sql-mode=', '--tmpdir=' + str(root / 'tmp')], stdout=log, stderr=log, start_new_session=True)
client = ['mariadb', '--no-defaults', '--socket=' + str(root / 'mysql.sock'), '-u', getpass.getuser()]
for _ in range(60):
    if subprocess.run(client + ['-e', 'SELECT 1'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode == 0:
        break
    time.sleep(1)
else:
    raise SystemExit('private database did not start')

def sql(text, database=None):
    subprocess.run(client + ([database] if database else []), input=text.encode(), check=True, stdout=subprocess.DEVNULL)

for database, filename in [('realmd', 'logon'), ('characters', 'characters'), ('mangos', 'mangos'), ('logs', 'logs')]:
    sql('CREATE DATABASE ' + database + ' CHARACTER SET utf8mb3;')
    with (root / 'mysql-dump' / (filename + '.sql')).open('rb') as source:
        subprocess.run(client + [database], stdin=source, check=True)
password = secrets.token_hex(16)
sql("CREATE USER 'gym'@'127.0.0.1' IDENTIFIED BY '" + password + "';")
for database in ['realmd', 'characters', 'mangos', 'logs']:
    sql('GRANT ALL ON ' + database + ".* TO 'gym'@'127.0.0.1';")
address = os.environ.get('WOW_REALM_ADDRESS', '100.74.238.61')
# Only numeric private/Tailscale addresses are accepted as SQL/config inputs.
import ipaddress
ip = ipaddress.ip_address(address)
if not (ip.is_private or ip in ipaddress.ip_network('100.64.0.0/10')):
    raise SystemExit('realm address must be private')
sql("DELETE FROM realmlist; INSERT INTO realmlist (id,name,address,localAddress,localSubnetMask,port,gamebuild_min,gamebuild_max) VALUES (1,'OpenAgents gym','" + address + "','" + address + "','255.255.255.255',8085,5875,5875);", 'realmd')
accounts = []
prime = int('894B645E89E1535BBDAD5B8B290650530801B18EBFBF5E8FAB3C82872A3E9BB7', 16)
for number in range(21):
    name = 'GYMSETUP' if number == 0 else 'GYM' + str(number)
    secret = secrets.token_hex(8).upper()
    salt = bytearray(secrets.token_bytes(32))
    salt[31] |= 128
    salt[0] |= 1
    inner = hashlib.sha1((name + ':' + secret).encode()).digest()
    x = int.from_bytes(hashlib.sha1(bytes(salt) + inner).digest(), 'little')
    verifier = pow(7, x, prime)
    sql("INSERT INTO account (username,v,s,gmlevel) VALUES ('" + name + "','" + format(verifier, 'X') + "','" + format(int.from_bytes(salt, 'little'), 'X') + "'," + ('4' if number == 0 else '0') + ");", 'realmd')
    accounts.append({'account': name, 'password': secret})
sql("INSERT INTO account_access (id,gmlevel,RealmID) SELECT id,4,1 FROM account WHERE username='GYMSETUP';", 'realmd')
(root / 'accounts.json').write_text(json.dumps(accounts, indent=2) + '\n')
for binary in ['realmd', 'mangosd']:
    template = (root / 'install/etc' / (binary + '.conf.dist')).read_text()
    values = {'BindIP': '"' + address + '"', 'LogsDir': '"' + str(root / 'logs') + '"', 'DataDir': '"' + str(root / 'client') + '"', 'Warden.WinEnabled': '0', 'Warden.OSXEnabled': '0', 'Console.Enable': '0', 'Ra.Enable': '0', 'SOAP.Enabled': '0', 'PidFile': '"' + str(root / (binary + '.pid')) + '"'}
    for key, database in [('LoginDatabaseInfo', 'realmd'), ('LoginDatabase.Info', 'realmd'), ('WorldDatabase.Info', 'mangos'), ('CharacterDatabase.Info', 'characters'), ('LogsDatabase.Info', 'logs')]:
        values[key] = '"127.0.0.1;13306;gym;' + password + ';' + database + '"'
    lines = []
    for line in template.splitlines():
        key = line.split('=', 1)[0].strip()
        lines.append(key + ' = ' + values[key] if key in values else line)
    (root / (binary + '.conf')).write_text('\n'.join(lines) + '\n')
print('Private database, realm configuration, 20 gym accounts, and setup account initialized.')
