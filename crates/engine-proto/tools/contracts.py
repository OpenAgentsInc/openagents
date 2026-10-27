"""Generate JSON-presentation field names and limits for callback-based codecs.
Nanopb remains the wire parser/encoder; this table is derived, never authored.
"""
import re
from pathlib import Path
from google.protobuf import descriptor_pb2


def generate(descriptor, schemas, output):
    descriptors = descriptor_pb2.FileDescriptorSet.FromString(descriptor)
    files = [file for file in descriptors.file if file.name != 'openagents/engine/v1/scene.proto']
    lines = ['// Generated from proto/. Do not edit.', '#define OPENAGENTS_CONTRACT_TABLE_VERSION 2']
    messages = []
    limits = {}
    for file in files:
        stem = Path(file.name).with_suffix('')
        lines.append(f'#include "{stem}.pb.h"')
        options = []
        for line in (schemas / Path(file.name).with_suffix('.options')).read_text().splitlines():
            line = line.split('#', 1)[0].strip()
            if line:
                pattern, _, settings = line.partition(' ')
                options.append((re.compile('^'+re.escape(pattern).replace(r'\*', '.*')+'$'), settings))
        for message in file.message_type:
            full = file.package + '.' + message.name
            c = full.replace('.', '_')
            lines.append(f'static const WireFieldDef fields_{c}[] = {{')
            for field in message.field:
                length, count = 65536, 100
                for pattern, settings in options:
                    if pattern.fullmatch(full+'.'+field.name):
                        for key, value in re.findall(r'(max_length|max_count):(\d+)', settings):
                            if key == 'max_length': length = int(value)
                            else: count = int(value)
                repeated = 'true' if field.label == field.LABEL_REPEATED else 'false'
                subtype = field.type_name.lstrip('.')
                limits[full+'.'+field.name] = (length, count)
                presence = 'true' if field.proto3_optional or field.type == field.TYPE_MESSAGE else 'false'
                lines.append(f'  {{{field.number}, "{field.name}", {field.type}, {repeated}, {length}, {count}, "{subtype}", {presence}}},')
            lines.append('};')
            messages.append((full, c, len(message.field)))
    lines.append('static const WireMessageDef wire_messages[] = {')
    for full, c, count in messages:
        lines.append(f'  {{"{full}", &{c}_msg, sizeof({c}), fields_{c}, {count}}},')
    lines.append('};\n')
    target = output / 'generated/c/openagents/engine/v1/contracts.inc'
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text('\n'.join(lines))

    python = output / 'generated/python/engine_contract_limits.py'
    python.parent.mkdir(parents=True, exist_ok=True)
    python.write_text('# Generated from proto/ options. Do not edit.\nLIMITS = '+repr(limits)+'\n')
