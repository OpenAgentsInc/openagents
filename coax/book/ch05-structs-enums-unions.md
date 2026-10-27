# 5. Structs, enums and unions

> **Design.** `coax0` reserves `struct`, `enum`, `union` and `enum_flags`
> and refuses them for now. Structs come first in the next compiler stage
> (spec §20, stage 2). Syntax can change before it's built.

## 5.1 Structs

```coax
Room :: struct {
  name:     string;
  rate:     int;
  tunnels:  [..] RoomIndex;
  selected: RoomIndex = -1;    // a field default
}
```

A struct is a type, so it's declared like every other constant:
`name :: struct { fields }`. Each field is `name: type`, optionally with a
default.

**Initialization has meaning.** A new `Room` gets its declared defaults,
and zero for every field without one. So "default initialized" is the
precise rule, not "zeroed": `selected` starts at `-1`. As with locals,
`room: Room = ---;` opts out entirely.

**Layout is what you wrote.** Fields go in declaration order with each
field's natural alignment. The compiler doesn't reorder them to save
padding, because then a struct's layout would change when someone adds a
field, and a C or GPU consumer would silently break. When you want a
different layout, you ask for it with an explicit layout mode, and you
can always print what you got (Chapter 10).

**Copies are shallow.** Assigning a `Room` copies its fields: the string's
pointer and count, the dynamic array's pointer, count and capacity. It
does not copy the characters or the tunnel list. Two `Room`s can then
share one tunnel array. That is cheap and explicit, and it is also the
first thing to check when two objects mysteriously change together.

### Literals name their fields

```coax
start := Room.{name = "AA", rate = 0};
```

Named fields can't be misordered. Positional literals (`v3.{1, 2, 3}`)
are for small math types where the order is the meaning.

## 5.2 Composition with `using`

```coax
Body :: struct { position: v3; velocity: v3; }

Player :: struct {
  using body: Body;
  score: int;
}

p: Player;
p.position.x += 1;        // means p.body.position.x
```

`using` makes an embedded field's members reachable through the outer
struct. It's a naming convenience. It doesn't create a class hierarchy,
there's no virtual dispatch, and a `*Player` doesn't become a `*Body`. A
conversion between them has to be declared on its own with `#as`, and the
compiler checks it. Most engine code never needs either: it passes the
`Body` it cares about.

## 5.3 Enums

```coax
Outcome :: enum u8 {
  completed = 0;
  refused   = 1;
  failed    = 2;
  cancelled = 3;
  unknown   = 4;
}

Layer_Mask :: enum_flags u32 { terrain; props; characters; effects; }
```

- **Every enum names its underlying type.** `enum u8` is one byte
  everywhere.
- **Values that persist are written out.** Anything saved, sent over a
  network or stored in a registry has explicit numbers, so reordering the
  source can't change what old data means. Jai's `#specified` does this
  as an opt-in. In Coax it's required wherever the enum crosses a
  boundary.
- **Flags are a different form.** `enum_flags` gives each value its own
  bit, so `terrain | props` is meaningful.

## 5.4 Unions are tagged

```coax
Answer :: union {
  noul:   struct { noul: f64; };
  choice: struct { choice: u32; confidence: f64; };
  score:  struct { score: u32; confidence: f64; };
}

describe :: (a: Answer) {
  switch a {
    case .noul:   (n) { print("noul %\n", n.noul); }
    case .choice: (c) { print("choice % at %\n", c.choice, c.confidence); }
    case .score:  (s) { print("score %\n", s.score); }
  }
}
```

A Coax `union` records which alternative it holds, and `switch` on it
must handle every alternative (or say `case else`). Add an alternative
and every `switch` that forgot it fails to compile.

This differs from C and from Jai, where a plain `union` overlays storage
and the program must track the active member itself. Raw overlapping
storage has uses, such as reinterpreting bits, and if Coax adds it, it
will be a separate, visibly unsafe form with manual discriminator rules.
The safe form is the one you get by default.

## 5.5 A struct is not a file format

It's tempting to save a struct by writing its bytes. Don't. A struct's
memory holds:

- **pointers**, which mean nothing in another process;
- **padding**, which can hold anything;
- **a byte order and layout** for one target;
- **no version**, so next month's struct can't read this month's bytes.

Saves, network messages and protocol records go through explicit,
versioned encodings (Chapter 14). Reflection can generate those encoders,
but someone still has to decide what each field means on the wire. A
`*Entity` field might be saved as an entity ID, as an asset reference, or
not at all. The type can't decide that for you.

## 5.6 Deriving the obvious

```coax
Grid_Key :: struct #derive(eq, hash) { x, y: int; }

visited: Table(Grid_Key, int);
```

Forrest Smith's Advent of Code solutions in Jai write the same three
blocks for every key type: an `operator ==` comparing each field, a
`get_hash` combining each field's hash, and the table declaration that
passes the hash procedure in. `#derive(eq, hash, order, print, proto)`
generates those from the struct's fields, in declaration order. It's
library code over reflection (Chapter 9), not compiler magic, so you can
read what it generated.

A hand-written `==` must come with a matching hash before the type can be
a table key. Two values that compare equal but hash differently is a bug
you'd otherwise find one lost lookup at a time.
