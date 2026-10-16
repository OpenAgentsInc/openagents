#!/usr/bin/env python3
"""Generate Rust wire bindings from the Rust-exported Computers OpenAPI subset.

This is repository build tooling, not an API implementation. Python's standard
library and the repository's pinned rustfmt are its only dependencies. Run with
--check to compare every generated artifact without changing it.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[2]
SPEC = ROOT / 'docs/sdk/openapi.computers.json'
OUT = ROOT / 'crates/openagents-sdk/src/generated/computers.rs'
INDEX = ROOT / 'docs/sdk/computers-operations.json'
VERSION = 1
METHODS = {'get', 'post', 'put', 'patch', 'delete'}
KEYWORDS = {'type', 'box', 'ref', 'self', 'Self', 'match', 'in', 'loop', 'use',
            'mod', 'pub', 'move', 'async', 'await', 'where', 'fn', 'impl', 'enum'}


def snake(name):
    name = re.sub(r'([A-Z]+)([A-Z][a-z])', r'\1_\2', name)
    name = re.sub(r'([a-z0-9])([A-Z])', r'\1_\2', name)
    name = re.sub(r'[^a-zA-Z0-9]+', '_', name).lower()
    return name + '_' if name in KEYWORDS else name


def pascal(name):
    return ''.join(p[:1].upper() + p[1:] for p in re.split(r'[^a-zA-Z0-9]', name))


def literal(value):
    return json.dumps(value, ensure_ascii=False)


def validate(spec):
    assert spec['openapi'] == '3.1.0', 'Unsupported OpenAPI version'
    assert spec['servers'] == [{'url': 'https://api.openagents.com'}], 'Wrong public origin'
    identifiers = set()

    def refs(value):
        if isinstance(value, dict):
            for key, child in value.items():
                if key == '$ref':
                    assert child.startswith('#/components/schemas/'), 'Unsupported external reference'
                    assert child.rsplit('/', 1)[1] in spec['components']['schemas'], 'Missing component'
                else:
                    refs(child)
        elif isinstance(value, list):
            for child in value:
                refs(child)

    refs(spec)
    for path, item in spec['paths'].items():
        assert path == '/v1/computers' or path.startswith('/v1/computers/') or path in ('/v1/device/code', '/v1/device/token', '/v1/device/sign-out', '/v1/account'), 'Internal or unrelated path'
        for method, op in item.items():
            assert method in METHODS, 'Unsupported path item'
            ident = op['operationId']
            assert ident not in identifiers, 'Duplicate operation ID'
            identifiers.add(ident)
            security = [] if ident in ('startAppLogin', 'pollAppLogin') else [{'appBearer': []}]
            assert op['security'] == security, 'Unexpected credential boundary'
            parameters = item.get('parameters', []) + op.get('parameters', [])
            actual = {p['name'] for p in parameters if p['in'] == 'path' and p.get('required')}
            assert actual == set(re.findall(r'\{([^}]+)\}', path)), 'Missing path parameter'
            for response in op['responses'].values():
                assert 'text/event-stream' not in response.get('content', {}), 'Polling and SSE need distinct transports'
    return identifiers


class Generator:
    def __init__(self, spec):
        self.spec = spec
        self.types = {}

    def kind(self, schema):
        kind = schema.get('type')
        if isinstance(kind, list):
            kind = next((v for v in kind if v != 'null'), 'null')
        if kind is None and 'const' in schema:
            value = schema['const']
            kind = 'boolean' if isinstance(value, bool) else 'integer' if isinstance(value, int) else 'string'
        return kind

    def nullable(self, schema):
        return 'null' in schema.get('type', [])

    def debug(self, name):
        return f'''impl std::fmt::Debug for {name} {{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {{
        f.debug_struct("{name}").finish_non_exhaustive()
    }}
}}'''

    def rust_type(self, schema, name):
        if '$ref' in schema:
            return schema['$ref'].rsplit('/', 1)[1]
        choices = schema.get('anyOf', schema.get('oneOf'))
        if choices:
            variants = [self.rust_type(part, name + str(i)) for i, part in enumerate(choices)]
            self.types[name] = '#[derive(Clone, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum ' + name + ' {\n' + '\n'.join(f'    Variant{i}({t}),' for i, t in enumerate(variants)) + '\n}\n' + self.debug(name)
            return name
        kind = self.kind(schema)
        if kind == 'object' or 'properties' in schema:
            if not schema.get('properties'):
                extra = schema.get('additionalProperties', {})
                value = self.rust_type(extra, name + 'Value') if isinstance(extra, dict) and extra else 'serde_json::Value'
                return f'std::collections::BTreeMap<String, {value}>'
            self.struct(name, schema)
            return name
        if kind == 'array':
            return f'Vec<{self.rust_type(schema["items"], name + "Item")}>'
        if kind == 'integer':
            return 'u64' if schema.get('minimum', -1) >= 0 else 'i64'
        return {'string': 'String', 'number': 'f64', 'boolean': 'bool', 'null': '()'}.get(kind, 'serde_json::Value')

    def struct(self, name, schema, extra=True):
        fields = []
        for key, prop in schema.get('properties', {}).items():
            typ = 'Vec<u8>' if prop.get('x-rust-binary') else self.rust_type(prop, name + pascal(key))
            optional = key not in schema.get('required', [])
            nullable = self.nullable(prop)
            attrs = [f'rename = {literal(key)}']
            if optional or nullable:
                typ = f'Option<{typ}>'
            if optional:
                attrs += ['default', 'skip_serializing_if = "Option::is_none"']
            fields.append('    #[serde(' + ', '.join(attrs) + ')]\n' + f'    pub {snake(key)}: {typ},')
        if extra:
            fields.append('    #[serde(flatten)]\n    pub extra: std::collections::BTreeMap<String, serde_json::Value>,')
        # Required request parameters cannot be silently supplied by Default.
        default = ', Default' if extra or not schema.get('required') else ''
        self.types[name] = '#[derive(Clone, Serialize, Deserialize' + default + ')]\npub struct ' + name + ' {\n' + '\n'.join(fields) + '\n}\n' + self.debug(name)

    def operation(self, path, method, op):
        ident = op['operationId']
        name = pascal(ident)
        parameters = op.get('parameters', [])
        properties = {p['name']: p['schema'] for p in parameters}
        required = [p['name'] for p in parameters if p.get('required')]
        body = op.get('requestBody')
        mime = None
        if body:
            mime = 'application/json' if 'application/json' in body['content'] else 'application/octet-stream'
            properties['body'] = {'x-rust-binary': True} if mime == 'application/octet-stream' else body['content'][mime]['schema']
            if body.get('required'):
                required.append('body')
        self.struct(name + 'Params', {'properties': properties, 'required': required}, extra=False)
        query, headers = [], []
        for parameter in parameters:
            if parameter['in'] == 'path':
                continue
            target = query if parameter['in'] == 'query' else headers
            field = snake(parameter['name'])
            pair = f'({literal(parameter["name"])}, value.to_string())'
            if parameter.get('required'):
                target.append(f'        {{ let value = self.{field}; values.push({pair}); }}')
            else:
                target.append(f'        if let Some(value) = self.{field} {{ values.push({pair}); }}')
        success = {status: response for status, response in op['responses'].items() if status.startswith('2')}
        json_schemas = [r['content']['application/json']['schema'] for r in success.values() if 'application/json' in r.get('content', {})]
        binary = any('application/octet-stream' in r.get('content', {}) for r in success.values())
        empty = any(not r.get('content') for r in success.values())
        result = 'Vec<u8>' if binary else self.rust_type(json_schemas[0], name + 'Response') if json_schemas else '()'
        if empty and json_schemas:
            result = f'Option<{result}>'
        segments = ['self.' + snake(s[1:-1]) if s.startswith('{') else literal(s) + '.to_owned()' for s in path.split('/')[2:]]
        body_expression = 'RequestBody::Empty'
        if body:
            if mime == 'application/octet-stream':
                body_expression = 'RequestBody::Binary(self.body)'
            elif body.get('required'):
                body_expression = 'RequestBody::Json(serde_json::to_value(self.body)?)'
            else:
                body_expression = 'self.body.map(serde_json::to_value).transpose()?.map(RequestBody::Json).unwrap_or(RequestBody::Empty)'
        const = snake(ident).upper()
        auth = 'Account' if op['security'] else 'Public'
        lines = [f'pub type {name}Result = {result};', f'pub const {const}: Operation = Operation {{',
                 f'    id: {literal(ident)}, method: {literal(method.upper())}, path: {literal(path)},',
                 f'    authority: Authority::{auth}, success: &[' + ','.join(success) + '],',
                 f'    binary_response: {str(binary).lower()}, empty_response: {str(empty).lower()},',
                 f'    max_request_bytes: {"Some(" + str(op["x-openagents-max-request-bytes"]) + ")" if "x-openagents-max-request-bytes" in op else "None"}, max_response_bytes: {op.get("x-openagents-max-response-bytes", 1048576)},',
                 f'    retry: {literal(op.get("x-openagents-retry", "manual"))},', '};',
                 f'impl {name}Params {{', '    pub fn into_request(self) -> Result<Request, serde_json::Error> {']
        for field, values in [('query', query), ('headers', headers)]:
            lines += [f'        let {field} = {{', '            let mut values: Vec<(&\'static str, String)> = Vec::new();', *values, '            values', '        };'] if values else [f'        let {field} = Vec::new();']
        lines += [f'        let body = {body_expression};', f'        Ok(Request {{ operation: &{const}, path_segments: vec![' + ','.join(segments) + '], query, headers, body })', '    }', '}']
        metadata = {'operation_id': ident, 'method': method.upper(), 'path': path, 'params': name + 'Params', 'result': name + 'Result', 'authority': auth.lower(), 'success': sorted(map(int, success)), 'binary_request': mime == 'application/octet-stream', 'binary_response': binary, 'retry': op.get('x-openagents-retry', 'manual'), 'max_request_bytes': op.get('x-openagents-max-request-bytes'), 'max_response_bytes': op.get('x-openagents-max-response-bytes', 1048576)}
        return '\n'.join(lines), metadata


PRELUDE = '''use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Authority { Public, Account }
#[derive(Debug)]
pub struct Operation {
    pub id: &'static str, pub method: &'static str, pub path: &'static str,
    pub authority: Authority, pub success: &'static [u16],
    pub binary_response: bool, pub empty_response: bool, pub retry: &'static str,
    pub max_request_bytes: Option<usize>, pub max_response_bytes: usize,
}
pub enum RequestBody { Empty, Json(serde_json::Value), Binary(Vec<u8>) }
/// Unencoded path components relative to Deployment::api_base; never a URL.
pub struct Request {
    pub operation: &'static Operation, pub path_segments: Vec<String>,
    pub query: Vec<(&'static str, String)>, pub headers: Vec<(&'static str, String)>,
    pub body: RequestBody,
}
impl std::fmt::Debug for Request {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Request").field("operation", &self.operation.id).finish_non_exhaustive()
    }
}
'''


def generate(raw):
    spec = json.loads(raw)
    validate(spec)
    generator = Generator(spec)
    for name, schema in sorted(spec['components']['schemas'].items()):
        if schema.get('properties'):
            generator.struct(name, schema)
        else:
            typ = generator.rust_type(schema, name)
            if typ != name:
                generator.types[name] = f'pub type {name} = {typ};'
    operations, metadata = [], []
    for path, item in sorted(spec['paths'].items()):
        for method, op in sorted(item.items()):
            operation, record = generator.operation(path, method, op)
            operations.append(operation)
            metadata.append(record)
    digest = hashlib.sha256(raw).hexdigest()
    source = f'// Generated by scripts/api/computers_codegen.py, version {VERSION}.\n// Edit the owning Rust schema, export it, and regenerate.\n' + PRELUDE
    source += f'pub const SCHEMA_SHA256: &str = "{digest}";\n'
    source += '\n'.join(code for _, code in sorted(generator.types.items())) + '\n' + '\n'.join(operations)
    source += '\npub const OPERATIONS: &[&Operation] = &[' + ','.join('&' + snake(m['operation_id']).upper() for m in metadata) + '];\n'
    toolchain = tomllib.loads((ROOT / 'rust-toolchain.toml').read_text())['toolchain']['channel']
    source = subprocess.run(['rustup', 'run', toolchain, 'rustfmt', '--edition', '2024', '--emit', 'stdout'], input=source, text=True, capture_output=True, check=True).stdout
    index = {'format': 'openagents.computers.operations.v1', 'generator_version': VERSION, 'schema_sha256': digest, 'operations': metadata}
    return {OUT: source, INDEX: json.dumps(index, indent=2) + '\n'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    outputs = generate(SPEC.read_bytes())
    for path, text in outputs.items():
        if args.check:
            assert path.exists() and path.read_text() == text, 'Generated artifact drift: ' + str(path.relative_to(ROOT))
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
    count = len(json.loads(outputs[INDEX])['operations'])
    print('PASS Computers generation' + (' check' if args.check else '') + f': {count} operations')


if __name__ == '__main__':
    main()
