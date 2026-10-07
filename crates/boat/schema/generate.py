#!/usr/bin/env python3
"""Generate Rust wire types and operations from the pinned Boat specification."""
import json
import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parent.parent
SPEC = yaml.safe_load((ROOT / "schema/boat-v1.yaml").read_text())
SCHEMAS = SPEC["components"]["schemas"]
TYPES = {}
KEYWORDS = {"type", "box", "from", "ref", "self", "match", "in", "loop", "use", "mod", "pub", "extra"}


def snake(name):
    name = re.sub(r"([A-Z]+)([A-Z][a-z])", r"\1_\2", name)
    name = re.sub(r"([a-z0-9])([A-Z])", r"\1_\2", name)
    name = re.sub(r"[^a-zA-Z0-9]+", "_", name).lower()
    return name + "_" if name in KEYWORDS else name


def pascal(name):
    return "".join(x[:1].upper() + x[1:] for x in re.split(r"[^a-zA-Z0-9]", name))


def resolve(value):
    if "$ref" in value:
        return SPEC["components"][value["$ref"].split("/")[-2]][value["$ref"].split("/")[-1]]
    return value


def merged(schema):
    if "allOf" not in schema:
        return schema
    result = {"type": "object", "properties": {}, "required": []}
    for part in schema["allOf"]:
        part = merged(resolve(part))
        result["properties"].update(part.get("properties", {}))
        result["required"] += part.get("required", [])
    return result


def nullable(schema):
    schema = resolve(schema)
    return "null" in schema.get("type", []) or any(p.get("type") == "null" for p in schema.get("oneOf", []))


def rust_type(schema, name):
    if "$ref" in schema:
        return schema["$ref"].split("/")[-1]
    schema = merged(schema)
    kind = schema.get("type")
    if isinstance(kind, list):
        kinds = [k for k in kind if k != "null"]
        if len(kinds) > 1:
            return "serde_json::Value"
        kind = kinds[0] if kinds else "null"
    if "oneOf" in schema or "anyOf" in schema:
        choices = [p for p in schema.get("oneOf", schema.get("anyOf", [])) if p.get("type") != "null"]
        if len(choices) == 1:
            return rust_type(choices[0], name)
        variants = []
        for i, part in enumerate(choices):
            t = rust_type(part, name + str(i + 1))
            variant = {"CommandResponse": "Finished", "CommandStartedResponse": "Started"}.get(t, f"Variant{i + 1}")
            variants.append(f"    {variant}({t}),")
        TYPES[name] = "#[derive(Clone, Serialize, Deserialize)]\n#[serde(untagged)]\npub enum " + name + " {\n" + "\n".join(variants) + "\n}\n" + debug(name)
        return name
    if kind == "object" or "properties" in schema:
        if not schema.get("properties"):
            extra = schema.get("additionalProperties", {})
            t = rust_type(extra, name + "Value") if isinstance(extra, dict) and extra else "serde_json::Value"
            return f"std::collections::BTreeMap<String, {t}>"
        emit_struct(name, schema)
        return name
    if kind == "array":
        return f"Vec<{rust_type(schema.get('items', {}), name + 'Item')}>"
    return {"string": "String", "integer": "i64", "number": "f64", "boolean": "bool", "null": "()"}.get(kind, "serde_json::Value")


def debug(name):
    return f'impl std::fmt::Debug for {name} {{\n    fn fmt(&self, f: &mut std::fmt::Formatter<\'_>) -> std::fmt::Result {{\n        f.debug_struct("{name}").finish_non_exhaustive()\n    }}\n}}\n'


# Fields the spec marks required but its own published examples omit (the
# `apiKeyUsage` example has no `credentialLane`). Decode them as optional.
LENIENT_FIELDS = {"credentialLane"}


def emit_struct(name, schema):
    schema = merged(schema)
    fields = []
    for key, prop in schema.get("properties", {}).items():
        t = rust_type(prop, name + pascal(key))
        optional = key not in schema.get("required", []) or key in LENIENT_FIELDS
        null = nullable(prop)
        attrs = [f'rename = "{key}"']
        if optional and null:
            t = f"crate::Nullable<{t}>"
            attrs += ['default', 'skip_serializing_if = "crate::Nullable::is_unset"']
        elif optional:
            t = f"Option<{t}>"
            attrs += ['default', 'skip_serializing_if = "Option::is_none"']
        elif null:
            t = f"Option<{t}>"
        fields.append('    #[serde(' + ', '.join(attrs) + ')]\n' + f"    pub {snake(key)}: {t},")
    # Preserve fields added by the server without weakening known field types.
    fields.append('    #[serde(flatten)]\n    pub extra: std::collections::BTreeMap<String, serde_json::Value>,')
    TYPES[name] = '#[derive(Clone, Default, Serialize, Deserialize)]\npub struct ' + name + ' {\n' + '\n'.join(fields) + '\n}\n' + debug(name)


