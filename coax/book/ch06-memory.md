# 6. Arrays, strings, memory and the context

> **Partly runs today.** Fixed arrays and views (6.1) and strings (6.2)
> run on `coax0` 0.3.0, and their examples are checked. Dynamic arrays,
> pointers, handles, allocators, the context and scratch memory (6.3 to
> 6.7) are *Design* (spec §6.2 and §11).

Coax has no garbage collector and no destructors. Memory is something you
manage, and the language's job is to make that management short to
write, visible to read, and checkable where it can be. This chapter is
the longest in Part II because memory is where systems languages are won
or lost.

## 6.1 Three kinds of array

| Form | What it is | When it's copied |
| --- | --- | --- |
| `[N] T` | `N` elements stored inline, as part of the value | The elements are copied |
| `[] T` | A **view**: a pointer and a count, borrowing someone else's elements | Only the pointer and count are copied |
| `[..] T` | A **dynamic array**: pointer, count, capacity and an allocator | Only the descriptor is copied |

```coax
corners: [4] v3;                 // 4 × 12 bytes, inside whatever holds it
visible: [] Entity_Id = ids[0..<count];   // a view of part of another array
pending: [..] Pulse;             // grows with array_add, freed with free
```

The first two forms run today, with your own element types. Dynamic
arrays are *Design*.

```coax
#import "Basic";

sum :: (values: [] int) -> int {
  total := 0;
  for v: values { total += v; }
  return total;
}

double_all :: (values: [] int) {
  for i: 0..<values.count { values[i] *= 2; }   // writes reach the caller's array
}

bump :: (copy: [4] int) -> int {
  local := copy;          // a fixed-array parameter is a copy
  local[0] += 100;
  return local[0];
}

main :: () {
  a := [4] int.[1, 2, 3, 4];
  b := a;                 // copies all four elements
  b[0] = 99;
  print("% % %\n", a, b, a.count);

  print("% %\n", sum(a[..]), sum(a[1..<3]));  // a view of all, a view of part
  double_all(a[2..<4]);
  print("%\n", a);

  print("% %\n", bump(a), a[0]);

  part := a[1..<4];
  inner := part[1..<3];   // a view of a view
  print("% % %\n", part, inner, inner.count);
}
```

```
[1, 2, 3, 4] [99, 2, 3, 4] 4
10 5
[1, 2, 6, 8]
101 1
[2, 6, 8] [6, 8] 2
```

- **A fixed array is a value.** `[4] int.[1, 2, 3, 4]` is a literal with
  its type written first, and it must give exactly four elements
  (`E0358`). Assigning it, passing it or returning it copies every
  element, like a struct. `.count` is its length, known while checking.
  `N` must be known while checking too: a literal, a constant, `size_of`,
  or arithmetic on them (`E0357`).
- **A view borrows.** `a[..]` is a view of all of `a`; `a[lo..<hi]` is a
  view of part of it, half-open like a `for` range. Slicing a view gives
  another view of the same storage. A view is two words, a pointer and a
  count, and that is all that's copied when you pass one.
- **Writes go through.** `double_all` changes the caller's array, because
  its view points at the caller's storage. A view parameter can't be
  reassigned, but its elements can. Passing `[] int` says "I may read or
  write these elements"; passing `[4] int` says "I get my own copy".
- **Loops walk arrays in place.** `for x: a` reads each element as the
  loop reaches it, so a write by the body to a later element is seen by
  a later iteration. The loop variable itself is a copy and can't be
  assigned.
- **Every index and slice is checked**, in debug and release builds.
  Chapter 7 shows the trap.

Arrays nest: `grid: [2] [3] u8;` is two rows of three bytes, and
`grid[1][2] = 9;` writes one. Strings and views of `u8` work the same
way (6.2).

### What a view doesn't do

What the shape doesn't tell you is **who owns the elements or how long
they live**. A view proves nothing about lifetime, and `coax0` has three
rules that follow from that:

