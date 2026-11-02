#!/usr/bin/env python3
# Firm reason: 2 (independent SPIR-V byte/type/decorations/interface/reflection oracle, sharing no compiler or product parser)
"""Read one SPIR-V byte file and print independently decoded facts as JSON.

The spirv(data) function is retained byte-for-byte from the original
check_build.py. Coax owns process deadlines, manifests, identities, catalog
expectations, publication controls and pass/fail verdicts.
"""
import json
import struct
import sys
from pathlib import Path

def spirv(data: bytes) -> dict:
    assert len(data) >= 20 and len(data) % 4 == 0
    words = struct.unpack("<" + str(len(data) // 4) + "I", data)
    assert words[0] == 0x07230203 and words[1] <= 0x00010300 and words[3] > 0 and words[4] == 0
    types, decorations, variables, stages, instructions = {}, {}, {}, {}, []
    at = 5
    while at < len(words):
        count, opcode = words[at] >> 16, words[at] & 65535
        assert 1 <= count <= len(words) - at
        operands = words[at + 1:at + count]
        instructions.append((opcode, operands))
        if 19 <= opcode <= 33:
            types[operands[0]] = (opcode, operands[1:])
        elif opcode == 71:
            decorations.setdefault(operands[0], {})[operands[1]] = operands[2:]
        elif opcode == 59:
            variables[operands[1]] = (operands[0], operands[2])
        elif opcode == 15:
            # Entry-point names occupy NUL-terminated padded u32 words.
            end = 2
            while b"\0" not in struct.pack("<I", operands[end]):
                end += 1
            stages[operands[0]] = operands[end + 1:]
        at += count
    assert at == len(words) and 0 in stages and set(stages) <= {0, 4}

    def value_type(pointer: int) -> tuple[int, int]:
        opcode, values = types[pointer]
        assert opcode == 32
        opcode, values = types[values[1]]
        width = 1
        if opcode == 23:
            width = values[1]
            opcode, values = types[values[0]]
        if opcode == 22:
            assert values == (32,)
            return 2, width
        assert opcode == 21 and values[0] == 32
        return (3 if values[1] else 4), width

    attributes, targets, depth, resources = [], [], False, []
    for identity in stages[0]:
        pointer, storage = variables[identity]
        decor = decorations.get(identity, {})
        if storage == 1 and 30 in decor:
            scalar, width = value_type(pointer)
            attributes.append((decor[30][0], scalar, width))
    for identity in stages.get(4, ()):
        pointer, storage = variables[identity]
        decor = decorations.get(identity, {})
        if storage == 3:
            if 30 in decor:
                targets.append(decor[30][0])
            depth |= decor.get(11) == (22,)
    for identity, (pointer, storage) in variables.items():
        decor = decorations.get(identity, {})
        if 33 not in decor:
            continue
        assert decor[34] == (0,)
        opcode, values = types[pointer]
        assert opcode == 32
        opcode, values = types[values[1]]
        if storage == 12:
            assert opcode == 30 and len(values) == 1
            array = values[0]
            assert types[array][0] == 29
            resources.append((decor[33][0], 1, 0, decorations[array][6][0]))
        else:
            assert storage == 0
            if opcode == 25:
                resources.append((decor[33][0], 2, 2 if values[2] else (1 if values[3] else 0), 0))
            else:
                assert opcode == 26
                resources.append((decor[33][0], 3, None, 0))
    opcodes = {opcode for opcode, _ in instructions}
    assert sorted(targets) == list(range(len(targets)))
    return dict(attributes=sorted(attributes), fragment_targets=len(targets), fragment_depth=depth,
                resources=sorted(resources), native_int64=(17, (11,)) in instructions,
                derivatives=(bool(opcodes & {207, 208, 209, 213, 214, 215}) | (2 if opcodes & {210, 211, 212} else 0)),
                control=(bool(opcodes & {252}) | (2 if 5380 in opcodes else 0)), has_fragment=4 in stages, words=len(words))


if __name__ == "__main__":
    print(json.dumps(spirv(Path(sys.argv[1]).read_bytes()), sort_keys=True))
