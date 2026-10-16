#!/usr/bin/env python3
"""Extract the public System One slice from the owning decision API contract.

Requires PyYAML. Run with --check to verify deterministic regeneration without
writing files. No network, credentials, inference, or server code is used.
"""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import yaml

ROOT = Path(__file__).resolve().parents[2]
SOURCE = ROOT / 'docs/decision-models/api/openapi.yaml'
OUT = ROOT / 'docs/sdk/openapi.systemone.json'
PATH = '/v1/systemone'
PREFIX = '#/components/'


class UniqueLoader(yaml.SafeLoader):
    """Reject duplicate mapping keys instead of silently changing the contract."""


def mapping(loader, node, deep=False):
    result = {}
    for key_node, value_node in node.value:
        key = loader.construct_object(key_node, deep=deep)
        if key in result:
            raise ValueError('Duplicate YAML key: ' + str(key))
        result[key] = loader.construct_object(value_node, deep=deep)
    return result


UniqueLoader.add_constructor(yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, mapping)


def generate(source):
    source = copy.deepcopy(source)
    operation = source['paths'][PATH]['post']
    operation['operationId'] = 'answerSystemOne'
    operation['tags'] = ['System One']
    operation.setdefault('security', source.get('security', []))
    included = {}
    pending = []

    def component(section, name):
        key = (section, name)
        if key not in included:
            included[key] = None
            pending.append(key)
        return 'SystemOne' + name[0].upper() + name[1:]

    def rewrite(value):
        if isinstance(value, dict):
            for key, child in list(value.items()):
                if key == '$ref':
                    if not child.startswith(PREFIX):
                        raise ValueError('System One needs a local component reference: ' + child)
                    section, name = child[len(PREFIX):].split('/')
                    value[key] = PREFIX + section + '/' + component(section, name)
                elif key == 'security':
                    for requirement in child:
                        for name in list(requirement):
                            requirement[component('securitySchemes', name)] = requirement.pop(name)
                else:
                    rewrite(child)
        elif isinstance(value, list):
            for child in value:
                rewrite(child)

    rewrite(operation)
    while pending:
        section, name = pending.pop()
        value = copy.deepcopy(source['components'][section][name])
        rewrite(value)
        included[(section, name)] = value
    components = {}
    for (section, name), value in sorted(included.items()):
        components.setdefault(section, {})['SystemOne' + name[0].upper() + name[1:]] = value
    return {
        'openapi': '3.1.0',
        'info': {'title': 'OpenAgents public System One slice', 'version': source['info']['version'],
                 'description': 'Only POST /v1/systemone and its referenced components. Other decision-gateway routes are not public API-front operations.'},
        'servers': [{'url': 'https://api.openagents.com'}],
        'paths': {PATH: {'post': operation}},
        'components': components,
        'x-openagents-source': {'path': str(SOURCE.relative_to(ROOT)), 'sha256': hashlib.sha256(SOURCE.read_bytes()).hexdigest()},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    document = generate(yaml.load(SOURCE.read_text(), Loader=UniqueLoader))
    rendered = json.dumps(document, indent=2, ensure_ascii=False, sort_keys=True) + '\n'
    if args.check:
        assert OUT.read_text() == rendered, 'Regenerate with python3 scripts/api/systemone_openapi.py'
        print('PASS generated System One slice matches its owning contract')
    else:
        OUT.write_text(rendered)
        print('Wrote', OUT.relative_to(ROOT))


if __name__ == '__main__':
    main()
