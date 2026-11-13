# Coax Lab

An unlisted learning world at https://openagents.com/worlds/coax-lab, made
for Episode 296, *The Coax Programming Language*. You come in on a plaza on
the Grid with your Everglade character. Five avenues leave it, one per
district, each with its name in light on the plaza floor. There are
seventeen stations, and they follow
[the book](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/README.md).

Each station ends with a line that says where its values come from:

- **Live** means the page works the value out as you stand there. The code
  that does it is ordinary Coax, compiled into the page.
- **Recorded** means the station shows what the compiler (`coax0` 0.52.0)
  printed on 2026-11-13. The compiler doesn't run in the page, so its output
  is kept as text.

Source code on a station is coloured by the compiler's own lexer, which is
written in Coax and runs in the page for each line.

## Getting around

- **INDEX** (or **M**) lists every district and its stations. Choose a
  station to go straight to it.
- **NEXT** and **PREV** (or **]** and **[**) step through the stations in
  order, district by district. The plaza comes before the first and after
  the last.
- Every station has its own address, such as
  [`?at=overflow`](/worlds/coax-lab?at=overflow). The address bar follows
  you, so a link opens where you were. A district's address, such as
  [`?at=agents`](/worlds/coax-lab?at=agents), opens its first station, and
  `?at=start` opens the plaza.

## 1 Start here

1. **A name and a value** (`?at=names`): the ways to declare. `LIMIT :: 64`
   is a constant, `x := 1` a variable, `y: int = 2` a variable with its type
   written, and a conversion is always written out. Each part lights up in
   turn with its value beside it. Live.
2. **A procedure** (`?at=procedure`): `progress`, from the engine's camera,
   read a part at a time: its name, its arguments, what it returns. The
   same procedure moves a light along a track as you read. Live.
3. **Numbers that stop** (`?at=overflow`): two bytes start at 250 and add 1
   ten times. The checked one stops at 255. The wrapping one goes round to 0
   and ends at 4, which is what C's `uint8_t` does. Both counters are live;
   the compiler's line, `over.coax:4: integer overflow in '+'`, is recorded.

## 2 Data

4. **A struct, byte by byte** (`?at=struct`): three structs as rows of
   bytes, a colour per field, padding in red. The same four fields take 24
   bytes in one order and 16 in another. Live, from `size_of` and
   `field_info`.
5. **An array and a view** (`?at=view`): eight cells, and a view over four
   of them. An index steps along the view; one past its end is refused,
   even where the array still has a value. The reads are live; the
   compiler's line, `index out of bounds`, is recorded.
6. **Cleanup you can see** (`?at=defer`): three `defer` lines are set aside
   as they run, then run in reverse when the block ends. Below, a parser
   passes an error up with `try`, and its cleanup still runs once. Live.
7. **Life** (`?at=life`): Conway's Life, a struct holding an array that is
   copied by value every generation. Live.

## 3 The compiler

8. **The lexer** (`?at=lexer`): the first stage of the compiler, reading a
   line of source into tokens one at a time, each named by its kind. This is
   the compiler's own lexer. The lexer, parser and checker are written in
   Coax; the rest of the compiler is C++ today and isn't in the page. Live.
9. **Work done while compiling** (`?at=run`): a CRC-32 table of 256 numbers
   that the compiler built by running the program's own code (`#run`). The
   page holds the finished table and uses it to sum a text as it is typed.
   Live.
10. **The compiler drew this** (`?at=mandelbrot`): a Mandelbrot set of 3,128
    points, each worked out by the compiler before the page existed. A probe
    calls the same procedure now for one point and compares. Live.

## 4 For agents

11. **An error with a fix** (`?at=error`): the compiler refusing a shadowed
    name: the code `E0303`, the place, the caret and the fix line, then the
    same error as JSON and `coax explain`. Recorded.
12. **A check** (`?at=forall`): a `#forall` over refunds. All 1,974 pairs of
    amounts are tried in order and fill a grid. Live.
13. **A planted bug** (`?at=mutant`): one operator changes from `<=` to `<`.
    Forty cases go red, so the check catches it. The planted bug runs live;
    `43 mutants, 43 killed, 0 survived` is the recorded result of
    `coax mutate` on the book's example.
14. **Authority** (`?at=authority`): a file marked `#authority` with a
    procedure that no check runs. The compiler refuses it (`E0380`). With a
    check added, it builds. Recorded.

## 5 Where it runs

15. **One source, three targets** (`?at=targets`): one small procedure as
    Coax, as the C the compiler writes and as the WGSL shader it writes. Two
    panels run it now: the page on the left, for 728 cells, and the GPU on
    the right, for every pixel. The panels are live; the C and WGSL text is
    recorded.
16. **Beside Jai, C and Rust** (`?at=compare`): five small things written in
    each language. Coax starts from Jai's design; it isn't Jai, and it makes
    no promise of memory safety. From the book's Appendix C.
17. **Built with it** (`?at=built`): the folders of the repository and how
    many Coax files each holds, counted on 2026-11-13, ending with this
    world and the number of lit quads it is drawing.

Read more in [The Coax Programming Language](https://github.com/OpenAgentsInc/openagents/blob/main/coax/book/README.md),
the book these stations follow.

Use WASD or the arrow keys to walk, Shift to walk slowly and Space to jump;
drag to turn the follow camera. On touch, drag the left side to walk and the
right side to look, and tap INDEX, PREV and NEXT under the panel at the top
left. WORLDS (top right) goes back to [Worlds](/worlds).
