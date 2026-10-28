# The protocol boundary

OpenAgents surfaces talk to each other in a public protocol: protobuf
messages for agents, payments, decisions, the HUD and the Verse, plus
Nostr events. The schemas for those messages are `.proto` files in the
repository's root `proto/` folder, and they are the contract. Anything
that reads or writes those messages, in any language, has to agree with
them.

Coax code sits on that boundary all the time. The iOS home screen reads
its configuration as a protobuf message, and when you tap Start it
writes one. In this chapter we'll learn how Coax does that without
anybody writing a parser by hand: `#import_proto` reads a compiled
schema and generates checked Coax types and codecs for every message in
it. We'll build a message, look at its bytes, read it back, learn the
one rule about buffers that matters most, and then write an admission
function that refuses bad input the way the real home screen does.

> Note: Everything in this chapter runs on `coax0` 0.10.0. The importer
> handles a defined subset of proto3, listed near the end of the
> chapter, and the version table at the end describes rules rather than
> features. The full contract is in `coax/proto.md`.

## Where the schemas come from

The `.proto` files themselves are text. The compiler doesn't read them
directly. Instead, the repository keeps compiled descriptor sets in
`proto/descriptors/`, one `.pb` file per consumer profile, each holding
the profile's schemas plus everything they import. The script
`scripts/proto.sh` regenerates and checks those files with a pinned
version of `protoc`. You only need `protoc` when a schema changes;
ordinary Coax builds read the checked-in `.pb` files and need no
protobuf tools, no Python and no helper programs.

The iOS home uses the `ios-home` profile. Its main message is in
`proto/openagents/metareal/home/v1alpha1/home.proto`:

```protobuf
message HomeConfiguration {
  option (openagents.options.v1alpha1.data_class) = DATA_CLASS_DOCUMENT;
  option (openagents.options.v1alpha1.max_message_bytes) = 1024;
  option (openagents.options.v1alpha1.contract) = "openagents.metareal.home.v1alpha1";
  optional uint32 schema_version = 1 [(openagents.options.v1alpha1.required_value) = true];
  optional uint32 seed = 2 [(openagents.options.v1alpha1.required_value) = true];
  optional uint32 mote_count = 3 [(openagents.options.v1alpha1.required_value) = true];
  optional uint32 satellite_count = 4 [(openagents.options.v1alpha1.required_value) = true];
  openagents.engine.v1.HudAction start = 5 [(openagents.options.v1alpha1.required_value) = true];
}
```

*Listing 15-1: The home screen's configuration message*

The `start` field is a `HudAction`, a button, from the shared engine
schema `proto/openagents/engine/v1/hud.proto`:

```protobuf
message HudAction { string id = 1; string label = 2; bool enabled = 3; bool pressed = 4; bool shown = 5; }
```

The `openagents.options` annotations are OpenAgents' own. They attach
limits to the schema, such as `max_message_bytes` and `required_value`,
and the generated codecs enforce them. Keep an eye on `required_value`;
we'll run into it shortly.

## Building a message

Let's start with the button on its own. We'll build a `HudAction`,
encode it, and print the bytes.

Filename: ch15_start_button.coax

```jai
#import_proto "../../../proto/descriptors/ios-home.pb" as Home;
#import "Basic";

main :: () {
  first: [256] u8;
  second: [256] u8;
  output: [256] u8;

  start := Home_HudAction_new();
  start = Home_HudAction_set_id(start, "home.start", first[..]);
  start = Home_HudAction_set_label(start, "Start", second[..]);
  start = Home_HudAction_set_enabled(start, true, first[..]);
  start = Home_HudAction_set_shown(start, true, second[..]);

  encoded := Home_HudAction_encode(start, output[..]);
  print("error % , % bytes:", encoded.error, encoded.wire.count);
  for b: encoded.wire { print(" %", b); }
  print("\n");
}
```

*Listing 15-2: Building a `HudAction` and printing its wire bytes*

```
$ coax run ch15_start_button.coax
error 0 , 23 bytes: 10 10 104 111 109 101 46 115 116 97 114 116 18 5 83 116 97 114 116 24 1 40 1
```

Let's go through it.

The first line is the import. The path is relative to the file doing
the importing; this example lives in `coax/book/examples/`, so three
`..` steps reach the repository root. `as Home` is the prefix for every
generated name, and it's required, so two imported profiles can never
collide. A file may import up to eight profiles.

