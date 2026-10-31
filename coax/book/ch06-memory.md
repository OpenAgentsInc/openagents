# 6. Arrays, strings, memory and the context

<!-- coax-release:current:start -->
Current compiler: **coax0 0.31.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
Every value in a program lives somewhere in memory, and somebody decides
when that memory is reused. In a garbage-collected language you rarely have
to think about it. Coax has no garbage collector and no destructors, so the
question "where is this, and how long will it be there?" is one you answer
yourself. In this chapter we'll learn the tools Coax gives you today for
working with memory: fixed arrays, views into them, strings, raw bytes, and
the bounds checks that guard every index. Then we'll look at what the design
adds for memory that has to outlive a procedure: dynamic arrays, pointers,
handles, allocators, the context and temporary storage.

Getting memory wrong is the most common way for a systems program to
misbehave without an error message. A wrong index reads someone else's
data. A reference to memory that has been reused reads whatever moved in.
Coax checks indexes and diagnoses some direct returns of local borrows, but
many lifetime mistakes still need manual review. Knowing exactly where that
line falls is the most useful thing this chapter can teach.

> Note: the current compiler has fixed arrays, views, strings and bounds checks,
> and every program in this chapter that prints something has its output
> checked. It has no heap at all: no pointers, no dynamic arrays, no
> allocators, no context, no scratch memory and no global variables. Those
> sections are marked planned, and the syntax in them can change. The
> book's copies of the programs are in `coax/book/examples/` with a `ch06_`
> prefix: `arrays.coax` below is `examples/ch06_arrays.coax`.

## Where values live today

Before any syntax, let's be precise about where a Coax value can be in
`coax0`. There are four places.

A *local variable* or *parameter* lives in its procedure's stack frame. The
frame is created when the procedure is called and reused as soon as it
returns. `coax0` compiles to C, and a Coax local is a C local, so this is
exactly C's stack.

A *constant* lives in the program's read-only data and exists for the whole
run. String literals are constants.

A *value returned from a procedure* is copied out to the caller before the
callee's frame goes away.

Memory the *host* owns, such as a buffer the Swift or C side of an app
allocated, can be handed to Coax as a view (more on views shortly). Coax
reads and writes it but never allocates or frees it.

There is no fifth place. You can't allocate a block that outlives the
procedure that asked for it, and there are no global variables:

```jai
glob.coax:1:1: error[E0212]: global variables are not supported by coax0 yet
  frame_count: int = 0;
  ^
1 error
```

That sounds limiting, and it is. It's also why the real Coax code in
Verse looks the way it does: state goes in and comes out of procedures as
values (`Player` in, `Player` out), and anything large, like a mesh or an
animation file, is owned by the native side and passed in as a view.

## Fixed arrays

A fixed array, `[N] T`, is `N` values of type `T` stored side by side, with
`N` known when the program is compiled.

Filename: arrays.coax

```jai
#import "Basic";

SLOTS :: 4;

main :: () {
  scores := [4] int.[10, 20, 30, 40];
  print("% has % elements, the first is %\n", scores, scores.count, scores[0]);

  copy := scores;               // all four elements are copied
  copy[0] = 99;
  print("scores % copy %\n", scores, copy);

  empty: [SLOTS] u8;            // zeroed, like every new value
  empty[SLOTS - 1] = 7;
  print("% % bytes\n", empty, size_of([SLOTS] u8));

  grid: [2] [3] u8;             // two rows of three
  grid[1][2] = 9;
  print("% %\n", grid, size_of([2] [3] u8));

  total := 0;
  for s: scores { total += s; }
  for s, i: scores { print("%:% ", i, s); }
  print("total %\n", total);
}
```

*Listing 6-1: Fixed arrays are values with a length known while compiling*

```shell
$ coax run arrays.coax
[10, 20, 30, 40] has 4 elements, the first is 10
scores [10, 20, 30, 40] copy [99, 20, 30, 40]
[0, 0, 0, 7] 4 bytes
[[0, 0, 0], [0, 0, 9]] 6
0:10 1:20 2:30 3:40 total 100
```