- **A view needs a variable.** A view may write through, so it must
  point at a local array variable, not at a parameter or a temporary:

  ```
  s.coax:1:34: error[E0366]: a view needs an array variable it may write through, and this is a parameter
    f :: (a: [4] int) -> int { v := a[..]; return v[0]; } main :: () {}
                                     ^
    fix: copy it into a local variable first
  ```

  Copy the parameter into a local, as `bump` does, and view that.
- **A view doesn't keep its array alive.** Returning `a[..]` where `a` is
  a local compiles, and the view points at storage that is gone when the
  procedure returns. That is the same mistake it is in C, and `coax0`
  doesn't catch it. Return the array, or take a view parameter and
  write into it.
- **A view can't leave the compiler.** A `#run` result can't contain one
  (`E0409`, Chapter 9), because it would point into memory that exists
  only while compiling.

The design adds two more ways for a view to go stale:

- **Growth moves storage.** `array_add` on a dynamic array can reallocate.
  Every view and pointer into the old storage is now stale.
- **Resets end borrows.** Resetting an arena invalidates everything
  allocated from it, even though the bytes still look fine for a while.

A bounds check catches a bad index. It can't catch a good index into
freed memory.

## 6.2 Strings

A `string` is a view of bytes: a pointer and a count.

```coax
#import "Basic";

main :: () {
  word := "coaxial";
  print("% % % %\n", word.count, word[0], word[2..<4], type_name(word[0]));

  // Strings are immutable. To change text, copy its bytes into an array.
  buffer: [7] u8;
  for c, i: word { buffer[i] = c; }
  buffer[0] = 'C';
  text := cast(string) buffer[..];       // borrows buffer's bytes
  print("% %\n", text, text == "Coaxial");

  zeros := "a\0b";
  print("%\n", zeros.count);
}
```

```
7 99 ax u8
Coaxial true
3
```

- **Indexing gives a byte.** `word[0]` is the `u8` 99, `'c'`. Slicing
  gives a string: `word[2..<4]` is `"ax"`. Both are checked.
- **Strings are immutable.** `word[0] = 'C'` is `E0359`, with the fix
  "copy the bytes into a `[N] u8`". A string literal lives in read-only
  memory, and a string you were handed may be someone else's.
- **`cast(string) bytes[..]`** turns a `[] u8` view into a string without
  copying. It borrows, like any view: change `buffer` and `text`
  changes with it.

- **Copying a string copies the view**, not the characters. Substrings are
  free.
- **Finding the length is free.** There's no scan for a terminating zero.
- **A string is not a C `char *`.** It isn't guaranteed to end in a zero
  byte, and it can contain zero bytes (`"a\0b"` is three bytes long, and
  `coax0` handles it). Crossing into C passes the pointer and the count
  (Chapter 16).
- **Bytes are not characters.** Text is UTF-8 by convention, and indexing
  is by byte. Iterating Unicode code points or graphemes is a library
  operation, not `s[i]`.
- **Someone owns the bytes.** A string literal lives forever. A string
  cast from a buffer lives as long as the buffer. In the design, a string
  formatted into scratch memory lives until the scratch is reset, and a
  string from a file lives as long as its buffer.

## 6.3 Pointers (design)

```coax
value := 7;
p: *int = *value;      // prefix * takes an address
p.* = 9;               // postfix .* dereferences: value is now 9
q: *int? = null;       // only an optional pointer can be null
```

- `*T` is never null. `*T?` may be, and must be tested before use.
- `*x` takes an address, and `p.*` reads through a pointer. That matches
  current Jai. Older Jai code, including Forrest Smith's, writes `<<p`.
- Field access works through pointers too: `p.position` is fine whether
  `p` is a struct or a pointer to one.

A pointer can outlive what it points to. Coax doesn't stop that with a
borrow checker. It reduces how often you need a raw pointer at all.