The import happens before ordinary parsing and checking. The compiler
reads the descriptor, validates it, and generates Coax declarations
that are checked like any code you wrote yourself. Every message `M`
in the profile becomes a type `Home_M` with procedures named
`Home_M_new`, `Home_M_set_FIELD`, `Home_M_get_FIELD`, `Home_M_encode`
and so on.

Next come three buffers. The generated codecs never allocate. A message
value is a view (Chapter 6) of wire bytes that live in storage you
provide, plus an error code:

```jai
Home_HudAction :: struct {wire:[] u8;error:u32;}
```

`Home_HudAction_new()` returns an empty message: no bytes, no error.
Each setter takes the message, the new value and an output buffer,
writes the edited message into the output, and returns a view of it.
That's why we alternate between `first` and `second`: each edit reads
the message from one buffer and writes into the other. We'll see what
happens if you get this wrong in a moment.

`Home_HudAction_encode` checks that the message is complete and valid
and copies its bytes into `output`. The result is a
`Home__pb_View`, which has a `wire` view and an `error`. An `error` of 0
means success. We'll meet the other codes as we go; they're numbered
the same for every profile.

### Reading the bytes

It's worth decoding those 23 bytes by hand once, because it makes the
rest of the chapter easier to follow. Protobuf writes each field as a
key followed by a value. The key is the field number times 8, plus a
wire type: 0 for a varint (a variable-length integer), 2 for something
with a length.

- `10` is field 1 (1 × 8 + 2), with a length: the `id`. `10` more bytes
  follow, `104 111 109 101 46 115 116 97 114 116`, which spell
  `home.start` in ASCII.
- `18` is field 2 (2 × 8 + 2): the `label`, `5` bytes, `Start`.
- `24` is field 3 (3 × 8 + 0), a varint: `enabled`, value `1`.
- `40` is field 5 (5 × 8 + 0): `shown`, value `1`.

There's no field 4. We never set `pressed`, so it isn't on the wire at
all.

### Seeing what was generated

To read the generated declarations, or save them to a file, use
`coax proto` from the repository root:

```
$ coax proto proto/descriptors/ios-home.pb --prefix Home -o /tmp/home-codecs.coax
$ wc -l /tmp/home-codecs.coax
    4769 /tmp/home-codecs.coax
```

That's every message in the profile, 27 of them, including the agent
and HUD messages `hud.proto` imports, plus a shared wire core. The first
line records where it came from:

```
// coax-proto-v4 generator=coax0-coax-v1 compiler=coax0-0.10.0 importer_sha256=3a9a7e61... fingerprint_sha256=94142391... descriptor_sha256=9ced2888... runtime_sha256=b0ceb1e5... prefix=Home
```

The hashes identify the descriptor, the importer that generated the
code and the runtime it embeds, so you can tell whether two builds used
the same inputs. They're fingerprints, not signatures; they don't make
anything trustworthy on their own. The same line appears at the top of
the C that `coax c` produces for any file with an import.

The generated code is plain Coax, and you can read any of it. Here's
the getter for `id`:

```jai
Home_HudAction_get_id :: (msg:Home_HudAction)->Home__pb_String {
checked:=Home_HudAction_decode(msg.wire);if msg.error!=0 || checked.error!=0 {return Home__pb_String.{error=msg.error|checked.error};}
pos:=0;while pos<msg.wire.count {f:=Home__pb_scan(msg.wire,pos);pos=f.next;if f.number==1 {return Home__pb_String.{value=cast(string) f.payload,present=true};}}
return Home__pb_String.{};}
```

It validates the whole message before reading anything, then scans the
fields for number 1. That re-validation on every read is a deliberate
trade: a view can never hand you a field from malformed bytes, at the
cost of a pass over the message per read. For messages of a few hundred
bytes, like these, that cost is tiny.

## Reading a message back

Now let's build the whole configuration, read it back the way a
receiver would, and look at the difference between a field that's
missing and a field that's zero.

Filename: ch15_home_config.coax