`[4] int.[10, 20, 30, 40]` is an array literal: the type first, then `.[`,
then the elements. It has to give exactly as many elements as the type says:

```jai
short.coax:2:13: error[E0358]: [5] int needs 5 elements, given 3
    scores := [5] int.[10, 20, 30];
              ^
1 error
```

A fixed array is a value, like a struct. `copy := scores` copies all four
elements, which is why changing `copy[0]` left `scores` alone. Passing a
fixed array to a procedure and returning one copy it too. Its storage is
inline: `size_of([SLOTS] u8)` is exactly 4, with no hidden header, and a
`[4] f32` field in a struct takes 16 bytes of that struct.

`.count` is the length. For a fixed array it's known while compiling, and so
is `N` itself. The length can be a literal, a constant like `SLOTS`, or
arithmetic on them, but not a runtime value:

```jai
size.coax:2:12: error[E0357]: an array length must be an integer known while checking: a literal, a constant, or arithmetic on them
    buffer: [n] u8;
             ^
1 error
```

A new array is zeroed (`empty` above), and `---` skips that, as it does for
any other variable.

Arrays nest. `[2] [3] u8` is an array of two elements, each of which is a
`[3] u8`, so `grid[1][2]` is the last byte of the second row. The six bytes
are stored row after row.

`for s: scores` walks the elements in order, and `for s, i: scores` also
gives each one's index. The loop variable is a copy of the element and can't
be assigned:

```jai
loopvar.coax:3:21: error[E0359]: a loop variable cannot be assigned
    for s: scores { s *= 2; }
                      ^
  fix: copy it into a local variable
1 error
```

To change elements in a loop, index the array: `for i: 0..<scores.count {
scores[i] *= 2; }`. The loop reads each element as it reaches it, so a write
the body makes to a later element is seen by a later iteration.

## Views

Copying a whole array every time we pass it to a procedure would be slow
for big arrays, and it would make it impossible for a procedure to fill in
an array for its caller. Coax's answer is the *view*, written `[] T`.

Here's an analogy that we'll keep using for the rest of the chapter. Think
of an array as a row of lockers. A view is a note that says "lockers 2
through 4". You can copy the note as often as you like and hand it to
anyone; copying it doesn't copy what's in the lockers. Anyone holding the
note can open those lockers and change what's inside. And the note doesn't
keep the lockers from being torn down. If they are, the note still says
"lockers 2 through 4", and following it leads somewhere you shouldn't be.

In memory, a view is two words: a pointer to its first element and a count.

```jai
a := [6] int.[1, 2, 3, 4, 5, 6];      middle := a[2..<5];

             a (48 bytes, in main's frame)
          ┌─────┬─────┬─────┬─────┬─────┬─────┐
          │  1  │  2  │  3  │  4  │  5  │  6  │
          └─────┴─────┴─────┴─────┴─────┴─────┘
  index:     0     1     2     3     4     5
                         ▲
   middle (16 bytes)     │
  ┌───────────┬───────┐  │
  │ pointer ──┼───────┼──┘
  ├───────────┼───────┤
  │ count     │   3   │
  └───────────┴───────┘
```

Let's see what that means in practice.

Filename: views.coax

```jai
#import "Basic";

sum :: (values: [] int) -> int {
  total := 0;
  for v: values { total += v; }
  return total;
}

double_all :: (values: [] int) {
  for i: 0..<values.count { values[i] *= 2; }   // writes reach the caller's array
}

main :: () {
  a := [6] int.[1, 2, 3, 4, 5, 6];

  all := a[..];                 // every element
  middle := a[2..<5];           // elements 2, 3 and 4
  print("% % %\n", all.count, middle, middle.count);
  print("% %\n", sum(all), sum(middle));

  double_all(middle);
  print("%\n", a);              // the array itself changed

  inner := middle[1..<3];       // a view of a view: still a's storage
  inner[0] = 0;
  print("% % %\n", inner, middle, a);

  copy := middle;               // copies the pointer and count, not the elements
  copy[0] = -1;
  print("% %\n", a[2], size_of([] int));
}
```

*Listing 6-2: Views borrow an array's storage, and writes go through*

