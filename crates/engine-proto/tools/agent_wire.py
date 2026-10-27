"""Bounded standard protobuf adapter for the existing SSH stdin channel."""
import base64
from google.protobuf.message import DecodeError
from openagents.engine.v1.agent_pb2 import AgentDispatch, AgentProgress
from engine_contract_limits import LIMITS

PREFIX = 'aethyr-agent-pb-v1:'
MAX_BYTES = 524288


def validate(message, depth=0):
    if depth > 64:
        raise ValueError('Protobuf nesting limit')
    for field, value in message.ListFields():
        length, count = LIMITS[field.full_name]
        repeated = field.label == field.LABEL_REPEATED
        values = value if repeated else [value]
        if repeated and len(value) > count:
            raise ValueError('Protobuf collection limit')
        for item in values:
            if field.type == field.TYPE_MESSAGE:
                validate(item, depth+1)
            elif field.type == field.TYPE_STRING:
                if len(item.encode()) > length or '\0' in item:
                    raise ValueError('Protobuf string limit')
            elif field.type in (field.TYPE_UINT64, field.TYPE_INT64, field.TYPE_SINT64):
                if abs(item) > 9007199254740991:
                    raise ValueError('Protobuf integer limit')


def project(message):
    result = {}
    for field in message.DESCRIPTOR.fields:
        value = getattr(message, field.name)
        if field.label == field.LABEL_REPEATED:
            result[field.name] = [project(x) if field.type == field.TYPE_MESSAGE else x for x in value]
        elif field.type == field.TYPE_MESSAGE:
            if message.HasField(field.name): result[field.name] = project(value)
        elif not field.has_presence or message.HasField(field.name):
            result[field.name] = value
    return result


def encode(value, message_type=AgentProgress):
    from google.protobuf.json_format import ParseDict
    message = ParseDict(value, message_type())
    validate(message)
    data = message.SerializeToString(deterministic=True)
    if len(data) > MAX_BYTES: raise ValueError('Protobuf byte limit')
    return PREFIX + base64.b64encode(data).decode('ascii')


def decode(text, message_type=AgentDispatch):
    if not text.startswith(PREFIX): raise ValueError('Expected agent protobuf envelope')
    data = base64.b64decode(text[len(PREFIX):], validate=True)
    if len(data) > MAX_BYTES: raise ValueError('Protobuf byte limit')
    message = message_type.FromString(data)
    # Dispatch is an admission request: refuse unknown intent fields rather
    # than silently dropping an extension that changes what is authorized.
    if message_type is AgentDispatch:
        known = message_type(); known.CopyFrom(message); known.DiscardUnknownFields()
        if known.SerializeToString(deterministic=True) != message.SerializeToString(deterministic=True):
            raise ValueError('Unknown dispatch field')
    validate(message)
    return project(message)