```jai
#import_proto "../../../proto/descriptors/ios-home.pb" as Home;
#import "Basic";

main :: () {
  // The start button gets its own two buffers.
  s1: [64] u8;
  s2: [64] u8;
  start := Home_HudAction_new();
  start = Home_HudAction_set_id(start, "home.start", s1[..]);
  start = Home_HudAction_set_label(start, "Start", s2[..]);
  start = Home_HudAction_set_enabled(start, true, s1[..]);
  start = Home_HudAction_set_shown(start, true, s2[..]);

  // The configuration alternates between two more.
  a: [256] u8;
  b: [256] u8;
  config := Home_HomeConfiguration_new();
  config = Home_HomeConfiguration_set_schema_version(config, 1, a[..]);
  config = Home_HomeConfiguration_set_seed(config, 7, b[..]);

  out: [256] u8;
  early := Home_HomeConfiguration_encode(config, out[..]);
  print("half built: error %\n", early.error);

  config = Home_HomeConfiguration_set_mote_count(config, 0, a[..]);
  config = Home_HomeConfiguration_set_satellite_count(config, 3, b[..]);
  config = Home_HomeConfiguration_set_start(config, start, a[..]);
  encoded := Home_HomeConfiguration_encode(config, out[..]);
  print("complete: error %, % bytes\n", encoded.error, encoded.wire.count);

  // Read it back the way a receiver would.
  read := Home_HomeConfiguration_decode(encoded.wire);
  seed := Home_HomeConfiguration_get_seed(read);
  motes := Home_HomeConfiguration_get_mote_count(read);
  print("seed % (present %)\n", seed.value, seed.present);
  print("motes % (present %)\n", motes.value, motes.present);

  button := Home_HomeConfiguration_get_start(read).value;
  label := Home_HudAction_get_label(button);
  pressed := Home_HudAction_get_pressed(button);
  print("label \"%\"\n", label.value);
  print("pressed % (present %)\n", pressed.value, pressed.present);
}
```

*Listing 15-3: Building a configuration, then reading it back*

```
$ coax run ch15_home_config.coax
half built: error 5
complete: error 0, 33 bytes
seed 7 (present true)
motes 0 (present true)
label "Start"
pressed false (present false)
```

The first line is the `required_value` annotation at work. After
setting only the version and the seed, we tried to encode, and got
error 5, `REQUIRED`: three required fields were still missing. Setters
let you build a message in any order; the completeness check waits until
you encode or decode.

`Home_HomeConfiguration_decode` takes bytes and returns a message view
after validating the whole message, including the nested button. Each
getter returns a small struct with three fields: `value`, `present` and
`error`. `present` is how you tell "the sender said zero" from "the
sender said nothing". Look at the motes: we set the count to 0, and
because `mote_count` is declared `optional`, the zero was written to
the wire, so it comes back `present true`.

`pressed` is different. It's a plain proto3 `bool`, which protobuf
doesn't write when it's false, and we never set it. Its getter reports
`false` and `present false`. For a field like this, `present` tells you
only whether the field appeared on the wire. The real home screen uses
exactly that to refuse any configuration whose button arrives already
pressed, as we'll see.

Getting the nested button works the same way:
`Home_HomeConfiguration_get_start` returns a `Home_HudAction_Option`
whose `value` is a `Home_HudAction` view into the same bytes. Nothing
is copied.

### The buffer rule

Here's the mistake we promised. Suppose we skip the separate buffers for
the button and build it in `a` and `b` as well:

```jai
  a: [256] u8;
  b: [256] u8;
  start := Home_HudAction_new();
  start = Home_HudAction_set_id(start, "home.start", a[..]);
  start = Home_HudAction_set_label(start, "Start", b[..]);

  config := Home_HomeConfiguration_new();
  config = Home_HomeConfiguration_set_schema_version(config, 1, a[..]);
  config = Home_HomeConfiguration_set_seed(config, 7, b[..]);
  // ... the other fields, then set_start and encode as before
```

This compiles, and the encode fails with error 1, `WIRE`, meaning
malformed bytes. The button's view points into `b`. Setting the seed
wrote the configuration into `b`, over the button's bytes. By the time
`set_start` reads `start`, the view holds pieces of a configuration.

This time the damage happened to produce invalid bytes, so the codec
caught it. It could just as easily have produced a valid button with the
wrong label. So the rule is: a message view borrows its buffer, and you
must not write to that buffer while any view of it is still in use. The
codec can check the case it can see. Pass the same buffer as input and
output,

```jai
  config = Home_HomeConfiguration_set_schema_version(config, 1, a[..]);
  config = Home_HomeConfiguration_set_seed(config, 7, a[..]);
```

and the setter refuses before writing anything, with error 6, `ALIAS`.
What it can't see is an earlier view you're still holding. Give each
message under construction its own pair of buffers, as Listing 15-3
does, and keep the final buffer alive until every reader is finished.