```shell
$ coax run views.coax
6 [3, 4, 5] 3
21 12
[1, 2, 6, 8, 10, 6]
[0, 10] [6, 0, 10] [1, 2, 6, 0, 10, 6]
-1 16
```

`a[..]` is a view of the whole array. `a[2..<5]` is a view of part of it,
from index 2 up to but not including 5, the same half-open range a `for`
loop uses. So `middle` has three elements, and its `middle[0]` is `a[2]`.
`a[2..<2]` is an empty view, which is allowed.

`sum` takes a `[] int`, so it works on any number of `int`s, whether they
are all of `a` or part of it. When we call it, only the 16-byte view is
copied, never the elements.

`double_all` changes the elements its view points at, and the third line of
the output shows `a` itself changed. That's the locker note at work: the
view leads to `main`'s storage. This is how a Coax procedure fills in or
updates an array for its caller.

Slicing a view gives another view of the same storage. `inner` is a view of
`middle`, which is a view of `a`, and writing `inner[0]` changes `a[3]`.

And copying a view copies only the note. `copy := middle` followed by
`copy[0] = -1` changed `a[2]`, because both views lead to the same lockers.

The signature of a procedure tells you which of the two you get. A `[4] int`
parameter says "I get my own copy of four ints". A `[] int` parameter says
"I may read or write your ints, however many there are".

> Note: In C, a procedure that takes an array takes a pointer, and the
> length travels separately, if at all. A Coax view is the pointer and
> length together. In Rust terms, a view is like `&mut [T]` without the
> borrow checker: it may write, and nothing tracks how long it's valid.

### What coax0 checks about views

Because a view may write through to its storage, `coax0` makes you take one
only of an array variable that can be written. A parameter can't be written
(parameters are immutable), so you can't take a view of a fixed-array
parameter:

```jai
total :: (scores: [5] int) -> int {
  all := scores[..];
  // ...
}
```

```shell
$ coax check total.coax
total.coax:2:16: error[E0366]: a view needs an array variable it may write through, and this is a parameter
    all := scores[..];
                 ^
  fix: copy it into a local variable first
1 error
```

Copy the parameter into a local (`mine := scores;`) and view that, or,
better, have the procedure take a `[] int` in the first place. A view of a
temporary, such as an array literal, is refused for the same reason:

```jai
view_of_lit.coax:2:25: error[E0366]: a view needs an array variable it may write through, and this is a temporary
    v := [3] int.[1, 2, 3][..];
                          ^
  fix: copy it into a local variable first
1 error
```

A view parameter is itself immutable, so a procedure can't move its view to
point somewhere else, though it can change the elements and slice it into a
new local view:

```jai
shift.coax:2:5: error[E0344]: parameters cannot be assigned
    v = v[1..<v.count];
      ^
  fix: copy it into a local variable
1 error
```

A local view variable can be reassigned: `v := a[..]; v = a[1..<3];` is
fine.

Finally, a view can't leave the compiler. A `#run` result (Chapter 9) that
contains one is refused, because it would point into memory that exists
only while compiling:

```jai
table.coax:3:10: error[E0409]: a compile-time result cannot contain a view; views borrow memory that exists only while compiling
  TABLE :: #run make();
           ^
1 error
```

## Bounds checks

Every index and every slice in Coax is checked against the length, in debug
builds and in release builds. When the index is a constant, the check
happens while compiling:

```jai
lookup.coax:4:23: error[E0374]: index 5 is outside [5] int
    print("%\n", scores[5]);
                        ^
  fix: valid indexes are 0 to 4
1 error
```

Most indexes aren't constants, though. Here's a procedure that returns the
last `n` elements of a view, called once too many.

Filename: bounds.coax

```jai
#import "Basic";

// The last n elements of v.
last :: (v: [] int, n: int) -> [] int {
  return v[v.count - n..<v.count];
}

main :: () {
  scores := [5] int.[10, 20, 30, 40, 50];
  print("%\n", last(scores[..], 2));
  print("%\n", last(scores[..], 5));
  print("%\n", last(scores[..], 6));   // one more than there is
  print("never printed\n");
}
```

*Listing 6-3: A slice that runs off the front of its view*

