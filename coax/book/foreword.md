# Foreword

This language started with a compile time.

The Verse is OpenAgents' 3D world: houses you change by talking to them,
planets you make by holding your finger on the stars, a payment network
whose pulses you can watch move. We first built it in Rust. Then we tried
building it in C++, in a repository called `aethyr-cpp`, and something
surprising happened. Everything got faster to work on. The engine
compiled in seconds, and the loop of change, build and look tightened
until it felt like play.

Seconds is still too long. Touching the entity system's header,
`src/ecs/ecs.h`, rebuilds in 11.9 seconds of wall time and 80 seconds of
CPU, because every file that includes it parses it again. Our measured
frame budgets are in milliseconds, and the ECS move pass takes 0.06 ms.
The program runs fast, and building the program is slow. That gap is the
reason for this book.

So we asked a question with no budget attached. If we could have any
stack at all, if we could snap our fingers and have the whole Verse
rewritten, what would be fastest? Fastest to run and fastest to change?

The honest answer was a language that already exists, and that we cannot
use. Jonathan Blow showed his Jai compiler rebuilding a game of roughly
300,000 lines from scratch in 2.3 seconds. It lets programs run code at
compile time, inspect the compiler's understanding of their own types,
and write the build itself.
It puts structure-of-arrays layout in the type system, and it hands every
procedure an allocator without making anyone thread it through by hand.
It was designed by people who ship games, for people who ship games.
Its first big game, *Order of the Sinking Star*, shipped on 8 October
2026. The compiler itself is still a closed beta: public binaries are
promised after the game, and source later, with no license yet that we
could plan a company around.

Wanting what you can't have is a good way to learn what you want. We read
the Jai primer, 4,800 lines of real Jai that Forrest Smith wrote for
Advent of Code 2022, and a long technical report on Jai's compiler, build
system and production use. We found that we wanted more than Jai, not
less:

- Most of our code is written by agents, so checks must be on by default
  and errors must say how to fix them.
- Our simulations replay bit for bit across phones and laptops, so
  floating point must be a language rule, not a compiler flag that falls
  off a CMake line.
- Our worlds run jobs across cores, so the language should know which data
  each job touches and refuse races before they happen.
- Our data crosses a public protocol, so field numbers, limits and
  versions belong in the type, not in a separate generator.
- Our renderer speaks Metal straight to the GPU, so shaders should share
  structs with the code that fills their buffers.

Coax is what you get when you take Jai's design seriously and add those
requirements. The name says what it does: it coaxes speed out of hardware,
and correct code out of the people and agents who write it.

It also starts small, on purpose. The first compiler, `coax0`, turns a
handful of the language into C, so the result builds with Xcode and
Emscripten like any other C file. It went from nothing to "Hello, world"
in a day. It will grow one feature at a time, and each feature will earn
its place by shipping in the real app. If a feature doesn't make the
Verse faster to build or safer to change, it doesn't go in. If the
language as a whole stops paying for itself, we will stop. The plan says
when (Chapter 20).

This book grows with the compiler. Part I runs today: every example in it
is compiled and checked. The rest is the design we're building toward,
written down so that people and agents can argue with it before it turns
into code.

*OpenAgents, October 2026*
