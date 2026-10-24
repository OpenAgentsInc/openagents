# Behavior bytecode v1

Status: proposed normative safe-language and VM format. No compiler or VM exists.
[Scripting](scripting.md) defines creator experience and state migration;
[security](security.md) defines origin and capability isolation. Bytecode never
authorizes native JavaScript evaluation or arbitrary Wasm modules.

## Module representation

The safe TypeScript dialect compiles into an immutable module plus a manifest
pinning bytecode hash, language/SDK versions, imported function signatures,
capability requirements, state schema, and resource profile. TypeScript's full
runtime and npm ecosystem are not implied by this syntax choice.

The portable VM identity is `af.behavior.1`, matching release `vmVersion`.
It maps to bytecode binary major/minor 1.0. The baseline module manifest
requires `engineAbi: af.kernel.1`; these named compatibility identities differ
from exact bytecode/engine artifact hashes. The loader checks all identities,
binary versions, imported SDK signatures, and resource profile before execution.
A VM-major change receives a new VM identity and cannot borrow compatibility
from an unchanged kernel ABI. See [runtime ABI](runtime-abi.md).

The binary header is 64 little-endian bytes. Bytes 0–7 are `AFBCv1` followed by
two zero bytes; 8/10 are major/minor `u16`; 12 is feature flags `u32`; 16 is
total length `u32`. Fields at 20/24/28/32/36 are `u32` offsets for type,
constant, function, code, and source-map sections. Fields at 40/44/48 are their
type, constant, and function counts; 52/56 are code and source-map lengths;
60 is reserved zero. Sections follow that order at eight-byte-aligned offsets,
with zero padding and no overlap or trailing data. Required unknown features
or versions fail before instantiation.

The type section is bounded JCS describing indexed function and state types.
It begins with a `u32` JSON byte length and reserved zero `u32`; bytes after
that JSON and before the next section are zero padding. Constants use
an eight-byte prefix: tag `u8`, three reserved zero bytes, payload length `u32`,
then payload and zero padding. Tags are 1 for finite little-endian `f64` and
2 for UTF-8 strings. Function descriptors are 32 bytes: function ID, entry and
end byte offsets (`u32`); parameter type, return type, local count, and declared
maximum stack (`u16`); flags `u32`; eight reserved zero bytes. Source-map rows
are four `u32` values: instruction PC, source-file index, start byte, end byte.
The manifest's source-file table gives immutable source hashes, not fetch URLs.

## Values and state

Runtime values are finite `f64`, boolean, string, null, bounded array, typed
record, tagged union, entity UUID, or opaque capability/request handle. There
are no prototypes, property getters, implicit object coercions, or cyclic graphs.
Record fields have fixed schema indexes. Strings contain valid UTF-8; indexing
uses Unicode scalar values, with byte counts charged separately. Equality is
value equality for scalars and stable IDs; compound values require explicit
bounded comparison. No object identity leaks a VM address.