```shell
$ coax run bounds.coax
[40, 50]
[10, 20, 30, 40, 50]
bounds.coax:5: slice out of bounds
```

With `n = 6`, the slice starts at `5 - 6 = -1`. The program stops at that
line, prints where, and exits with a failure status. It doesn't return a
view that starts one element before `scores`, which in C would quietly read
whatever sits in front of the array on the stack. `coax run bounds.coax
--release` stops the same way; bounds checks are not a debug-only feature.

A slice is in bounds when `0 <= lo <= hi <= count`. An index `i` is in bounds
when `0 <= i < count`, and a bad one stops the program with a similar
message:

```shell
$ coax run index.coax
50
index.coax:3: index out of bounds
```

That's `at(scores[..], -1)`; the `50` before it is `at(scores[..], 4)`, the
last valid index. Chapter 7 covers traps like these in more detail.

The check costs a comparison and a branch per access, which modern CPUs
predict well. In a hot loop where it shows up in a profile, the
better fix is usually to check the range once at the top (`if out.count < n
{ return false; }`) and keep the loop simple; the checks inside the loop then
always pass.

## A view doesn't keep its array alive

Bounds checks catch a bad *index*. They can't catch a good index into memory
that has gone away. The checker also diagnoses simple returns of borrows into
local fixed arrays. Here's the mistake in its simplest form.

```jai
#import "Basic";

first_squares :: () -> [] int {
  squares := [4] int.[0, 1, 4, 9];
  return squares[..];            // a view of a local that is about to go away
}

main :: () {
  v := first_squares();
  print("%\n", v);
}
```

*Listing 6-4: Returning a view of a local is rejected with E0383.*

```shell
$ coax check dangle.coax
error[E0383]: returned borrow may refer to a local fixed array
  fix: return the fixed array value, or use caller-owned storage passed as a view
```

`squares` lives in `first_squares`'s stack frame. Without the diagnostic, the
returned view would point into that frame, which is reused after the return:

```jai
 while first_squares runs          after it returns

 first_squares's frame             (free; the next call reuses it)
 ┌────┬────┬────┬────┐             ┌────┬────┬────┬────┐
 │ 0  │ 1  │ 4  │ 9  │             │ ?? │ ?? │ ?? │ ?? │
 └────┴────┴────┴────┘             └────┴────┴────┴────┘
   ▲                                 ▲
   │ pointer, count 4                │ pointer, count 4
 the view                          v, in main
```

The note still says "lockers 0 through 3", but the lockers belong to someone
else now. What `print` shows depends on what the next call happened to
write there; it might even look right, which is the worst case, because the
bug then waits for a change elsewhere to expose it. This is the same mistake
it is in C. The checker refuses this direct form before emitting C.

The same diagnostic follows local aliases and assignments, slices, string
casts and both `ifx` branches. It also recognizes inline array fields and
elements of local values. A local alias keeps a may-borrow flag after an
assignment, so a later overwrite with a parameter borrow can still be refused.
Returning a parameter view or a string borrowed from it is valid; its caller
must keep the storage alive. Returning an array value copies its elements.

This check does not follow calls or borrows stored in aggregate fields or
elements. It does not diagnose callees retaining a borrow, stores into caller
memory, shorter nested-block lifetimes or aliases propagated around loop back
edges. It is an early diagnostic, not general lifetime safety.

The fix is to decide who owns the storage. If the result is small and fixed,
return the array itself, `-> [4] int`, which copies it out. If the caller
should own the storage, have the caller pass a view to fill:

Filename: fill.coax

```jai
#import "Basic";

// Write the first squares into out. The caller owns out, so the results
// outlive this call. Too small a view is refused rather than half-filled.
squares :: (out: [] int, n: int) -> bool {
  if out.count < n { return false; }
  for i: 0..<n { out[i] = i * i; }
  return true;
}

main :: () {
  buffer: [8] int;
  ok := squares(buffer[..], 5);
  print("% %\n", ok, buffer);

  small: [3] int;
  print("% %\n", squares(small[..], 5), small);
}
```

*Listing 6-5: The caller owns the storage and passes a view to fill*

