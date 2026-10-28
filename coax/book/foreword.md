# Foreword

Coax started from a frustration that anyone who has worked on a game
engine will recognize. The code that runs every frame has to be fast, so
it ends up in C or C++. But C++ is slow to build, hard to change safely,
and keeps important facts out of sight: how big a struct really is, which
thread touches which data, whether two builds of a simulation will give
the same answer. The engine behind the Verse, which we now call Metareal,
went through Rust and then C++ before we decided to try something else.

The something else draws heavily on Jonathan Blow's Jai, which showed
that a systems language can compile quickly, run arbitrary code at
compile time, and still feel small. We changed it for our situation.
Most of our code is written by agents, so the compiler's errors need
stable codes and concrete fixes. Our simulations need to replay exactly,
so floating point and iteration order need rules. Our data crosses a
public protocol, so types need to carry field numbers and limits. And we
wanted to keep the parts of our native stack that already work, the Swift
host and the Metal renderer, and move pieces into Coax one at a time.

That is how Coax is being built. The compiler today, `coax0`, is small.
It turns a checked subset of the language into C11 that you can read,
and the iPhone app already runs real Coax: the home screen's scene and
HUD, the Grid's camera and movement, shader math, and the rules for
paying and receiving bitcoin in the wallet. Much of the rest of the
language exists only in the specification. This book keeps the two
apart. When it shows output, that output came from the compiler. When it
describes something that isn't built, it says so.

We hope you'll read it with a terminal open.

*OpenAgents, October 2026*
