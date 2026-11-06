# 19. Builds and hot reload

> **Status: partly implemented.** The build commands and the iOS app's build run in
> `coax0`. Phase/declaration reports, split C emission and the Wasm step cache
> run today. `build.coax`, general startup budgets and hot reload remain planned.

So far we've mostly run single files with `coax run`. Real Coax code
doesn't live alone. In the OpenAgents iOS app, Coax sources
become C libraries with headers and combined Metal shader source, which
Xcode compiles and links next to Swift. In this chapter we'll look at
how that build works today: the commands that turn a Coax file into
something another toolchain can use, a small build script of our own,
and the real script the iOS app runs on every build. Then we'll look at
what's planned, a build written in Coax and hot reload, and why hot
reload is mostly an engine problem rather than a compiler one.

> Note: Everything up to "Planned: the build as a program" works today
> with the current `coax0`. `build.coax`, a resident compiler and hot reload
> don't exist yet; they are stages 4 and 8 of the plan in
> [Chapter 28](ch28-road-ahead.md).

## The commands that make things

`coax0` is a single-shot compiler. Each run reads one top-level file,
checks it, and writes one kind of output. Here are the commands that
produce something, all of which you've seen in passing:

| Command | What you get |
| --- | --- |
| `coax run FILE` | Builds a temporary executable, runs it, deletes it |
| `coax build FILE [-o out]` | An executable |
| `coax lib FILE [-o out.o]` | An object file to link into another program |
| `coax header FILE [-o out.h]` | A C header for the file's `#export_c` and `#foreign` procedures |
| `coax c FILE` | The generated C, printed |
| `coax c FILE --library --header out.h` | The C for a library on stdout, and its header |
| `coax metal FILE` | Metal Shading Language, printed (Chapter 17) |

Three options adjust them. `--release` builds with `-O2` and makes
integer overflow wrap instead of trapping (Chapter 16 showed why that
matters). `--keep-c` keeps the intermediate C file that `build` and
`lib` normally delete. And the environment variable `COAX_CC` picks the
C compiler; without it, `coax0` uses `cc`.

```shell
$ coax build hello.coax --keep-c
$ ls
hello       hello.c     hello.coax  hello.dSYM
$ ./hello
Hello, world!
```

`build` and `lib` are thin. They write the generated C, then run one
command, which you could type yourself:

```shell
cc -std=c11 -w -O0 -g -o hello hello.c -lm
```

That's the debug form; `--release` swaps `-O0 -g` for `-O2`, and `lib`
adds `-c` and leaves off `-lm`. The `.dSYM` folder is the debug
information macOS keeps beside a `-g` executable.

If the C compiler fails on code `coax0` generated, that's always the
compiler's bug, not yours, and it says so:

```shell
$ COAX_CC=false coax build hello.coax
coax: the C compiler failed on generated code (exit 1); this is a coax bug
```

(Here we pointed `COAX_CC` at `false`, a program that always fails, to
see the message.)

## One library, two consumers

Let's build something with more than one output. In Chapter 17 we wrote
`examples/lib/ch17_glow.coax`, a light library for a Metal shader. The
same file is ordinary Coax, so a C program can call it too. We'll write
a tiny C host that draws one glowing dot as text, then a script that
builds both the CPU and GPU sides.

Here's the host:

Filename: examples/lib/ch19_preview.c

```c
/* A C host for ch17_glow.coax: draw one glowing dot as text. */
#include <stdio.h>
#include "ch17_glow.h"

int main(void) {
  const char *ramp = " .:-=+*#%@";
  for (int row = 0; row < 11; row++) {
    for (int col = 0; col < 23; col++) {
      float x = (col - 11) / 11.0f, y = (row - 5) / 5.0f;
      float c = glow_tone(glow_dot(x, y, 2.0f));
      putchar(ramp[(int)(c * 9.99f)]);
    }
    putchar('\n');
  }
  return 0;
}
```

*Listing 19-1: A C program that calls the Coax light functions*

It walks an 11-by-23 grid of characters (characters are about twice as
tall as they are wide, hence the different counts), maps each one to a
point on the quad from -1 to 1, asks Coax for the light there, and picks
a character from a ramp of increasingly dense ones.

The header it includes doesn't exist yet. The build script makes it:

Filename: examples/lib/ch19_build.sh