for name, schema in SCHEMAS.items():
    if merged(schema).get("properties"):
        emit_struct(name, schema)
    else:
        t = rust_type(schema, name)
        if t != name:
            TYPES[name] = f"pub type {name} = {t};\n"

# Creates and forks carry the caller's Idempotency-Key; only they are replayable among writes.
RETRY_IF_KEYED = {"create", "fork"}
operations = []
methods = []
params = []
for path, item in SPEC["paths"].items():
    for method, op in item.items():
        if method not in {"get", "post", "put", "patch", "delete"}:
            continue
        operation = op["operationId"]
        name = snake(operation)
        pn = pascal(operation) + "Params"
        properties = {}
        required = []
        path_fields = {}
        query = []
        headers = []
        for p in item.get("parameters", []) + op.get("parameters", []):
            p = resolve(p)
            key = p["name"]
            properties[key] = dict(p["schema"])
            if isinstance(properties[key].get("type"), list):
                properties[key]["type"] = next(t for t in properties[key]["type"] if t != "null")
            if p.get("required"):
                required.append(key)
            field = snake(key)
            if p["in"] == "path":
                path_fields[key] = f'&params.{field}.to_string()'
            else:
                target = query if p["in"] == "query" else headers
                target.append((key, field, p.get("required", False)))
        body = op.get("requestBody")
        if body:
            properties["body"] = body["content"]["application/json"]["schema"]
            if body.get("required"):
                required.append("body")
        if properties:
            emit_struct(pn, {"properties": properties, "required": required})
            params.append(pn)
        success = next(v for k, v in op["responses"].items() if str(k).startswith("2"))
        content = success.get("content", {})
        binary = "application/json" not in content
        result = "crate::Download" if binary else rust_type(content["application/json"]["schema"], pascal(operation) + "ResponseBody")
        segments = []
        for segment in path.strip("/").split("/"):
            segments.append(path_fields[segment[1:-1]] if segment.startswith("{") else json.dumps(segment))
        lines = [f'    /// Call `{method.upper()} {path}`.', f"    pub async fn {name}(&self" + (f", params: &{pn}" if properties else "") + f") -> crate::Result<{result}> {{"]
        lines.append('        let path = self.path(&[' + ', '.join(segments) + '])?;')
        for label, values in [("query", query), ("headers", headers)]:
            fixed = [f'({json.dumps(key)}, params.{field}.to_string())' for key, field, mandatory in values if mandatory]
            optional = [(key, field) for key, field, mandatory in values if not mandatory]
            lines.append(f'        let {"mut " if optional else ""}{label}: Vec<(&str, String)> = vec![' + ', '.join(fixed) + '];')
            for key, field in optional:
                lines.append(f'        if let Some(value) = &params.{field} {{ {label}.push(({json.dumps(key)}, value.to_string())); }}')
        b = "None"
        if body:
            if body.get("required"):
                b = "Some(serde_json::to_value(&params.body).map_err(crate::Error::encode)?)"
            else:
                b = "params.body.as_ref().map(serde_json::to_value).transpose().map_err(crate::Error::encode)?"
        retry = "Read" if method == "get" else ("IfKeyed" if operation in RETRY_IF_KEYED else "Never")
        org = any(key == "X-Boat-Org" for key, _, _ in headers)
        op_flags = f'crate::client::Op {{ retry: crate::client::Retry::{retry}, org: {"true" if org else "false"} }}'
        lines.append(f'        self.{"download" if binary else "json"}({op_flags}, reqwest::Method::{method.upper()}, path, &query, &headers, {b}).await')
        lines.append("    }")
        methods.append('\n'.join(lines))
        operations.append({"method": method.upper(), "path": path, "operation_id": operation, "rust_method": name, "response": result, "retry": retry.lower() if retry != "IfKeyed" else "if_idempotency_key"})

for directory in [ROOT / "src/models", ROOT / "src/api"]:
    for file in directory.glob("*.rs"):
        file.unlink()
model_mods = []
# One file per schema keeps generated files reviewable and below the file limit.
for name, code in TYPES.items():
    module = snake(name)
    (ROOT / f"src/models/{module}.rs").write_text('// Generated by `schema/generate.py`.\nuse super::*;\n' + code if not code.startswith("pub type") else '// Generated by `schema/generate.py`.\n' + code)
    model_mods += [f"mod {module};", f"pub use {module}::*;"]