```shell
$ coax run fill.coax
true [0, 1, 4, 9, 16, 0, 0, 0]
false [0, 0, 0]
```

`buffer` lives in `main`'s frame, which outlives the call to `squares`, so
the view `squares` writes through is valid the whole time. The first line of
`squares` checks the size once and refuses a view that's too small instead
of writing part of the answer and trapping halfway.

This is the most common shape of procedure in the Verse code. The camera
in `verse/grid.coax` writes a 4x4 matrix for the native renderer like
this:

```jai
verse_grid_view :: (eye_x: f32, eye_y: f32, eye_z: f32, at_x: f32, at_y: f32, at_z: f32,
                       width: f32, height: f32, out: [] f32) -> bool #export_c {
  if !(width > 0) || !(height > 0) || out.count < 16 { return false; }
  // ...
```

The Swift side owns `out`, Coax fills it, and the `out.count < 16` check
means a short buffer is refused, not overrun.

> Note: The design adds two more ways for a view to go stale, both tied to
> features `coax0` doesn't have yet. Adding to a dynamic array can move its
> storage, leaving every view of the old storage pointing at freed memory.
> Resetting an arena frees everything allocated from it at once, even
> though the bytes still look fine for a while. The design's answer to all
> three is the same: make owners and reset points visible, poison freed
> memory in debug builds, and use handles for anything long-lived. None of
> it is a proof, and the spec says so.

## Strings

A `string` is a view of bytes: a pointer and a count. Everything about
views applies to it, with one difference: you can't write through it.

Filename: strings.coax

```jai
#import "Basic";

main :: () {
  word := "coaxial";
  print("% % % %\n", word.count, word[0], word[2..<4], type_name(word[0]));

  // To change text, copy its bytes into an array you own.
  buffer: [7] u8;
  for c, i: word { buffer[i] = c; }
  buffer[0] = 'C';
  text := cast(string) buffer[..];       // borrows buffer's bytes
  print("% %\n", text, text == "Coaxial");

  buffer[6] = 'S';                       // text sees the change
  print("%\n", text);

  print("% %\n", "héllo".count, "a\0b".count);
  print("% %\n", size_of(string), word[0..<4] == "coax");
}
```

*Listing 6-6: Strings are byte views you can read but not write*

```shell
$ coax run strings.coax
7 99 ax u8
Coaxial true
CoaxiaS
6 3
16 true
```

Indexing a string gives a byte: `word[0]` is the `u8` 99, the code for
`'c'`. Slicing gives a string: `word[2..<4]` is `"ax"`. Both are bounds
checked like any view. `.count` is the number of bytes, and finding it is
free, since it's stored in the view; there's no scan for a terminating zero.

Strings are immutable:

```jai
shout.coax:3:15: error[E0359]: strings are immutable
    greeting[0] = 'H';
                ^
  fix: copy the bytes into a '[N] u8'
1 error
```

A string literal lives in read-only memory, and a string you were handed may
be someone else's bytes, so Coax doesn't let you write through one. To build
or change text, do what the fix says and what Listing 6-6 does: copy the
bytes into a `[N] u8` you own, change them there, and then
`cast(string) buffer[..]` to view them as a string. That cast doesn't copy.
`text` borrows `buffer`, and when we changed `buffer[6]`, `text` changed
with it. It's a view, so it lives exactly as long as `buffer` does.

`==` on strings compares their bytes. `text == "Coaxial"` is true even
though the two strings point at different memory. It's the only aggregate
`==` Coax has; arrays and structs don't compare (Chapter 5 explains why).

Going the other way, a string isn't silently a `[] u8`:

```jai
tostr.coax:3:14: error[E0325]: 'b' is [] u8 but is initialized with string
    b: [] u8 = name;
               ^
1 error
```

That would let you write through it.

Three more things that catch people coming from C:

A string is not a `char *`. It isn't guaranteed to end in a zero byte, and
it can contain zero bytes: `"a\0b"` is three bytes long, and `coax0`
handles it. Crossing into C passes the pointer and the count (Chapter 17).