```shell
#!/usr/bin/env bash
# Build ch17_glow.coax for both of its consumers, then run the C preview.
set -euo pipefail
cd "$(dirname "$0")"
coax=../../../bin/coax
out=${TMPDIR:-/tmp}/coax-glow
mkdir -p "$out"

# The CPU side: a header and an object file for C, C++ or Swift.
"$coax" header ch17_glow.coax -o "$out/ch17_glow.h"
"$coax" lib ch17_glow.coax --release -o "$out/ch17_glow.o"
${COAX_CC:-cc} -std=c11 -O2 -I"$out" -o "$out/preview" ch19_preview.c "$out/ch17_glow.o" -lm

# The GPU side: the same procedures as Metal, then the hand-written stages.
"$coax" metal ch17_glow.coax > "$out/Glow.metal"
cat ch17_stage.metal >> "$out/Glow.metal"

"$out/preview"
```

*Listing 19-2: Building one Coax file for C and for Metal*

```shell
$ examples/lib/ch19_build.sh


       .........
     ..:-==+==-:..
    .:-=*#%%%#*=-:.
   ..:=+#%@@@%#+=:..
    .:-=*#%%%#*=-:.
     ..:-==+==-:..
       .........


```

There's our dot, drawn by the same `glow_dot` the fragment shader calls.
The two blank rows at the top and bottom are where the quad's edge fades
to nothing, as the `#forall` checks promised.

Notice what the script does and doesn't do. Coax produced a header, an
object file and Metal text. Everything else, compiling the host,
linking, and joining the shader source, is ordinary shell and the
platform's own tools. Coax doesn't try to own the build; it produces
inputs for one.

### What's in the header

Here's the header `coax header ch17_glow.coax` wrote, lightly trimmed:

```c
/* Generated by coax0 from ch17_glow.coax. Do not edit: change the .coax file.
   Coax int is int64_t; a Coax string or view is a pointer and a count, not
   NUL-terminated; a failed check inside a call aborts the process. */
#ifndef COAX_CH14_GLOW_H
#define COAX_CH14_GLOW_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
#define _Bool bool
// ...
extern "C" {
// ...
#endif
// ...
float glow_dot(float x, float y, float light);
float glow_tone(float light);
// ...
#endif
```

*Listing 19-3: The generated header for the light library*

The comment at the top is for whoever calls this code from C, C++ or
Swift. It gives the three facts that most often cause trouble at a
language boundary: how big `int` is, that Coax strings aren't
NUL-terminated, and that a failed check inside a call ends the process
rather than returning an error. The `extern "C"` block means C++ can
include the header unchanged, and Swift reaches the same declarations
through a bridging header. Only the `#export_c` procedures appear; the
helpers `lo`, `hi`, `sat` and `smooth` stay private to the library.
[Chapter 20](ch20-interop.md) goes much further into calling across this
boundary.

## In the real world: the iOS app's build

`apps/openagents-ios/generate.sh` generates the app's Coax libraries,
headers and Metal source before Xcode compiles the platform shell. Read the
script in the current checkout for its complete module list; that list grows
as engine modules move into Aethyr.

The generator rebuilds when compiler inputs are newer and checks release
metadata before generation.
A checked build can come from the compiler cache or a verified published
build; otherwise `coax/build.sh` bootstraps it. Xcode's iOS SDK environment
must not turn that host compiler into an iOS executable. The script selects
the Mac SDK and host triple for compiler production, then Xcode compiles the
generated C for the actual app target.

A library generation uses `coax c --library --header`: C goes to stdout,
the header to the named file. The generator publishes complete outputs and
keeps the generated headers in the Swift bridging interface. Metal emission
supplies shader source to the host's platform stages. Generated module names,
C types and defaults are part of the host boundary; chapter 20 covers it.

### Checks that stop the build

`#authority;` marks rules that guard money, admission or policy. Compiling an
authority requires all its checks to pass. A matching cached passing result
can discharge that obligation, but a changed input invalidates it. Without
`#authority`, ordinary `#test` bodies wait for `coax test`.

For example, `verse/wallet.coax` guards payment admission. Its generated
library cannot be replaced after a failed check. Code generation is not
permission to send a payment; the runtime still applies the wallet's rules.

From the command line, the iOS shell's build entry is:

```shell
apps/openagents-ios/build.sh
```

That command builds for the simulator. The app's README gives its targeted
tests and signing workflow. This chapter's compiler reports do not measure
installation, device startup or GPU pipeline creation.

## Measure the whole loop

How long does all of this take? Here's the generate step on its own, the
earlier six-library source set plus one Metal module, on an Apple M5 Max running macOS
26.4, three runs:

```shell
$ cat gen.sh
for n in home player grid cast camera wallet; do
  coax/bin/coax c verse/$n.coax --library --header /tmp/$n.h > /tmp/$n.c
done
coax/bin/coax metal verse/light.coax > /tmp/light.metal
$ for i in 1 2 3; do /usr/bin/time -p bash gen.sh 2>&1 | grep real; done
real 0.09
real 0.09
real 0.09
```

That's about 28,000 lines of C and Metal from roughly 4,300 lines of
Coax. Most of it is `Home.c`, 22,615 lines, nearly all of them the
generated protocol codecs from Chapter 18; `coax c verse/home.coax`
takes about 0.04 seconds by itself. Our glow build script in Listing 19-2,
which also runs the C compiler and links, takes about 0.22 seconds.

That measurement predates the Haunted House, its buffer gate and HUD module,
its second Coax Metal module, and the voice libraries. It is not a current whole-app generation
baseline. Chapter 12 describes how to collect one, and
[chapter 41](ch41-compile-budgets.md#what-you-can-measure-today) shows the
per-phase report (`--timings-table`) and the kept baseline for the biggest apps,
`coax/perf/baseline.json`.

Those numbers are real, but they measure the easy part. The loop you
actually wait on, from saving a file to seeing the change on screen,
also includes:

1. parsing, checking and compile-time evaluation (in the numbers above);
2. C compilation, optimization and linking by Xcode;
3. asset processing;
4. installing and launching the app, and compiling the Metal shader
   source at startup (Chapter 17);
5. for the browser: Wasm processing, compression, transfer and
   instantiation.

Only the first step is measured here. A fair measurement names the
hardware, the revision and the workload, runs more than once, and
reports each stage separately, because a fast first stage can hide a
slow fourth one. Jai's best-known numbers are clean rebuilds of a single
project, and in one reported build a third of the time was the front end
and two thirds was LLVM. The number that matters is edit to visible
result, and for the iOS app nobody has measured it yet.

## Planned: the build as a program

> Note: Not implemented. `build.coax` is stage 4 of the plan (spec §16.1).

In Jai, the build is a Jai program: the compiler runs a metaprogram that
creates workspaces, sets options and reacts to compiler messages. Coax
plans the same thing with a file called `build.coax`:

```jai
#import "Build";

build :: () {
  w := Build.workspace("verse");
  w.options.targets = .[.macos_arm64, .ios_arm64];
  Build.add_files(w, "src/**/*.coax");
  Build.add_c_library(w, "third_party/stb", headers = "stb_image.h");
  Build.emit_xcode_sources(w, "apps/openagents-ios");
}
```

Compared with Listing 19-4, this would replace the shell loop with a
typed description of the same work. The plan adds three things a team
build needs on top of Jai's model.

Capabilities. Build code runs on the build machine, separately from the
code being built, and reading files, running processes or touching the
network needs a declared permission. Pure `#run` code never inherits
those permissions; as Chapter 9 showed, it has no I/O at all.

A recorded set of inputs. A reproducible build records the compiler's
version and hash, the target description, the toolchain versions, the
hashes of sources and modules, the options, and every file it read.
Directory listings are sorted, environment variables are read only from
an allowlist, and clocks and randomness are absent by default. The
`coax-proto-v4` fingerprint line we saw in `Home.c` is a small piece of
this that exists today.

Generate before you replace. `build.coax` would first generate the
Xcode and CMake source lists, and the existing `project.yml`, Xcode and
Emscripten paths stay until the generated output matches them and real
native and browser builds pass.

It's also worth saying what a programmable build is not. It doesn't come
with dependency resolution, lockfiles, remote caching, hermetic
execution or artifact signing. Each of those is its own system. Today,
for example, `coax0` doesn't provision or pin the C compiler; it uses
whatever `COAX_CC` or `cc` names.

## Measure and cache the browser build

Compiler reports and build caching run today. Use `--timings FILE` (or
`--timings-json FILE`) with `--timings-top N` to retain phase costs and the
most expensive declarations. `--timings-table` prints the same observations
as a table. [Chapter 41](ch41-compile-budgets.md) gives commands and recorded
output, including counted work when the clock alone does not explain a cost.

The browser driver, `coax/web/wasm.sh`, caches Coax-to-C, C objects, linking
and Wasm optimization separately. It writes a JSON report beside the output
under `.c/`, or at `COAXIAL_WASM_REPORT`. Each step says `hit`, `miss` or
`off`, its duration and, when the compiler ran, its compiler report.
Changing a Coax file need not rebuild an unchanged C object. Changing a
header invalidates each object that includes it. The toolchain, options and
input identities belong to the keys.

For large library units, `coax c FILE --library --split N --split-unit BYTES`
emits a stream of C parts and a shared prelude. The Wasm driver separates
them and caches their objects independently. Renderer carriers default to
up to 16 parts; `COAXIAL_SPLIT=0` disables splitting. The threshold defaults
to 512 KiB of procedure C, and a small unit stays whole. Coax still checks
the graph; this reduces repeated native compilation, not semantic work.

A capacity refusal now reports its limit, top users and a suggested fix.
`#traversal N` budgets aggregate-copy work for a check; `--traversal N` sets
the default. Near-limit notes appear before exhaustion, at 75 percent by
default (`--near-limit P` changes the threshold). Read the report before
raising a limit: passing a view or only the required fields can remove the
copy that consumed it. Chapter 41 demonstrates both a refusal and a note.

R22's five general kernel/startup budgets remain planned: first compile,
cached load, scene rebuild, parameter edit and structural edit. Those gates
need their own target and consumer evidence. The browser build's measured
cache does not establish a universal kernel cache or a startup deadline.

## Hot reload is an engine design

> Note: Not implemented. Nothing in `coax0`, `verse/` or the iOS app
> reloads code today. A resident compiler and reload are stage 8 of the
> plan (spec §16.3).

Fast code generation makes hot reload tempting: change a light function,
see the new glow without
restarting. But a fast compiler only produces new code quickly. Swapping
that code into a running program safely needs a list of things, and
compile speed provides none of them:

- A resident host that owns persistent storage, platform services,
  queues and GPU resources, and is never reloaded itself. In the iOS app
  that's the Swift and C side: `host.c` already owns the buffers the
  Coax code draws into, which is the right shape.
- A versioned C interface to the reloadable code, so the host can tell
  whether a new library is one it knows how to call. The generated
  headers are a start, but they carry no version today.
- A safe point. Workers stop, and no stack frame or callback from the
  old code can still be running.
- Stable procedure IDs. Anything that stores a reference to code stores
  an ID into a table, never a raw address that goes stale when the
  library is replaced.
- Transactional migration. If a persistent struct's layout changed,
  allocate new storage, migrate field by field with checks, validate
  handles and resources, then publish atomically, or roll back and keep
  the old state. Compatibility is never inferred from matching field
  names. This is the fourth row of the version table in Chapter 18.
- GPU resources that survive, with pipelines rebuilt between frames, as
  Chapter 17 described.

A resident compiler that knows which declarations changed, and only
recompiles those, is part of the same stage. `coax0` isn't incremental:
every run starts from nothing.

### iOS doesn't load code

There's one more limit, and it's not Coax's to remove. iOS doesn't allow
a signed app to load new code at run time, and Coax doesn't try to get
around that. When reload exists, it will run on a Mac, where the same
engine code can run natively against Metal. Builds for a device stay
fixed, signed images, built the way this chapter described.

## Summary

Today's Coax build is a set of small, single-shot commands, each writing
one kind of output: executables, object files, C, headers and Metal. A
build script, ours in Listing 19-2 or the iOS app's `generate.sh`,
chains them and hands the results to the platform's own tools, with
`#authority` files stopping the build when their checks fail. Phase and
declaration reports measure the compiler; the Wasm driver
reports and caches native steps independently, including split units. A build
written in Coax, a resident compiler and hot reload are planned, and hot reload
especially depends on engine machinery (safe points, stable IDs,
transactional migration) that no compiler can supply.

In the next chapter we'll look closely at the boundary we've been
crossing all along, between Coax and C, C++ and Swift.