## Mistakes the compiler catches

Because the codecs are ordinary generated Coax, ordinary mistakes get
ordinary errors. Misspell a field and the procedure doesn't exist:

```
$ coax check ch15_home_config.coax
ch15_home_config.coax:34:12: error[E0315]: procedure 'Home_HomeConfiguration_get_motes' is not declared
    motes := Home_HomeConfiguration_get_motes(read);
             ^
1 error
```

Give a field the wrong type and the schema's type shows up in the error:

```
ch15_home_config.coax:19:52: error[E0317]: argument 2 of 'Home_HomeConfiguration_set_seed' is u32, given string
    config = Home_HomeConfiguration_set_seed(config, "seven", b[..]);
                                                     ^
1 error
```

A wrong descriptor path is reported at the import, with the path the
compiler actually tried:

```
examples/ch15_home_config.coax:1:1: error[E0502]: cannot read proto descriptor: /Users/christopherdavid/work/openagents/proto/descriptors/ios-hom.pb
  #import_proto "../../../proto/descriptors/ios-hom.pb" as Home;
  ^
1 error
```

And importing two profiles under the same prefix is `E0501`. Like every
code, `coax explain` gives the longer story.

## Admitting input

A decoder answers one question: are these bytes a well-formed message
of this type? The program still has to decide whether it accepts what
the message says. Those are two layers, and Coax keeps them apart. The
codec owns the wire: lengths, varints, nesting, required fields,
duplicates, UTF-8 and the limits in the schema. Your code owns the
policy: which schema version, which ranges, which buttons.

Let's write that policy as a function and throw some bad input at it.

Filename: ch15_admit.coax

```jai
#import_proto "../../../proto/descriptors/ios-home.pb" as Home;
#import "Basic";

// The screen's own rules, applied after the codec has checked the wire.
admit :: (bytes: [] u8) -> string {
  config := Home_HomeConfiguration_decode_minimal(bytes);
  if config.error == Home__pb_WIRE { return "refused: malformed wire"; }
  if config.error == Home__pb_DUPLICATE { return "refused: a field appears twice"; }
  if config.error == Home__pb_REQUIRED { return "refused: a required field is missing"; }
  if config.error != 0 { return "refused: other codec error"; }
  if !Home_HomeConfiguration_unknown(config, 0).done { return "refused: unknown field"; }

  version := Home_HomeConfiguration_get_schema_version(config);
  motes := Home_HomeConfiguration_get_mote_count(config);
  if version.value != 1 { return "refused: not schema 1"; }
  if motes.value > 4096 { return "refused: too many motes"; }
  return "admitted";
}

// Append one varint field (wire type 0) to `buffer` at `start`.
put_varint :: (buffer: [] u8, start: int, field: int, value: int) -> int {
  at := start;
  buffer[at] = cast(u8) (field * 8);
  at += 1;
  v := value;
  while v >= 128 {
    buffer[at] = cast(u8) (v % 128 + 128);
    at += 1;
    v = v / 128;
  }
  buffer[at] = cast(u8) v;
  return at + 1;
}

main :: () {
  s1: [64] u8;
  s2: [64] u8;
  start := Home_HudAction_new();
  start = Home_HudAction_set_id(start, "home.start", s1[..]);
  start = Home_HudAction_set_label(start, "Start", s2[..]);

  a: [256] u8;
  b: [256] u8;
  config := Home_HomeConfiguration_new();
  config = Home_HomeConfiguration_set_schema_version(config, 1, a[..]);
  config = Home_HomeConfiguration_set_seed(config, 7, b[..]);
  config = Home_HomeConfiguration_set_mote_count(config, 900, a[..]);
  config = Home_HomeConfiguration_set_satellite_count(config, 3, b[..]);
  config = Home_HomeConfiguration_set_start(config, start, a[..]);
  out: [256] u8;
  good := Home_HomeConfiguration_encode(config, out[..]).wire;
  print("good bytes:     %\n", admit(good));

  print("cut short:      %\n", admit(good[0..<good.count - 3]));

  // Copy the good bytes into a scratch buffer we can extend.
  extra: [256] u8;
  for value, i: good { extra[i] = value; }
  n := good.count;

  end := put_varint(extra[..], n, 2, 9);
  print("seed twice:     %\n", admit(extra[0..<end]));

  end = put_varint(extra[..], n, 9, 1);
  print("field 9:        %\n", admit(extra[0..<end]));

  big := Home_HomeConfiguration_set_mote_count(config, 5000, b[..]);
  print("5000 motes:     %\n", admit(Home_HomeConfiguration_encode(big, out[..]).wire));

  print("no bytes:       %\n", admit(good[0..<0]));
}
```