Bytes are not characters. Coax source and text are UTF-8 by convention, and
`"héllo".count` is 6, because `é` takes two bytes. `s[i]` is the `i`th
byte, not the `i`th letter. Walking code points or grapheme clusters is a
library job, and `coax0` doesn't ship that library yet.

Copying a string copies the view. `t := s` and `s[1..<3]` cost nothing and
share the bytes. That's why substrings are free, and it's also why a string
is only valid as long as its bytes are. A literal lives forever. A string
cast from a buffer lives as long as the buffer.

## Bytes

Files, network packets and GPU buffers arrive as bytes, and turning bytes
into values is where memory bugs in systems code most often start. In Coax
the bytes are a `[] u8`, every read is bounds-checked, and we decode each
field explicitly. Let's read a small header: a four-byte magic number, a
count, and a scale stored as the bits of an `f32`.

Filename: bytes.coax

```jai
#import "Basic";

MAGIC :: 0x58414f43;           // "COAX" read as a little-endian u32
HEADER :: 12;                  // magic, count, scale

u32_at :: (b: [] u8, at: int) -> u32 {
  return cast(u32) b[at] | (cast(u32) b[at + 1] << 8) |
         (cast(u32) b[at + 2] << 16) | (cast(u32) b[at + 3] << 24);
}

Header :: struct { ok: bool; count: int; scale: f32; }

// Admit a header, or refuse it. Nothing is read past the checks.
read_header :: (b: [] u8) -> Header {
  h: Header;
  if b.count < HEADER || cast(int) u32_at(b, 0) != MAGIC { return h; }
  h.count = cast(int) u32_at(b, 4);
  h.scale = f32_from_bits(u32_at(b, 8));
  h.ok = true;
  return h;
}

main :: () {
  file := [12] u8.['C', 'O', 'A', 'X', 3, 0, 0, 0, 0x00, 0x00, 0x80, 0x3f];
  print("%\n", read_header(file[..]));
  print("%\n", read_header(file[0..<8]));      // too short: refused

  file[0] = 'J';
  print("%\n", read_header(file[..]));         // wrong magic: refused
}
```

*Listing 6-7: Decoding a header from bytes, one checked field at a time*

```shell
$ coax run bytes.coax
Header.{ok = true, count = 3, scale = 1.0}
Header.{ok = false, count = 0, scale = 0.0}
Header.{ok = false, count = 0, scale = 0.0}
```

`u32_at` assembles four bytes into a `u32`, lowest byte first. That's
little-endian order, and we wrote it out instead of reinterpreting the
memory, so the result is the same on every target. Each `b[at + k]` is
bounds-checked, so a header that claims more than the buffer holds traps
instead of reading past the end.

`read_header` *admits* the bytes before using them: it checks the length
and the magic number once, at the top, and returns a `Header` with `ok =
false` if either is wrong. Everything after that line can index freely,
because the checks proved the bytes are there. The second call passes only
the first eight bytes and is refused; the third has the wrong magic and is
refused too.

`f32_from_bits` reinterprets a `u32`'s bits as an `f32`. `0x3f800000` is the
IEEE bit pattern for 1.0, which is the `scale` we got. Its partners are
`f32_bits`, `f64_bits` and `f64_from_bits`. `coax0` also provides
`bytes_overlap(a, b)`, which tells you whether two byte views or strings
share any memory; it's for code that copies between buffers and must not
copy a buffer onto itself.

> Note: This is how `verse/cast.coax` loads the character and robot
> animation file. `verse_cast_open :: (b: [] u8) -> Cast` checks the
> file's length, magic, version and buffer count with `int_at`, `u32_at`
> and friends, and returns a `Cast` that isn't `ok` if anything is off. The
> native side owns the file's bytes. Chapter 15 shows the same idea
> generated from a protocol description.

## Memory that outlives a procedure (planned)

Everything so far has been memory with an obvious owner: a stack frame, a
constant, or the host. Real engines also need memory that one procedure
creates and another frees later: a list that grows as enemies spawn, a
level's worth of geometry, a cache. None of this is in `coax0`. This section
describes what the spec plans, and the errors you'll see if you try it today.

### Dynamic arrays