(ROOT / "src/models/mod.rs").write_text('// Generated by `schema/generate.py`.\nuse serde::{Deserialize, Serialize};\n' + '\n'.join(model_mods) + '\n')
api_mods = []
for i in range(0, len(methods), 10):
    module = f"operations_{i // 10}"
    (ROOT / f"src/api/{module}.rs").write_text('// Generated by `schema/generate.py`.\nuse crate::models::*;\nimpl crate::Client {\n' + '\n'.join(methods[i:i + 10]) + '\n}\n')
    api_mods.append(f"mod {module};")
(ROOT / "src/api/mod.rs").write_text('// Generated by `schema/generate.py`.\n' + '\n'.join(api_mods) + '\n')
(ROOT / "schema/operations.json").write_text(json.dumps(operations, indent=2) + '\n')
print(f"Generated {len(TYPES)} types and {len(operations)} operations.")


def sample(schema, include_optional=False):
    schema = merged(resolve(schema))
    if "oneOf" in schema or "anyOf" in schema:
        return sample(schema.get("oneOf", schema.get("anyOf"))[0], include_optional)
    if "const" in schema:
        return schema["const"]
    if "enum" in schema:
        return next(v for v in schema["enum"] if v is not None)
    kind = schema.get("type")
    if isinstance(kind, list):
        kind = next(t for t in kind if t != "null")
    if kind == "object" or "properties" in schema:
        return {k: sample(v) for k, v in schema.get("properties", {}).items() if include_optional or k in schema.get("required", [])}
    if kind == "array":
        return []
    return {"string": "sample", "integer": 1, "boolean": True, "number": 1.0}.get(kind)


from urllib.parse import urlencode
contracts = ['// Generated contract cases from the pinned specification.', 'mod support;', 'use boat::{Error, models::*};']
for path, item in SPEC["paths"].items():
    for method, op in item.items():
        if method not in {"get", "post", "put", "patch", "delete"}:
            continue
        name = snake(op["operationId"])
        values, query, headers = {}, [], []
        target = "/api/v1" + path
        for param in item.get("parameters", []) + op.get("parameters", []):
            param = resolve(param)
            value = sample(param["schema"])
            values[param["name"]] = value
            text = str(value).lower() if isinstance(value, bool) else str(value)
            if param["in"] == "path":
                target = target.replace("{" + param["name"] + "}", text)
            elif param["in"] == "query":
                query.append((param["name"], text))
            else:
                headers.append((param["name"].lower(), text))
        if query:
            target += "?" + urlencode(query)
        if "requestBody" in op:
            values["body"] = sample(op["requestBody"]["content"]["application/json"]["schema"], True)
        contracts += ['#[tokio::test]', f'async fn {name}_contract() {{', '    let (client, job) = support::serve(418, &[], b"refused").await;']
        if values:
            contracts += [f'    let params: {pascal(op["operationId"])}Params = serde_json::from_str(r#"{json.dumps(values)}"#).expect("parameters");']
        contracts += [f'    assert!(matches!(client.{name}(' + ('&params' if values else '') + ').await, Err(Error::Api(_))));', '    let request = job.await.expect("request");', f'    assert_eq!(request.method, "{method.upper()}");', f'    assert_eq!(request.target, "{target}");']
        for key, value in headers:
            contracts.append(f'    assert_eq!(request.headers["{key}"], {json.dumps(value)});')
        if "body" in values:
            contracts.append(f'    assert_eq!(serde_json::from_slice::<serde_json::Value>(&request.body).expect("body"), serde_json::json!({json.dumps(values["body"])}));')
        else:
            contracts.append('    assert!(request.body.is_empty());')
        contracts.append('}')
(ROOT / "tests/contracts.rs").write_text('\n'.join(contracts) + '\n')

examples = ['// Published response examples exercise the generated wire types.', 'use boat::models::*;']
for path, item in SPEC["paths"].items():
    for method, op in item.items():
        if method not in {"get", "post", "put", "patch", "delete"}:
            continue
        for status, response in op["responses"].items():
            if not str(status).startswith("2"):
                continue
            data = response.get("content", {}).get("application/json", {})
            for name, example in data.get("examples", {}).items():
                t = rust_type(data["schema"], pascal(op["operationId"]) + "ResponseBody")
                examples += ['#[test]', f'fn {snake(op["operationId"])}_{snake(name)}() {{', f'    let _: {t} = serde_json::from_str(r##"{json.dumps(example["value"])}"##).expect("published response");', '}']
(ROOT / "tests/published_examples.rs").write_text('\n'.join(examples) + '\n')
