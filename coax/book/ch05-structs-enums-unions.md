# 5. Structs, enums and unions

> **Partly runs today.** Structs (5.1) and enums (5.3) run on `coax0`
> 0.3.0, and their examples are checked. `using` (5.2), unions (5.4),
> `enum_flags` and `#derive` (5.6) are *Design*: `coax0` reserves
> `union`, `using` and `enum_flags` and refuses them. Syntax can change
> before they're built.

## 5.1 Structs

```jai
#import "Basic";

Room :: struct {
  name: string;
  rate: s32;
  selected: s32 = -1;      // a field default
}

main :: () {
  a: Room;                                 // defaults, and zero elsewhere
  b := Room.{name = "AA", rate = 13};      // name the fields you set
  print("%\n%\n", a, b);

  c := b;                                  // a copy, not a reference
  c.rate += 1;
  c.selected = 0;
  print("% % %\n", b.rate, c.rate, c.selected);
}
```

```
Room.{name = "", rate = 0, selected = -1}
Room.{name = "AA", rate = 13, selected = -1}
13 14 0
```

A struct is a type, so it's declared like every other constant:
`name :: struct { fields }`. Each field is `name: type`, optionally with a
default. Structs are declared at the top level, in any order, like
procedures.

**Initialization has meaning.** A new `Room` gets its declared defaults,
and zero for every field without one. So "default initialized" is the
precise rule, not "zeroed": `selected` starts at `-1`. As with locals,
`room: Room = ---;` opts out entirely, defaults included.

**Layout is what you wrote.** Fields go in declaration order with each
field's natural alignment. The compiler doesn't reorder them to save
padding, because then a struct's layout would change when someone adds a
field, and a C or GPU consumer would silently break. `coax layout` prints
what you got (Chapter 10). When you want a different layout, the design
lets you ask for it with an explicit layout mode.

**Structs are values.** Assigning a `Room`, passing one to a procedure or
returning one copies its fields. `c.rate += 1` above changes `c` and
leaves `b` alone. A field that is itself a view or a string copies only
its pointer and count, not the bytes behind it: copies are shallow. In
the design, a dynamic array field (`tunnels: [..] RoomIndex`) copies its
pointer, count and capacity the same way, so two `Room`s can share one
tunnel list. That is cheap and explicit, and it is also the first thing
to check when two objects mysteriously change together.

**Structs don't compare.** `a == b` on two structs is `E0311`. Equality
of a whole struct hides choices: is padding compared, are two strings
equal by pointer or by content, is `-0.0` equal to `0.0`? Compare the
fields you mean. `#derive(eq)` (5.6) is the planned way to say "all of
them".

A struct can't contain itself by value (`E0353`): its size would be
infinite.

### Literals name their fields

```jai
start := Room.{name = "AA", rate = 0};
```

A literal starts from the defaults and sets the fields it names, so
named fields can't be misordered and a new field doesn't break old
literals. A misspelled name is an error that lists the real ones:

```
s.coax:1:46: error[E0351]: struct 'P' has no field 'y'
  P :: struct { x: int; } main :: () { p := P.{y = 1}; }
                                               ^
  fix: its fields are: x
```

*Design:* positional literals (`v3.{1, 2, 3}`) for small math types
where the order is the meaning, and inferred literals (`.{x = 1}`)
where the type is known. Today the struct's name always comes first.

## 5.2 Composition with `using` (design)

```jai
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

```jai
#import "Basic";

Outcome :: enum u8 {
  completed = 0;
  refused   = 1;
  failed    = 2;
  cancelled = 3;
  unknown   = 4;
}

Mode :: enum u16 { idle; walking; running = 10; sprinting; }

main :: () {
  o := Outcome.refused;
  print("% % %\n", o, o == Outcome.refused, cast(u8) o);
  print("% % %\n", Mode.walking, Mode.sprinting, cast(u16) Mode.sprinting);
  print("% %\n", size_of(Outcome), size_of(Mode));

  code: u8 = 3;
  print("%\n", cast(Outcome) code);       // 3 is a member: cancelled
  code = 9;
  print("%\n", cast(Outcome) code);       // 9 is not: this traps
}
```

```
refused true 1
walking sprinting 11
1 2
cancelled
examples/outcomes.coax:22: value is not a member of enum Outcome
```

- **Every enum names its underlying type.** `enum u8` is one byte
  everywhere. Leaving it out is `E0219`, because the width is part of
  the layout, and a layout shouldn't depend on how many members happen
  to exist.
- **Members are named through the type**: `Outcome.refused`. A member
  without a value is the previous value plus one, starting at zero, so
  `Mode.sprinting` is `11`. Values must fit the type and be distinct
  (`E0363`).
- **An enum is not an integer.** It compares with `==` and `!=` against
  its own type only, and has no ordering. Getting the number takes
  `cast(u8) o`. Going the other way, `cast(Outcome) code` checks that the
  number names a member and stops the program if it doesn't, in every
  build. A bad value read from a file or a network is caught where it
  enters, not three calls later.
- **Enums print by name.** A log line says `refused`, not `1`.
- **A zeroed enum is the number zero.** `o: Outcome;` is `completed`
  because `completed = 0`. An enum with no zero member should be given
  an explicit value wherever it's declared, since `coax0` doesn't check
  the zero; it prints such a value as its type and number, `Name(0)`.
- **Values that persist are written out.** Anything saved, sent over a
  network or stored in a registry has explicit numbers, as `Outcome`
  does, so reordering the source can't change what old data means. Jai's
  `#specified` does this as an opt-in. In Coax it's meant to be required
  wherever the enum crosses a boundary; `coax0` doesn't enforce that yet.

*Design:* flags are a different form.

```jai
Layer_Mask :: enum_flags u32 { terrain; props; characters; effects; }
```

`enum_flags` gives each value its own bit, so `terrain | props` is
meaningful. A plain enum keeps `|` off, because the result of or-ing two
members usually names no member.

## 5.4 Unions are tagged (design)

```jai
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

## 5.6 Deriving the obvious (design)

```jai
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