A dynamic array, `[..] T`, is a pointer, a count, a capacity and the
allocator it came from. It grows with `array_add` and is released with
`free`. Copying one copies that descriptor, not the elements, so two copies
share storage, just as two views do. `coax0` refuses the type and suggests
what to do in the meantime:

```jai
grow.coax:2:12: error[E0301]: dynamic arrays '[..] T' are not supported by coax0 yet
    pending: [..] int;
             ^
  fix: use a fixed array '[N] T' and a count
1 error
```

A fixed array with a separate count, `pending: [64] Pulse; pending_count:
int;`, is the honest version today: a bounded list with its capacity
written in the type.

Growth is the reason dynamic arrays need care. When `array_add` runs out of
capacity it moves the elements to a bigger block and frees the old one.
Every view and pointer into the old block is now a note for lockers that
have been torn down.

### Pointers

The planned pointer type is `*T`, and it is never null. A pointer that may
be null is `*T?`, and it must be tested before use. `*x` takes an address,
and `p.*` reads or writes through a pointer. A field access works through a
pointer too, so `p.pos` means the same whether `p` is an `Entity` or a
`*Entity`. None of this parses in `coax0`:

```jai
ptr.coax:3:6: error[E0202]: expected a type, found '*'
    p: *int = *value;
       ^
1 error
```

(`int?` and other optionals are planned as well; today `?` is
`E0108: unexpected character`.)

> Note: The spelling matches current Jai. Older Jai code, including Forrest
> Smith's Advent of Code solutions, writes `<<p` to dereference.

A pointer can outlive what it points at, exactly like a view. Coax doesn't
plan a borrow checker to stop that. It plans to make long-lived raw
pointers unnecessary most of the time, with handles.

### Handles instead of long-lived pointers

A `Handle(T)` is a 32-bit index into a pool plus a generation number. When
a slot is recycled, its generation changes, and a stale handle fails its
check instead of reading whatever moved in.

```jai
door: Handle(Entity);
if e := world.get(door) { e.open = true; }
```

The engine's entity system already works this way (`ecs::Entity`). Anything
that outlives a frame, is saved, or has to survive a hot reload (Chapter 16)
should hold a handle rather than an address. Today, the closest thing is an
`int` index into an array plus a check of `count`, which catches an index
past the end but not a slot that has been reused.

### Who owns what

Every block of memory in an engine has an owner and a moment when it goes
away. The design wants both written down:

| Storage | Owner | Invalidated when |
| --- | --- | --- |
| Persistent world | The resident host | Never by a code reload; versioned handles survive it |
| Level arena | The level | The level unloads; every borrow must end first |
| Thread scratch | One worker | Reset per task or frame; can't escape to another worker |
| Pool | The pool | A slot is recycled and its generation changes |
| Dynamic array | The container's owner | It grows (storage moves) or is freed |
| Foreign buffer | The foreign library | Its documented release call |

An arena's value is as much its shared lifetime as its speed. Everything in
a level arena dies together, so nothing in it is freed one at a time. The
danger is a pointer that escapes the arena and is used after the reset.

Allocation will always be a visible call: `New`, `NewArray`, `array_add`,
`free` and `destroy`. Freeing a struct won't recursively free what its
fields point at, and `destroy` releases only what its own contract says it
owns. Today those names are simply undeclared:

```jai
new.coax:2:13: error[E0315]: procedure 'NewArray' is not declared
    scores := NewArray(100, int);
              ^
new.coax:2:27: error[E0306]: 'int' is not declared
    scores := NewArray(100, int);
                            ^
2 errors
```

## The context (planned)

If procedures allocate, the next question is from where. Passing an
allocator parameter to every procedure that might allocate is tedious and
leaks into every signature in between. Jai and Odin answer with a
*context*: an implicit value passed to every ordinary procedure, holding the
current allocator, a scratch arena and a logger. Coax plans the same:

```jai
build_level :: (desc: Level_Desc) -> Level { ... }   // allocates through the context

push_context .{allocator = level_arena} {
  level := build_level(desc);    // everything it allocates lands in the level arena
}
```