Arithmetic uses IEEE-754 binary64 round-to-nearest ties-to-even without fused
contraction. Nonfinite arithmetic traps, negative zero normalizes to positive
zero on state serialization, and division by zero fails. Math library functions
use pinned owned implementations and conformance vectors. This defines the VM
only; it does not make physics or GPU results deterministic across machines.
Time is the supplied logical tick; random values come only from the instance's
serialized seeded stream. The stream uses
[xoshiro128** 1.1](https://prng.di.unimi.it/xoshiro128starstar.c) with four `u32` words,
standard rotate-left transitions, and unsigned output divided by 2^32. Seed
expansion hashes the domain `aethyrforge.behavior.rng.v1`, session seed, and
instance UUID; the first 16 digest bytes form four little-endian words,
replacing all-zero with (1,0,0,0). Stream state survives compatible reload and
is unsuitable for secrets.

State schemas allow depth 8, strings of at most 1,024 UTF-8 bytes, arrays of at
most 256 elements, and records of at most 64 fields. Each accepted instance state
is at most 64 KiB encoded. Persistent state excludes handles to pending host
requests, functions, and VM references. State bytes validate before migration
or commit. Canonical serialization follows [contracts](contracts.md).

## Instruction encoding and semantics

An instruction starts with a four-byte word: opcode `u8` and operand bytes A/B/C.
V1 reserves A/B/C as zero; opcode numbers below are hexadecimal. A listed index or branch offset adds one `u32` or
`i32` immediate word; `CONST` references its pool rather than embedding a double.
The VM is a typed operand-stack machine. All branch offsets are signed byte
offsets from the following instruction and must land on an instruction boundary
within the same function.

| Opcodes | Family and effect | Base fuel |
| --- | --- | --- |
| `00–05` | NOP, CONST(index), TRUE, FALSE, NULL, DROP | 1 |
| `10–13` | LOCAL_GET(index), LOCAL_SET(index), STATE_GET(field), STATE_SET(field) | 1 |
| `20–27` | ADD, SUB, MUL, DIV, REM, NEG, MIN, MAX; typed finite numeric operations | 2 |
| `28–2f` | EQ, NE, LT, LE, GT, GE, NOT, SELECT; typed comparison/control values | 2 |
| `30–33` | JUMP(offset), JUMP_IF(offset), CALL(function), RETURN | 1, 1, 5, 1 |
| `40–45` | ARRAY_NEW(type), ARRAY_GET, ARRAY_SET, RECORD_NEW(type), FIELD_GET(field), FIELD_SET(field) | 4 |
| `50–53` | STRING_CONCAT, STRING_SLICE, STRING_LENGTH, VALUE_EQUAL(type) | 4 |
| `60–63` | HOST_QUERY(import), PROPOSE_ACTION(import), EMIT_EVENT(import), RANDOM | 64, 64, 64, 4 |
| `70–72` | MATH(import), TICK, FAIL(code) | 64, 1, 1 |

Indexes are immediate words except stack-supplied array indexes. Conditional
jumps pop a boolean; SELECT pops a boolean and two identically typed values.
CALL pops declared arguments and pushes its declared result; RETURN requires
that exact result and no extra operands. Mutation instructions affect only an
invocation's private state copy and temporary values. Array accesses are bounds
checked; record accesses require the declared field. ARRAY_NEW pops a length
and initial value; RECORD_NEW consumes fields in schema order. GET consumes
the collection/index or record and returns its value; SET consumes the target,
index where applicable, and value, then returns the modified collection/record.
LOCAL_SET and STATE_SET consume a value without returning it. Collections use
value semantics; bounded copy-on-write is permitted. MATH supports only abs,
sqrt, floor, ceil, sin, cos, and atan2 in v1, with manifest-declared signatures.
TICK returns an exact numeric tick; this profile stops before tick 2^53.
String operations add one
fuel per processed UTF-8 byte; collection allocation/copy/comparison adds one per
element and one per 16 allocated bytes. Host calls additionally charge eight
fuel per returned query row and one per 16 input/output bytes. Host calls
reserve their declared maximum charge before execution, then settle actual
bounded usage. A failed reservation performs no work or effect. Unknown
opcodes, imports, or indices fail verification.

## Verification and limits

The verifier decodes every instruction, validates section/count arithmetic and
constants, rejects unreachable malformed bytes, and performs control-flow type
analysis. Branch merges require identical stack types and initialized locals;
every path respects declared stack bounds. Function signatures match imports
and calls. Recursion is permitted only under the call-depth budget. The verifier
does not assume loops terminate; metering bounds execution. Host queries have
bounded work and result counts, not just nominal instruction charges. Read-only
queries operate on a capability-filtered completed-tick snapshot in the VM's
own memory. Requests requiring the live kernel become queued host requests and
return opaque handles; later results arrive as events. No query introduces
per-object JavaScript/Wasm calls or shared mutable kernel pointers.

The default profile permits 100,000 fuel per invocation, 256 total stack values,
32 call frames, 256 locals per function, 256 KiB temporary heap,
128 host queries returning at most
32 rows each, 64 proposed actions, and 64 emitted events. A module permits
4,096 functions, 1,024 types, 16,384 constants, 256 KiB constant bytes, and
1 MiB total bytecode. Desktop aggregate ceilings are 2 million fuel and
1,024 actions/events per tick, 8 MiB accepted state, and 4 MiB loaded bytecode;
mobile uses 500,000 fuel, 512 actions/events, 2 MiB state, and 2 MiB bytecode.
Each world also has a fair per-instance scheduling quota. Limits are ceilings,
not promises that every instance can consume them simultaneously.

Fuel exhaustion, heap overflow, invalid output, or a missed wall deadline
discards the invocation's entire state/action proposal, including RNG advancement.
No host effect executes
during guest evaluation. A later validated broker commit owns external effects.
The external watchdog and stale-result rules in [scripting](scripting.md) remain
mandatory. Defaults are initial calibration targets; changing a qualified profile
changes its signed identity, and opcode semantics require a format version.

## Conformance

**VM-01:** golden binaries cover every opcode, branch merge, numeric edge,
Unicode operation, and exact fuel boundary. **VM-02:** malformed modules,
recursion, expensive strings, forged imports, and allocation bombs leave the
accepted state unchanged. **VM-03:** browser-Wasm and native authorities execute
identical supported bytecode/input vectors to identical state/action bytes.
Major changes require explicit state and module migration; minor versions cannot
reinterpret existing opcodes. Source maps and diagnostics do not participate in
execution semantics but remain content-addressed module data.
