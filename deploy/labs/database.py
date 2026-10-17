#!/usr/bin/env python3
"""Give Labs a URL-form copy of the existing account database credential."""
import argparse
import subprocess
from urllib.parse import quote, urlencode, urlsplit


def database_url(dsn):
    if dsn.startswith(('postgres://', 'postgresql://')):
        parsed = urlsplit(dsn)
        if not parsed.hostname or not parsed.path.strip('/'):
            raise ValueError('Configure a database address and name.')
        return dsn
    fields = {}
    at = 0
    while at < len(dsn):
        while at < len(dsn) and dsn[at].isspace():
            at += 1
        if at == len(dsn):
            break
        start = at
        while at < len(dsn) and (dsn[at].isalnum() or dsn[at] == '_'):
            at += 1
        key = dsn[start:at]
        while at < len(dsn) and dsn[at].isspace():
            at += 1
        if not key or at == len(dsn) or dsn[at] != '=' or key in fields:
            raise ValueError('Check the database connection fields.')
        at += 1
        while at < len(dsn) and dsn[at].isspace():
            at += 1
        quoted = at < len(dsn) and dsn[at] == "'"
        if quoted:
            at += 1
        value = []
        closed = not quoted
        while at < len(dsn):
            char = dsn[at]
            at += 1
            if char == '\\':
                if at == len(dsn):
                    raise ValueError('Check the database connection escaping.')
                value.append(dsn[at])
                at += 1
            elif quoted and char == "'":
                closed = True
                if at < len(dsn) and not dsn[at].isspace():
                    raise ValueError('Separate database connection fields with spaces.')
                break
            elif not quoted and char.isspace():
                break
            else:
                value.append(char)
        if not closed:
            raise ValueError('Close the quoted database connection field.')
        fields[key] = ''.join(value)
    allowed = {'host', 'port', 'user', 'password', 'dbname', 'sslmode',
               'connect_timeout', 'application_name', 'options'}
    if fields.keys() - allowed or not all(fields.get(k) for k in ('host', 'user', 'dbname')):
        raise ValueError('Use one database host, user, and database with supported options.')
    if ',' in fields['host'] or ',' in fields.get('port', ''):
        raise ValueError('Configure one database host for Labs.')
    user = quote(fields.pop('user'), safe='')
    password = fields.pop('password', '')
    database = quote(fields.pop('dbname'), safe='')
    credentials = user + (':' + quote(password, safe='') if password else '')
    # Both drivers recognize host= in the query, including Cloud SQL sockets.
    return f'postgresql://{credentials}@localhost/{database}?' + urlencode(fields)


def command(args, *, data=None, missing=False):
    result = subprocess.run(['gcloud', *args, '--project', 'openagentsgemini'],
                            input=data, capture_output=True)
    if result.returncode:
        if missing and b'NOT_FOUND' in result.stderr:
            return None
        raise RuntimeError('The database secret operation failed; check gcloud access.')
    return result.stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('environment', choices=['staging', 'production'])
    args = parser.parse_args()
    env = args.environment
    source = f'openagents-{env}-pg-dsn'
    target = f'openagents-labs-{env}-pg-url'
    raw = command(['secrets', 'versions', 'access', 'latest', '--secret', source]).decode().strip()
    url = database_url(raw).encode()
    exists = command(['secrets', 'describe', target, '--format=value(name)'], missing=True)
    if exists is None:
        command(['secrets', 'create', target, '--replication-policy=automatic', '--quiet'])
    previous = command(['secrets', 'versions', 'access', 'latest', '--secret', target], missing=True) if exists else None
    if previous is None or previous.strip() != url:
        command(['secrets', 'versions', 'add', target, '--data-file=-', '--quiet'], data=url)
    runtime = ('oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com' if env == 'staging'
               else '157437760789-compute@developer.gserviceaccount.com')
    command(['secrets', 'add-iam-policy-binding', target,
             '--member=serviceAccount:' + runtime, '--role=roles/secretmanager.secretAccessor',
             '--condition=None', '--quiet'])
    print('Configured ' + target + ' for the Labs runtime.')


if __name__ == '__main__':
    main()
