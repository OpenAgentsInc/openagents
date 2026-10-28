"""Reference validation for the NEW alpha fixtures, not a Coax runtime codec."""
import math
from google.protobuf.descriptor import FieldDescriptor
from google.protobuf.message import DecodeError
from openagents.options.v1alpha1 import options_pb2 as options
from openagents.verse.presence.v1alpha1.presence_pb2 import PresenceSnapshot


def validate(message):
    settings = message.DESCRIPTOR.GetOptions()
    maximum = settings.Extensions[options.max_message_bytes]
    if maximum and message.ByteSize() > maximum:
        raise ValueError('Message byte limit exceeded')
    for field in message.DESCRIPTOR.fields:
        limits = field.GetOptions()
        repeated = field.label == FieldDescriptor.LABEL_REPEATED
        value = getattr(message, field.name)
        required = limits.Extensions[options.required_value]
        if required:
            if not field.has_presence:
                raise ValueError('Required field lacks explicit presence in schema')
            if not message.HasField(field.name):
                raise ValueError('Missing ' + field.full_name)
        if not repeated and field.has_presence and not message.HasField(field.name):
            continue
        count = limits.Extensions[options.max_count]
        if repeated and count and len(value) > count:
            raise ValueError('Count limit exceeded: ' + field.full_name)
        values = value if repeated else [value]
        for item in values:
            if field.type == FieldDescriptor.TYPE_MESSAGE:
                validate(item)
            elif field.type == FieldDescriptor.TYPE_STRING:
                maximum = limits.Extensions[options.max_bytes]
                if maximum and len(item.encode('utf-8')) > maximum:
                    raise ValueError('String byte limit exceeded: ' + field.full_name)
                grammar = limits.Extensions[options.grammar]
                if grammar == 'nonempty-no-nul' and (not item or '\0' in item):
                    raise ValueError('Invalid identifier: ' + field.full_name)
            elif field.type in (FieldDescriptor.TYPE_FLOAT, FieldDescriptor.TYPE_DOUBLE):
                if not math.isfinite(item):
                    raise ValueError('Non-finite coordinate')
                if limits.Extensions[options.unit] == 'rad' and not -math.pi <= item <= math.pi:
                    raise ValueError('Heading out of range')
    if isinstance(message, PresenceSnapshot):
        ids = [peer.actor_id for peer in message.peers]
        if len(ids) != len(set(ids)):
            raise ValueError('Duplicate actor identity')


def decode_presence(data):
    # Bound input BEFORE protobuf allocation; nested semantic checks follow parse.
    if len(data) > 262144:
        raise ValueError('Presence envelope byte limit exceeded')
    try:
        message = PresenceSnapshot.FromString(data)
    except DecodeError as error:
        raise ValueError('Malformed presence envelope') from error
    validate(message)
    return message