*Listing 15-4: An admission policy over a generated codec*

```
$ coax run ch15_admit.coax
good bytes:     admitted
cut short:      refused: malformed wire
seed twice:     refused: a field appears twice
field 9:        refused: unknown field
5000 motes:     refused: too many motes
no bytes:       refused: a required field is missing
```

`admit` decodes with `decode_minimal` rather than `decode`. The minimal
variant also insists that every varint and key uses its shortest
encoding and every `bool` is exactly 0 or 1. Protobuf allows longer
encodings of the same number, and ordinary `decode` accepts them; for a
configuration file that we generate ourselves, there's no reason to, so
we refuse anything unusual.

The error codes are constants the import generates, such as
`Home__pb_WIRE` and `Home__pb_DUPLICATE`. After the codec is satisfied,
`Home_HomeConfiguration_unknown(config, 0)` asks for the first field the
schema doesn't know. If the answer isn't `done`, there is one, and our
policy refuses it. Then come the policy's own rules: schema 1 only, at
most 4096 motes.

`put_varint` lets us make bad input by hand. It writes a key and a
varint, seven bits at a time with the high bit meaning "more follows".
Coax doesn't let us assign to the parameter `start`, so it copies it
into `at` first.

Now the results, one by one.

Cutting three bytes off the end leaves the last field's length pointing
past the end of the input. That's malformed wire, error 1.

Appending a second `seed` gives a message that standard protobuf would
accept: when a singular field appears twice, the last value wins. Coax's
reader refuses it with `DUPLICATE`. If two parts of a system could
disagree about which `seed` counts, the message is ambiguous, and
refusing it is safer than picking one.

A field numbered 9 decodes fine. The codec doesn't reject unknown
fields. It keeps them, and if you edit the message and encode it again
they come through byte for byte. That's how a newer sender can add a
field without breaking an older reader. Our policy, though, is for a
closed resource at schema 1, so it refuses anything it doesn't
understand.

5000 motes is a perfectly good `uint32`, so the codec accepts it and the
policy says no.

And no bytes at all is a legal empty message, so the wire is fine, but
every required field is missing.

### Limits come before memory

Every decode checks sizes before it touches anything else: by default 1
MiB per message, 64 levels of nesting and 524,288 field visits in one
validation, tightened by any schema limit such as `HomeConfiguration`'s
1024 bytes. Nothing is ever allocated from the input, because nothing is
allocated at all; the caller's buffers set the ceiling. So hostile input
can't make the program use more memory than the buffers it already has.

And a message that decodes successfully grants nothing. It's data. A
`HudAction` with `id` `"home.start"` doesn't authorize anything to
start. Permissions come from somewhere else.

## In the real world: the home screen

`verse/home.coax` uses the same generated codecs for both directions.
At startup, the host hands it the configuration bytes, and Coax copies
them into a bounded scratch buffer and applies the screen's admission
policy:

```jai
#import_proto "../proto/descriptors/ios-home.pb" as Home;
// ...
  config:=Home_HomeConfiguration_decode_minimal(scratch[0..<count]);
  if config.error!=0 {return false;}
  if !Home_HomeConfiguration_unknown(config,0).done {return false;}
  version:=Home_HomeConfiguration_get_schema_version(config);
  seed:=Home_HomeConfiguration_get_seed(config);
  motes:=Home_HomeConfiguration_get_mote_count(config);
  satellites:=Home_HomeConfiguration_get_satellite_count(config);
  start:=Home_HomeConfiguration_get_start(config);
  if version.value!=1 || seed.value==0 || motes.value>4096 || satellites.value>8 {return false;}
  action:=start.value;
  if !Home_HudAction_unknown(action,0).done {return false;}
  // ...
  // The old resource subset did not allow the transient pressed field at all.
  if Home_HudAction_get_pressed(action).present {return false;}
  if id.value!="home.start" || !enabled.value || !shown.value {return false;}
  if label.value.count<1 || label.value.count>32 {return false;}
```

*Listing 15-5: The home screen's admission, from `verse/home.coax`*