`push_context` changes the context for a block and everything called from
it, then restores the old one when the block ends. That lets you put a
whole subsystem under an arena, a tracking allocator or a different logger
without changing any signature. It's the idea from Jai we're most confident
about. `coax0` has no context at all:

```jai
ctx.coax:2:3: error[E0204]: 'push_context' is reserved and not supported by coax0 yet
    push_context .{allocator = level_arena} { }
    ^
1 error
```

An implicit context has costs, and the design handles each one explicitly:

A signature shows less. A procedure with no allocator parameter can still
allocate. So the compiler is to track allocation as an *effect* in every
procedure's summary, through every call it makes. A job (Chapter 12), a
shader or a no-allocation section can't call something that allocates, and
the error names the call that does.

It stops at the C boundary. C-callable procedures, such as `#export_c`
ones, and `#no_context` leaf procedures neither take nor pass a context. Changing the
Coax context doesn't redirect a C library's `malloc`.

Its layout is private. Separately built modules talk through C entry points
and opaque handles, never by sharing a context struct whose layout could
change between versions.

## Temporary storage (planned)

Much of a frame's memory only has to last a moment: a formatted label, a
sorted copy of a list, the set of entities near the cursor. The context's
scratch arena is for that:

```jai
names := format_list(entities, allocator = context.scratch);
draw_label(names);
// ... once per frame, the frame loop calls reset_scratch()
```

Allocating from scratch is a pointer bump, and freeing all of it is one
reset. Each worker thread has its own, so scratch allocation never needs a
lock. In debug builds, the design poisons scratch memory on reset and
reports a scratch pointer that was stored somewhere persistent at the reset
that invalidates it. The spec is explicit that an allocator existing doesn't
by itself enforce any of these lifetimes.

Today, temporary storage is a local fixed array, like `buffer` in Listing
6-5. It's freed when the procedure returns, which is a fine lifetime for a
lot of temporary work, as long as nothing returns a view of it.

## What Coax checks, today and later

Coax doesn't promise general memory safety. Views, foreign calls and, later,
pointers and shared mutable data can all be misused, and no allocator design
changes that. Here is what's actually checked.

| Protection | Current status |
| --- | --- |
| New values are zeroed or default initialized unless you write `---` | Enforced |
| Every index and slice is bounds-checked, debug and release | Enforced (trap at run time, error for constant indexes) |
| Strings can't be written through | Enforced (`E0359`) |
| A view is only taken of a writable array variable | Enforced (`E0366`) |
| A view can't escape compile time | Enforced (`E0409`) |
| Integer overflow traps in development builds | Enforced (wraps in `--release`) |
| Direct/simple-alias returns of local-array views and strings | Rejected (`E0383`); general borrow lifetimes remain unchecked |
| Handles with generation checks for long-lived references | Planned |
| Explicit owners and reset points; debug poisoning of freed memory | Planned |
| Allocation as a checked effect | Planned |

That's how careful C++ engines stay correct today, by convention and
tooling. The difference Coax aims for is that these are the defaults, and
the compiler knows about them.

## Summary

In `coax0`, every value lives in a stack frame, in constant data, or in
memory the host owns and passes in as a view. Fixed arrays are values with a
length known while compiling: assigning or passing one copies every element.
A view is a pointer and a count that borrows an array's storage. Copying it
is cheap, writes through it reach the array, and slicing it gives another
view of the same storage. Strings are views of bytes that can't be written
through, and `cast(string)` borrows a byte array as text. Bytes from outside
are decoded field by field and admitted once at the top.

Every index and slice is checked in every build, so a bad index stops the
program at the line that made it. Simple returns of local-array borrows are
diagnosed too, but a view still doesn't keep its array alive. General lifetime
discipline remains the programmer's responsibility, usually by having the
caller pass in a view to fill.

Dynamic arrays, pointers, handles, allocators, the context and scratch
memory are planned, with ownership and allocation made visible rather than
proven safe. In Chapter 7 we'll look at the other half of writing reliable
code: errors, traps, and cleaning up with `defer`.

For callbacks, cancellation and GPU buffers whose use outlives a call, see
[Chapter 23](ch23-asynchronous-work.md). A view does not retain its backing storage.
