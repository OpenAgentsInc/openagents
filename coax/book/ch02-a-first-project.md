# A first project

Let's jump into Coax by writing a real program together. In this chapter
we'll draw the Mandelbrot set, the famous fractal, as text in the
terminal. It's a good first project because it's small, it's fun to
look at, and it touches a lot of the everyday language: constants and
variables, loops, procedures, structs, floating-point math, fixed
arrays, strings and printing. We'll even write a few checks that the
compiler runs for us.

We'll go quickly. Each idea is introduced lightly here and covered
properly in a later chapter, and the summary at the end says which. The
goal for now is to get a feel for what writing Coax is like, including
what happens when we get something wrong.

> Note: Everything in this chapter runs with `coax0` today. Each stage of
> the program is in [`examples/`](https://github.com/OpenAgentsInc/openagents/tree/main/coax/book/examples/) as `ch02_step1.coax`
> through `ch02_step7.coax`, with its output beside it.

## The Mandelbrot set in one paragraph

Take a point *c* on the plane, and treat it as a complex number: an *x*
coordinate (the real part) and a *y* coordinate (the imaginary part).
Start with *z* = 0 and repeat *z* = *z*² + *c* over and over. For some
points, *z* grows without limit. For others, it stays small forever.
The Mandelbrot set is the set of points where it stays small. If *z*
ever gets more than 2 away from the origin, it's guaranteed to escape,
so that's our test. That's all the math we need. Coloring each point by
how quickly it escapes gives the famous pictures.

Our plan: treat each character of the terminal as one point, do the
test for each, and print a character.

## Setting up a grid

Make a new file called `mandelbrot.coax`. We'll keep editing this one
file through the whole chapter. The first version doesn't do any math.
It just prints a rectangle of dots, 64 characters wide and 24 tall, so
we know our loops are right.

Filename: mandelbrot.coax

```jai
#import "Basic";

WIDTH :: 64;
HEIGHT :: 24;

main :: () {
  for row: 0..<HEIGHT {
    for col: 0..<WIDTH {
      print(".");
    }
    print("\n");
  }
}
```

*Listing 2-1: Printing a 64 by 24 grid of dots*

There are two new things here. The first is `WIDTH :: 64;`. In
Chapter 1 we saw that `::` declares a constant; here the constant is a
number. Constants can't change, and the compiler knows their values
while compiling, so we can use them anywhere a fixed number is needed.
By convention, constants that are plain numbers get uppercase names.

The second is the `for` loop. `for row: 0..<HEIGHT` runs its body once
for each integer from 0 up to, but not including, `HEIGHT`. The `..<`
reads as "up to but not including", so the loop runs 24 times, with
`row` taking the values 0, 1, …, 23. Inside it, a second loop does the
same for `col`, printing a dot for each column, and after each row we
print a newline.

Run it:

```sh
$ coax run mandelbrot.coax
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
................................................................
```

A field of dots, exactly 64 by 24. Our loops work.

## From characters to points

Now we need to decide which part of the plane each character stands
for. The interesting part of the Mandelbrot set lies between about −2.2
and 1.0 on the *x* axis, and −1.2 and 1.2 on the *y* axis. We'll
spread our 64 columns across the *x* range and our 24 rows across the
*y* range.

To check that the mapping is right before we do anything fancy, let's
draw something whose shape we know: a circle of radius 1 around the
origin. A point (*x*, *y*) is inside that circle when *x*² + *y*² < 1.

Here's a first attempt. We add four constants for the edges of the
picture, compute *x* and *y* for each character, and print `#` for points
inside the circle:

```jai
#import "Basic";

WIDTH :: 64;
HEIGHT :: 24;

LEFT :: -2.2;
RIGHT :: 1.0;
TOP :: 1.2;
BOTTOM :: -1.2;

main :: () {
  for row: 0..<HEIGHT {
    y := TOP - row * (TOP - BOTTOM) / HEIGHT;
    for col: 0..<WIDTH {
      x := LEFT + col * (RIGHT - LEFT) / WIDTH;
      if x * x + y * y < 1.0 { print("#"); } else { print("."); }
    }
    print("\n");
  }
}
```

*Listing 2-2: Mapping rows and columns to points (this doesn't compile)*

The `if` statement works the way you'd expect: if the condition is
true, the first block runs, and otherwise the `else` block does. Note
that the braces are required, even around a single statement.

Let's run it:

```sh
$ coax run mandelbrot.coax
mandelbrot.coax:13:27: error[E0350]: a float literal cannot become int
      y := TOP - row * (TOP - BOTTOM) / HEIGHT;
                            ^
  fix: write an integer, or cast(int) a float value
mandelbrot.coax:15:32: error[E0350]: a float literal cannot become int
        x := LEFT + col * (RIGHT - LEFT) / WIDTH;
                                 ^
  fix: write an integer, or cast(int) a float value
2 errors
```

This is an error almost everyone hits in their first hour of Coax, so
let's understand it. `row` is an `int`: loop variables over a range of
integers are integers. `TOP - BOTTOM` is made of constants we wrote as
`1.2` and `-1.2`, which don't have a fixed type yet. Coax calls them
*untyped*: they take on whatever type their use needs, as long as the
value fits. Here the use is `row * (TOP - BOTTOM)`, and multiplying by
an `int` needs an `int`. But 2.4 isn't an integer, so the constant
can't become one, and the compiler stops.

In C, this would compile. C would quietly convert `row` to a `double`
and carry on, and much of the time that's what you wanted. Sometimes it
isn't, and the bug is very hard to see. Coax never converts between
number types for you. You write the conversion, and then anyone reading
the line knows exactly what happens.

The fix is to say that we want `row` as a 64-bit float. `f64(row)`
converts it. While we're editing these lines, let's make one more
change: we'll add `0.5` to the row and column so that each character
samples the middle of its cell instead of its top-left corner. That
keeps the picture symmetric. Here are the two fixed lines:

```jai
    y := TOP - (f64(row) + 0.5) * (TOP - BOTTOM) / HEIGHT;
    // ...
      x := LEFT + (f64(col) + 0.5) * (RIGHT - LEFT) / WIDTH;
```

*Listing 2-3: Converting the loop counters to `f64`*

Now everything in each expression is an `f64`, and the untyped
constants happily become `f64` too. Notice that `y` and `x` are declared
with `:=` and no type. Coax infers `f64` from the value. Run it again:

```sh
$ coax run mandelbrot.coax
................................................................
................................................................
......................................############..............
.................................######################.........
...............................##########################.......
.............................##############################.....
...........................##################################...
..........................####################################..
.........................######################################.
.........................######################################.
........................########################################
........................########################################
........................########################################
........................########################################
.........................######################################.
.........................######################################.
..........................####################################..
...........................##################################...
.............................##############################.....
...............................##########################.......
.................................######################.........
......................................############..............
................................................................
................................................................
```

A circle! It's centered where *x* is 0, which is a little right of the
middle because our *x* range isn't centered on 0. It also looks round
rather than squashed, because a terminal character is about twice as
tall as it is wide, and we chose ranges to match: each column covers
0.05 of the plane and each row 0.1.

## Complex numbers as a struct

Now for the real thing. Our test needs to square complex numbers and
add them, so let's give ourselves a type for them. A *struct* groups
named values, its *fields*, into one value:

```jai
Complex :: struct {
  re: f64;
  im: f64;
}
```

`Complex` is a new type with two `f64` fields, `re` for the real part
and `im` for the imaginary part. A field declaration reads like a
variable declaration: the name, a colon, and the type.

Next, some procedures that work on `Complex` values. Here's the new
version of the program, with the parts that haven't changed left out:

Filename: mandelbrot.coax

```jai
#import "Basic";

// ...

MAX_STEPS :: 50;

Complex :: struct {
  re: f64;
  im: f64;
}

add :: (a: Complex, b: Complex) -> Complex {
  return Complex.{re = a.re + b.re, im = a.im + b.im};
}

square :: (z: Complex) -> Complex {
  return Complex.{re = z.re * z.re - z.im * z.im, im = 2.0 * z.re * z.im};
}

length_squared :: (z: Complex) -> f64 {
  return z.re * z.re + z.im * z.im;
}

// Does z = z*z + c stay small for MAX_STEPS steps?
in_set :: (c: Complex) -> bool {
  z: Complex;
  for step: 0..<MAX_STEPS {
    z = add(square(z), c);
    if length_squared(z) > 4.0 { return false; }
  }
  return true;
}

main :: () {
  for row: 0..<HEIGHT {
    y := TOP - (f64(row) + 0.5) * (TOP - BOTTOM) / HEIGHT;
    for col: 0..<WIDTH {
      x := LEFT + (f64(col) + 0.5) * (RIGHT - LEFT) / WIDTH;
      c := Complex.{re = x, im = y};
      if in_set(c) { print("#"); } else { print("."); }
    }
    print("\n");
  }
}
```

*Listing 2-4: Testing each point for membership in the Mandelbrot set*

There's a lot here, so let's take it a piece at a time.

`add :: (a: Complex, b: Complex) -> Complex` declares a procedure with
two parameters, each with a name and a type, and `-> Complex` says what
it returns. Unlike `main`, these procedures return a value, so they end
with `return`.

`Complex.{re = a.re + b.re, im = a.im + b.im}` is a *struct literal*. It
builds a new `Complex`, naming each field it sets. `a.re` reads the `re`
field of `a`. Structs in Coax are values, like numbers: passing `a` to
`add` copies it, and there's no hidden pointer or allocation involved.

`square` uses the rule for squaring a complex number, (*a* + *bi*)² =
*a*² − *b*² + 2*abi*. And `length_squared` gives the square of *z*'s
distance from the origin. We compare it with 4 instead of comparing the
distance with 2, which saves a square root.

`in_set` is where the work happens. `z: Complex;` declares a variable
with an explicit type and no value. Coax doesn't leave a variable
holding garbage unless you ask it to (with `---`, which we'll meet in
Chapter 3): a declared variable starts at zero, so `z` starts as
0 + 0*i*, which is what the math wants. Then the loop runs our step up to
`MAX_STEPS` times. If *z* escapes, we return `false` right away. If it
survives all 50 steps, we say the point is in the set. (Strictly, we
don't know that for sure, but 50 steps is plenty for a picture this
size.)

Finally, `main` builds a `Complex` for each character and asks `in_set`
about it. Notice that `in_set` and the other procedures are declared
before `main` here, but they don't have to be. Coax lets you refer to
procedures declared later in the file, so you can order a file however
reads best.

Run it:

```sh
$ coax run mandelbrot.coax
................................................................
................................................................
................................................................
.........................................#......................
.......................................####.....................
.......................................#####.#..................
.................................###############.#..............
...............................###################..............
..............................######################............
.....................#.###...#######################............
...................#########.#######################............
.................##################################.............
.................##################################.............
...................#########.#######################............
.....................#.###...#######################............
..............................######################............
...............................###################..............
.................................###############.#..............
.......................................#####.#..................
.......................................####.....................
.........................................#......................
................................................................
................................................................
................................................................
```

There it is: the Mandelbrot set, with its big cardioid body, the round
bulb on its left, and little sprouts on its edges.

## Adding shading

The pictures you've seen of the Mandelbrot set are colored by how fast
each point escapes. We can do the same with characters, using a string
of increasingly "heavy" characters as a palette:

```jai
PALETTE :: " .:-=+*#%@";
```

Constants can be strings too. Now `in_set` becomes `escape_steps`, which
returns *how many* steps a point survived instead of yes or no:

```jai
// How many steps until z = z*z + c escapes? MAX_STEPS if it never does.
escape_steps :: (c: Complex) -> int {
  z: Complex;
  for step: 0..<MAX_STEPS {
    z = add(square(z), c);
    if length_squared(z) > 4.0 { return step; }
  }
  return MAX_STEPS;
}
```

In `main`, we'll use that count to pick a character from the palette. A
string can be *sliced*: `PALETTE[i..<i + 1]` is the part of the string
from byte `i` up to but not including byte `i + 1`, which is a string
one character long. So our first try is:

```jai
      c := Complex.{re = x, im = y};
      i := escape_steps(c);
      print("%", PALETTE[i..<i + 1]);
```

*Listing 2-5: Choosing a character from the palette (this has a bug)*

This compiles fine. Let's run it:

```sh
$ coax run mandelbrot.coax
           ........:::::::::::::::::----=----::::...............
          ......::::::::::::::::-----==+mandelbrot.coax:48: slice out of bounds
```

The program printed one row and part of the next, then stopped. A step
count can be anything from 0 to 50, but `PALETTE` has only 10
characters. As soon as a point survived 10 steps, we asked for a slice
that runs past the end of the string, and Coax stopped the program and
told us which line did it.

This is a different kind of error from the one we saw before. The
compiler can't know ahead of time which points will survive how long,
so it can't reject the program. Instead, it checks every index and
slice while the program runs. In C, reading past the end of a string is
undefined behavior: the program might print garbage, crash later, or
appear to work. Coax makes this check in every build, including release
builds, because there's no sensible value to read past the end.

What we really want is to scale the step count, 0 to 50, onto the
palette's positions, 0 to 9:

```jai
      i := escape_steps(c) * (PALETTE.count - 1) / MAX_STEPS;
      print("%", PALETTE[i..<i + 1]);
```

*Listing 2-6: Scaling the step count onto the palette*

`PALETTE.count` is the length of the string in bytes, 10. Since
everything here is an `int`, the division rounds down, so we multiply
first and divide last; dividing first would round almost everything to
zero. A point that never escapes gets `50 * 9 / 50`, which is 9, the
`@` at the end of the palette. Run it:

```sh
$ coax run mandelbrot.coax
                                                                
                                        :.                      
                                       .....                    
                                     .::-@-.:                   
                                   ...:@@@@%...                 
                               .+:..*:-@@@@@#@-.....            
                              ..+@@@@@@@@@@@@@@@-@#.            
                            ..*@@@@@@@@@@@@@@@@@@@:..           
                  :....:.....+@@@@@@@@@@@@@@@@@@@@@@-.          
                 ...+@#@@@=..@@@@@@@@@@@@@@@@@@@@@@@.           
               ....@@@@@@@@@-@@@@@@@@@@@@@@@@@@@@@@@.           
        ... ...:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@.            
        ... ...:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@.            
               ....@@@@@@@@@-@@@@@@@@@@@@@@@@@@@@@@@.           
                 ...+@#@@@=..@@@@@@@@@@@@@@@@@@@@@@@.           
                  :....:.....+@@@@@@@@@@@@@@@@@@@@@@-.          
                            ..*@@@@@@@@@@@@@@@@@@@:..           
                              ..+@@@@@@@@@@@@@@@-@#.            
                               .+:..*:-@@@@@#@-.....            
                                   ...:@@@@%...                 
                                     .::-@-.:                   
                                       .....                    
                                        :.                      
                                                                
```

Much better. The set itself is solid `@`, and the bands around it show
how close each point came to staying inside.

## Building each line in an array

Our program calls `print` 64 times for each row. That works, but it's
a lot of small writes. Let's build each row in memory first and print it
once. That's a job for a *fixed array*:

```jai
  line: [WIDTH] u8;
```

`[WIDTH] u8` is an array of `WIDTH` bytes (`u8` is an unsigned 8-bit
integer). The size has to be known while compiling, which is why
`WIDTH` being a constant matters. Like `z`, the array starts out all
zeros. The array lives right there in `main`'s local storage; nothing is
allocated.

To fill it, we index `line` and the palette. Indexing a string with a
single number, `PALETTE[i]`, gives the byte at that position. Our first
thought might be to keep the slice we used before:

```jai
      line[col] = PALETTE[i..<i + 1];
```

But the compiler objects:

```sh
$ coax run mandelbrot.coax
mandelbrot.coax:49:26: error[E0327]: the target is u8, assigned string
        line[col] = PALETTE[i..<i + 1];
                           ^
1 error
```

An element of `line` is a byte, and a slice of a string is a string,
even when it's one character long. Indexing gives us the byte we need.
Here's the new `main`:

```jai
main :: () {
  line: [WIDTH] u8;
  for row: 0..<HEIGHT {
    y := TOP - (f64(row) + 0.5) * (TOP - BOTTOM) / HEIGHT;
    for col: 0..<WIDTH {
      x := LEFT + (f64(col) + 0.5) * (RIGHT - LEFT) / WIDTH;
      c := Complex.{re = x, im = y};
      i := escape_steps(c) * (PALETTE.count - 1) / MAX_STEPS;
      line[col] = PALETTE[i];
    }
    print("%\n", cast(string) line[..]);
  }
}
```

*Listing 2-7: Building each row in a fixed array*

The last line is the interesting one. `line[..]` is a *view* of the
whole array: a pointer to its first element plus a count, without
copying anything. `cast(string)` then lets us treat that view of bytes
as text, which `print` can print. The output is the same picture as
before, and that's exactly what we wanted. Indexing an array is checked
just like slicing a string, so if we'd got a loop bound wrong and
written `line[64]`, the program would stop and tell us the line.

> Note: If you print the array itself, with `print("%\n", line)`, you get
> its elements as numbers, such as `32` for each space. `print` shows values
> as what they are, so a byte array is a list of bytes until you say
> it's text.

## Zooming in

Every part of the picture has been controlled by constants so far. To
look at a different part of the set, we'd have to edit four of them.
Let's describe what we're looking at with a struct instead, so that we
can draw several pictures in one run:

```jai
// The part of the plane we draw, and how hard we look.
View :: struct {
  center: Complex = Complex.{re = -0.6, im = 0.0};
  width: f64 = 3.2;
  max_steps: int = 50;
}
```

The fields have *default values*. A `View` that doesn't say otherwise
is centered on −0.6, 3.2 units wide, and tries 50 steps per point,
which is exactly the picture we've been drawing. A field can itself be
a struct, as `center` is.

We delete `LEFT`, `RIGHT`, `TOP`, `BOTTOM` and `MAX_STEPS`, give
`escape_steps` a `max_steps` parameter, and move the drawing out of
`main` into a procedure that takes a `View`:

```jai
escape_steps :: (c: Complex, max_steps: int) -> int {
  z: Complex;
  for step: 0..<max_steps {
    z = add(square(z), c);
    if length_squared(z) > 4.0 { return step; }
  }
  return max_steps;
}

render :: (view: View) {
  // A character cell is about twice as tall as it is wide.
  height := view.width * HEIGHT / WIDTH * 2.0;
  left := view.center.re - view.width / 2.0;
  top := view.center.im + height / 2.0;

  line: [WIDTH] u8;
  for row: 0..<HEIGHT {
    y := top - (f64(row) + 0.5) * height / HEIGHT;
    for col: 0..<WIDTH {
      x := left + (f64(col) + 0.5) * view.width / WIDTH;
      steps := escape_steps(Complex.{re = x, im = y}, view.max_steps);
      i := steps * (PALETTE.count - 1) / view.max_steps;
      line[col] = PALETTE[i];
    }
    print("%\n", cast(string) line[..]);
  }
}

main :: () {
  whole: View;
  render(whole);

  bulb := View.{center = Complex.{re = -1.25, im = 0.0}, width = 0.5, max_steps = 200};
  print("\nzoomed in on %\n", bulb.center);
  render(bulb);
}
```

*Listing 2-8: Drawing any part of the set with a `View`*

`render` works out the picture's height from its width, keeping the
same shape as before, then finds its left and top edges from the
center. The rest is the loop we already had, reading `view.width` and
`view.max_steps` instead of constants. `render` has no return type
because it doesn't return anything; it prints.

In `main`, `whole: View;` declares a `View` that takes all the default
values. For `bulb`, a struct literal names only the fields it changes.
It's centered on the round bulb to the left of the main body, a sixth of
the width, and since we're looking more closely we allow 200 steps. We
also print `bulb.center` with `%`: `print` knows how to show a whole
struct, which is handy when you're debugging.

```sh
$ coax run mandelbrot.coax
                                                                
                                        :.                      
                                       .....                    
                                     .::-@-.:                   
                                   ...:@@@@%...                 
                               .+:..*:-@@@@@#@-.....            
                              ..+@@@@@@@@@@@@@@@-@#.            
                            ..*@@@@@@@@@@@@@@@@@@@:..           
                  :....:.....+@@@@@@@@@@@@@@@@@@@@@@-.          
                 ...+@#@@@=..@@@@@@@@@@@@@@@@@@@@@@@.           
               ....@@@@@@@@@-@@@@@@@@@@@@@@@@@@@@@@@.           
        ... ...:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@.            
        ... ...:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@.            
               ....@@@@@@@@@-@@@@@@@@@@@@@@@@@@@@@@@.           
                 ...+@#@@@=..@@@@@@@@@@@@@@@@@@@@@@@.           
                  :....:.....+@@@@@@@@@@@@@@@@@@@@@@-.          
                            ..*@@@@@@@@@@@@@@@@@@@:..           
                              ..+@@@@@@@@@@@@@@@-@#.            
                               .+:..*:-@@@@@#@-.....            
                                   ...:@@@@%...                 
                                     .::-@-.:                   
                                       .....                    
                                        :.                      
                                                                

zoomed in on Complex.{re = -1.25, im = 0.0}
                             .  .  :   .@@@@@@@@@@@@@@@@@@@@@@@@
                                 ..-=-.-@@@@@@@@@@@@@@@@@@@@@@@@
                                  .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                                  .*@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                               .:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               .      .          .-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               :.  .  ..  :    .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                ...::..:-.. .  .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                 .=@@@@@@@@@:...@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               .:--@@@@@@@@@@@-.@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
            .....@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
          ...-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
          ...-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
            .....@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               .:--@@@@@@@@@@@-.@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                 .=@@@@@@@@@:...@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                ...::..:-.. .  .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               :.  .  ..  :    .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
               .      .          .-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                               .:-@@@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                                  .*@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                                  .=@@@@@@@@@@@@@@@@@@@@@@@@@@@@
                                 ..-=-.-@@@@@@@@@@@@@@@@@@@@@@@@
                             .  .  :   .@@@@@@@@@@@@@@@@@@@@@@@@
```

The first picture is the one from before. The second is the bulb up
close, with smaller copies of the pattern sprouting from its edge.
Try changing `bulb`'s center and width to explore. Points near the
boundary of the set, like −0.745 + 0.11*i*, are where the detail is.

## Checking our work

We've been checking our work by looking at pictures. That's fine for a
picture, but it's worth seeing how Coax lets us state facts about our
code and have the compiler confirm them. Let's add some tests just
before `render`:

```jai
#test "the origin never escapes" {
  #assert escape_steps(Complex.{re = 0.0, im = 0.0}, 50) == 50;
}

#test "far points escape at once" {
  #assert escape_steps(Complex.{re = 2.0, im = 2.0}, 50) == 0;
}

#test "one escapes on the third step" {
  #assert escape_steps(Complex.{re = 1.0, im = 0.0}, 50) == 3;
}
```

*Listing 2-9: Three tests for `escape_steps` (one is wrong)*

A `#test` block has a name and a body. `#assert` states something that
must be true. The tests don't become part of the program. Instead, the
command `coax test` runs them inside the compiler:

```sh
$ coax test mandelbrot.coax
ok   #test the origin never escapes (1 case, 2966 steps)
ok   #test far points escape at once (1 case, 76 steps)
FAIL #test one escapes on the third step
mandelbrot.coax:49:3: error[E0406]: assertion failed
    #assert escape_steps(Complex.{re = 1.0, im = 0.0}, 50) == 3;
    ^
  note: in #test one escapes on the third step
3 checks, 2 passed, 1 failed
```

The steps in parentheses count how much work the compiler did to run
each test. It has a budget, so a test that loops forever fails instead
of hanging.

Our third test failed. Let's work it through by hand for *c* = 1. The
first step gives *z* = 0² + 1 = 1. The second gives 1² + 1 = 2, which is
exactly 2 from the origin and so hasn't escaped yet. The third gives
2² + 1 = 5, which has. So *c* = 1 does escape on the third step. But
`escape_steps` returns `step`, and the loop counts steps from 0, so the
third step is step 2. The test was wrong, not the code: we were counting
from 1 and the loop counts from 0. That's the most common bug there is,
and a test caught it in seconds. Change the `3` to `2`:

```sh
$ coax test mandelbrot.coax
ok   #test the origin never escapes (1 case, 2966 steps)
ok   #test far points escape at once (1 case, 76 steps)
ok   #test one escapes on the third step (1 case, 194 steps)
3 checks, 3 passed, 0 failed
```

`coax run` and `coax build` don't run tests; they leave them out. That
keeps them free in the program you ship. Chapter 10 shows how to make
the compiler run them on every build for code where mistakes are
expensive, like the OpenAgents wallet.

Here's the finished program, all in one place:

Filename: mandelbrot.coax

```jai
#import "Basic";

WIDTH :: 64;
HEIGHT :: 24;
PALETTE :: " .:-=+*#%@";

Complex :: struct {
  re: f64;
  im: f64;
}

add :: (a: Complex, b: Complex) -> Complex {
  return Complex.{re = a.re + b.re, im = a.im + b.im};
}

square :: (z: Complex) -> Complex {
  return Complex.{re = z.re * z.re - z.im * z.im, im = 2.0 * z.re * z.im};
}

length_squared :: (z: Complex) -> f64 {
  return z.re * z.re + z.im * z.im;
}

// The part of the plane we draw, and how hard we look.
View :: struct {
  center: Complex = Complex.{re = -0.6, im = 0.0};
  width: f64 = 3.2;
  max_steps: int = 50;
}

escape_steps :: (c: Complex, max_steps: int) -> int {
  z: Complex;
  for step: 0..<max_steps {
    z = add(square(z), c);
    if length_squared(z) > 4.0 { return step; }
  }
  return max_steps;
}

#test "the origin never escapes" {
  #assert escape_steps(Complex.{re = 0.0, im = 0.0}, 50) == 50;
}

#test "far points escape at once" {
  #assert escape_steps(Complex.{re = 2.0, im = 2.0}, 50) == 0;
}

#test "one escapes on the third step" {
  #assert escape_steps(Complex.{re = 1.0, im = 0.0}, 50) == 2;
}

render :: (view: View) {
  // A character cell is about twice as tall as it is wide.
  height := view.width * HEIGHT / WIDTH * 2.0;
  left := view.center.re - view.width / 2.0;
  top := view.center.im + height / 2.0;

  line: [WIDTH] u8;
  for row: 0..<HEIGHT {
    y := top - (f64(row) + 0.5) * height / HEIGHT;
    for col: 0..<WIDTH {
      x := left + (f64(col) + 0.5) * view.width / WIDTH;
      steps := escape_steps(Complex.{re = x, im = y}, view.max_steps);
      i := steps * (PALETTE.count - 1) / view.max_steps;
      line[col] = PALETTE[i];
    }
    print("%\n", cast(string) line[..]);
  }
}

main :: () {
  whole: View;
  render(whole);

  bulb := View.{center = Complex.{re = -1.25, im = 0.0}, width = 0.5, max_steps = 200};
  print("\nzoomed in on %\n", bulb.center);
  render(bulb);
}
```

*Listing 2-10: The finished Mandelbrot renderer*

## Summary

That was a lot of ground for one small program. We used:

- constants like `WIDTH :: 64;` and variables declared with `:=`, with
  types inferred from their values;
- `for` loops over ranges like `0..<HEIGHT`, and `if`/`else`;
- procedures with parameters and return types, declared in any order;
- explicit conversions like `f64(row)`, because Coax never converts
  between number types by itself;
- structs with fields, defaults and struct literals;
- strings, string indexing and slicing, and their checked bounds;
- a fixed array, a view of it with `[..]`, and `cast(string)`;
- `print` with values of many types, including whole structs;
- `#test` and `#assert`, run by `coax test`.

We also met three kinds of mistakes. A type error (E0350) stopped the
program before it ran. An out-of-bounds slice stopped it while it ran,
with the line that did it. And a failing test showed that our own
expectation was wrong.

The rest of Part I and Part II go back over all of this carefully.
[Chapter 3](ch03-common-concepts.md) covers variables, types,
conversions, procedures and control flow.
[Chapter 4](ch04-reading-errors.md) is about reading errors like the
ones we saw. [Chapter 5](ch05-structs-enums-unions.md) covers structs
and their relatives. [Chapter 6](ch06-memory.md) covers arrays, views
and strings. [Chapter 9](ch09-compile-time.md) shows what else the
compiler can run while compiling, and [Chapter 10](ch10-writing-checks.md)
is all about `#test`, `#assert` and their stronger cousins.

> Note: The repository has a slightly different Mandelbrot program in
> [`coax/examples/mandelbrot.coax`](https://github.com/OpenAgentsInc/openagents/tree/main/coax/examples/mandelbrot.coax), and a
> Game of Life in [`coax/examples/life.coax`](https://github.com/OpenAgentsInc/openagents/tree/main/coax/examples/life.coax).
> Both make good reading once you've finished this chapter.