## 6.4 Handles instead of long-lived pointers (design)

```coax
door: Handle(Entity);
if e := world.get(door) { e.open = true; }
```

A `Handle(T)` is a 32-bit index plus a generation. When a slot is
recycled, its generation changes, and a stale handle fails its check
instead of reading whatever moved in. The engine's ECS already works this
way (`ecs::Entity`). Anything that outlives a frame, gets saved, or
survives a hot reload should hold a handle, not an address.

## 6.5 Who owns what (design)

Every piece of storage in an engine has an owner and a moment when it
goes away. Coax wants both visible:

| Storage | Owner | Invalidated when |
| --- | --- | --- |
| Persistent world | The resident host | Never by code reload; versioned handles survive it |
| Level arena | The level | The level unloads: every borrow must end first |
| Thread scratch | One worker | Reset per task or frame; can't escape to another worker |
| Pool | The pool | A slot is recycled: its generation changes |
| Dynamic array | The container's owner | It grows (storage moves) or is freed |
| Foreign buffer | The foreign library | Its documented release call |

The value of an arena is as much its **shared lifetime** as its speed.
Everything in a level arena dies together, so nothing in it needs freeing
one by one. The danger is a pointer that escapes the arena and is used
after the reset. The arena can't repair that pointer, so the design keeps
lifetimes visible and debug builds poison reset memory.

Allocation is always explicit: `New`, `NewArray`, `array_add`, `free` and
`destroy`. `destroy` releases only what its contract says it owns.
Freeing a struct does not recursively free what its fields point to.

## 6.6 The context (design)

Ordinary Coax procedures receive an implicit **context**: the current
allocator, a scratch arena and a logger. Libraries allocate through it,
so the caller decides where their memory comes from without every
function taking an allocator parameter:

```coax
build_level :: (desc: Level_Desc) -> Level { ... }   // allocates through context

push_context .{allocator = level_arena} {
  level := build_level(desc);    // everything it allocates lands in the level arena
}
```

`push_context` changes the policy for a block and everything it calls,
then restores the old one. This is the idea from Jai (and Odin) we're
most sure about. It lets you put a subsystem under an arena, a tracking
allocator or a different logger without changing a single signature.

What it costs, and what Coax adds:

- **A signature shows less.** A function that takes no allocator can
  still allocate. So Coax tracks allocation as an **effect** in every
  procedure's summary, transitively, even though the context is passed
  implicitly. A job, a shader or a no-allocation section can't call
  something that allocates, and the compiler says which call does.
- **It stops at the C boundary.** `#c_call` and `#no_context` procedures
  don't take or pass a context. Changing the Coax context doesn't
  redirect a C library's `malloc`.
- **Its layout is private.** Separately built modules talk through C entry
  points and opaque handles, never by sharing a context struct.

## 6.7 Scratch memory (design)

The scratch arena in the context is for memory that only has to last a
moment: a formatted string, a temporary list, a sort buffer.

```coax
names := format_list(entities, allocator = context.scratch);
draw_label(names);
// ... once per frame, the frame loop calls reset_scratch()
```

Allocating from scratch is a pointer bump, and freeing it is one reset
for everything. Each worker thread has its own, so scratch allocation
never needs a lock. In debug builds, memory is poisoned on reset, and a
scratch pointer stored somewhere persistent is reported at the reset that
invalidates it.

## 6.8 What Coax does not promise

Coax does not promise general memory safety. Pointers, views, foreign
calls and shared mutable data can all be misused, and no allocator design
changes that. What Coax provides instead:

- zero or default initialization, unless you write `---`;
- bounds and cast checks in every build, and overflow checks in
  development builds (these run today);
- handle checks in development builds;
- generation-checked handles for anything long-lived;
- explicit owners and reset points;
- allocation as a checked effect;
- poisoning and allocation provenance to find stale views fast.

That is how game engines stay correct in C++ today. The difference is
that in Coax these are the defaults.