That's our Listing 15-4 with a stricter policy: a nonzero seed, at most
eight satellites, exactly the `home.start` button, enabled, shown, never
pre-pressed, with a label of 1 to 32 printable ASCII bytes. The comment
in the source puts it plainly: "The generated codec owns wire
validation; this is the screen's closed schema-1 admission policy."

When you tap Start, the home screen writes a `HudInvocation` the same
way we built our button, alternating two 512-byte buffers:

```jai
  message:=Home_HudInvocation_new();
  message=Home_HudInvocation_set_schema(message,1,first[..]);
  message=Home_HudInvocation_set_world(message,"home",second[..]);
  message=Home_HudInvocation_set_action_id(message,"home.start",first[..]);
  message=Home_HudInvocation_set_request_id(message,cast(string)request[0..<at],second[..]);
  message=Home_HudInvocation_set_revision(message,cast(u64)count,first[..]);
  encoded:=Home_HudInvocation_encode(message,output[..]);
  if encoded.error!=0 {return false;}
```

"Only request-ID formatting is local; every protobuf byte is generated,"
says the comment above it. The Swift side of the app contains no
protobuf code at all.

## What the importer supports

The importer handles proto3 data messages: every scalar type, nested
messages, open enums, repeated fields (packed or not), explicit
`optional` presence and `oneof`. Enums become constants like
`Home_Enum_MEMBER`, and enum fields keep unknown values as `s32`, so a
newer sender's enum member isn't lost. Repeated fields get iterators,
and oneofs get a `case` accessor that says which member is set.

It deliberately rejects proto2, editions, groups, `map` fields,
extensions and unknown custom options, with an error at the import.
Services aren't generated. These are limits of the first importer, not
judgments about the features, and the full list with every bound is in
`coax/proto.md`.

The importer itself is written in Coax: the descriptor parser, the
validator and the source generator are `coax/lib/proto/importer.coax`,
with checks that run, and mutation tests that must all be killed, every
time the compiler is built. [Chapter 20](ch20-inside-coax0.md) explains
how that bootstrap works.

## The `.proto` files stay in charge

There is exactly one schema, and it isn't in Coax. Coax code consumes
the `.proto` contract. If Coax work needs a new field, the change is
made to the `.proto` file and goes through the owning repository's
versioning and compatibility review like any other change, and only
then do the descriptors and generated codecs follow. A Coax file is
never a second schema that someone can quietly edit.

## Signed bytes are sacred

Nostr events and other signed records are verified over their original
bytes. Coax never re-serializes a signed record to check its signature
or to hash it. Protobuf's serialization isn't canonical: the same
message can be encoded in more than one valid way. Coax's encoder
preserves the original bytes and order of every field you didn't
touch, and appends edited fields at the end, which is useful but still
not a canonical form. So the original JSON or content-addressed bytes
keep their own representation and their own validation, and a decoded
event is not a verified event.

## Four kinds of version

When a program crosses the protocol boundary, "version" can mean four
different things:

| Axis | What changes | Example |
| --- | --- | --- |
| Protocol version | The public contract | A field added to `HudAction` |
| Native layout version | A struct's memory layout | A field reordered for cache |
| Save version | What's on disk | A world save written last month |
| Hot-reload version | Live state in a running host | A struct changed while the app runs |

Coax keeps these apart. The configuration's `schema_version` is the
first kind; nothing about it says how the home screen lays out its
nodes in memory. Renaming a field never implies a safe migration on any
of the four axes, and pointers, `Any` values and live resources are
never wire data. Notice that the generated message types contain only a
view and an error code, never an embedded native struct, so a change to
an engine struct can't leak onto the wire.

## Summary

`#import_proto` turns a checked-in descriptor set into Coax types and
codecs, with no protobuf library and no tools at build time. Messages
are views of bytes in buffers you provide; setters write into a
separate output buffer, so you alternate buffers and never overwrite one
a live view points into. Getters report `present` alongside `value`,
and encoding and decoding enforce the schema's required fields and
limits. On top of that codec layer you write the admission policy,
which is where the home screen refuses unknown fields, duplicates,
pre-pressed buttons and out-of-range counts. The `.proto` files remain
the single contract.

In the next chapter we'll look at how all these generated files, C
libraries, headers and shaders get built into the iOS app, and why hot
reload is further away than a fast compiler makes it look.

For a complete generated-codec, native-file and checked-request example,
see [Chapter 26](ch26-a-complete-boundary.md).
