"""Check the published v1 descriptor for wire and generated API changes."""
from google.protobuf import descriptor_pb2


def definitions(descriptor):
    messages, enums = {}, {}
    def walk(prefix, values):
        for message in values:
            name = prefix + '.' + message.name
            messages[name] = message
            for enum in message.enum_type:
                enums[name + '.' + enum.name] = enum
            walk(name, message.nested_type)
    for file in descriptor.file:
        walk(file.package, file.message_type)
        for enum in file.enum_type:
            enums[file.package + '.' + enum.name] = enum
    return messages, enums


def check(before, after):
    old_messages, old_enums = definitions(before)
    messages, enums = definitions(after)
    errors = []
    for name, old in old_messages.items():
        if name not in messages:
            errors.append(f'{name}: message removed')
            continue
        new = messages[name]
        by_number = {f.number: f for f in new.field}
        for field in old.field:
            current = by_number.get(field.number)
            if current is None:
                reserved = any(r.start <= field.number < r.end for r in new.reserved_range)
                if not reserved or field.name not in new.reserved_name:
                    errors.append(f'{name}.{field.name}: removed field must reserve its name and number')
                continue
            def shape(owner, f):
                oneof = owner.oneof_decl[f.oneof_index].name if f.HasField('oneof_index') else None
                return f.name, f.type, f.type_name, f.label, f.proto3_optional, oneof
            if shape(old, field) != shape(new, current):
                errors.append(f'{name}.{field.number}: field name, type, cardinality, or presence changed')
        # A previously retired number or name cannot return in a later version.
        for field in new.field:
            if field.name in old.reserved_name or any(r.start <= field.number < r.end for r in old.reserved_range):
                errors.append(f'{name}.{field.number}: reserved field reused')
    for name, old in old_enums.items():
        if name not in enums:
            errors.append(f'{name}: enum removed')
            continue
        new = enums[name]
        current = {v.number: v.name for v in new.value}
        for value in old.value:
            if value.number not in current:
                if value.name not in new.reserved_name or not any(r.start <= value.number <= r.end for r in new.reserved_range):
                    errors.append(f'{name}.{value.name}: removed enum must reserve its name and number')
            elif current[value.number] != value.name:
                errors.append(f'{name}.{value.number}: enum renamed or reused')
        for value in new.value:
            if value.name in old.reserved_name or any(r.start <= value.number <= r.end for r in old.reserved_range):
                errors.append(f'{name}.{value.number}: reserved enum reused')
    return errors


def parse(data):
    value = descriptor_pb2.FileDescriptorSet()
    value.ParseFromString(data)
    return value
