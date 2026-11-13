# Episode 296 show notes: The Coax Programming Language

Drafted 2026-11-13 for Episode 296, after
[Episode 294, *Bitcoin From Scratch*](294-show-notes.md) and
[Episode 295, *Nostr From Scratch*](295-show-notes.md). This is a talk
outline, not a script. The lines are written to be said out loud, in the
voice of Episodes 290, 293, 294 and 295. Read them, change the words, and
say them your own way.

Episodes 294 and 295 each walked a world. This one has a world too: **Coax
Lab is built** (`/worlds/coax-lab`, unlisted, on `main`, not deployed yet).
[Section 7](#7-coax-lab) lists its seventeen stations.
[Section 6](#6-the-terminal-tour) is a tour you can record in a terminal:
every command in it was run on 2026-11-13 with `coax0` 0.52.0.
[Check before recording](#check-before-recording) lists what to decide.

## The short version

- **The idea:** the last two episodes said "written in Coax" a dozen times
  and never said what Coax is. This episode does.
- **The line:** agents write most of our code now. So we made a language
  for the reader of the compiler's output to be an agent.
- **What it is:** a small systems language in the Jai family. It compiles
  to C you can read, runs natively and in the browser, and has its tests
  built into the language.
- **The proof:** the game engine, Everglade, the website, Bitcoin and
  Nostr are all written in it.
- **The tour:** hello world, an error with a fix, a check, a planted bug
  caught, then Coax Lab.
- **The ask:** read the book, point your agents at it, write something
  small, tell us where it breaks.

## 1. Cold open: the word we keep saying

> Last two episodes: Bitcoin from scratch, Nostr from scratch. And I kept
> saying "in Coax, in Coax, in Coax." So what is Coax?
>
> Coax is our programming language. We wrote our own. Yes, really.

On screen: the repo tree, then a filter on `*.coax`. The count on
2026-11-13 is 7,448 files and about 1.5 million lines.

> Three weeks ago this language didn't exist.

The fact behind the line: version 0.1.0 is dated 2026-10-27. The current
compiler is 0.52.0, dated 2026-11-06.

## 2. Where it came from

Episode 290 told this story at 14:00. Tell it again, shorter.

> We built the engine in Rust. Then we rebuilt it in C++. Both times the
> same problem: slow builds, and the important facts are hidden. How big
> is this struct? Does this add overflow? Will this run the same way twice?
>
> There's a game developer, Jonathan Blow, who spent ten years on a
> language called Jai for exactly this kind of code. It's a closed beta.
> So we told the agents: read everything public about it, and make us our
> own.

Then the honest part:

> Coax isn't Jai. It starts from Jai's design and changes it for our
> situation: agents write the code, simulations have to replay exactly,
> and the same code has to run on a phone, a server and in a browser tab.

If you know Jai, the syntax carries over: `name :: value`, `:=`, `#run`,
`defer`, `print` with `%`. [Appendix C](../../coax/book/appendix-c-jai.md)
lists what changed, side by side.

## 3. What it looks like

Show code early. Two snippets, both real and both in the book's
introduction.

The camera, to show it reads like C without the noise:

```jai
// How far through the dolly `now` is, 0 to 1.
progress :: (d: Dolly, now: f64) -> f32 {
  if !(d.length > 0) { return 1; }
  return clamp01(f32((now - d.start) / f64(d.length)));
}
```

> You can read that. A name, two colons, the arguments, what it returns.
> The conversions between number types are written out, because Coax never
> converts on its own.

The wallet, to show checks:

```jai
#authority;

// All the bitcoin there will ever be, in satoshis.
MAX_SAT :: 2_100_000_000_000_000;

verse_wallet_sat_to_msat :: (sat: int) -> int #export_c {
  if sat < 0 || sat > MAX_SAT { return -1; }
  return sat * 1000;
}
```

> See that word at the top, authority? That one line means: every
> procedure in this file must be tested, or the file doesn't compile.

## 4. Why a language for agents

This is the centre of the episode. Talk to two people: someone who doesn't
code, and a developer who'll ask why not Rust, Zig or Odin.

> Most of our code is written by agents. An agent edits a file, runs the
> compiler, reads what it printed, and decides what to do next. Whatever
> the compiler prints is the agent's whole view of whether the code is
> right. So we designed the language around that loop.

Five points. Each has a station in Coax Lab and a step in the terminal
tour.

- **One way to write each thing.** One declaration form, named fields,
  conversions written out. Agents copy what they see, so there is one
  thing to see.
- **Nothing happens off the line you're reading.** No implicit
  conversions, no shadowing, no constructors, no exceptions, no garbage
  collector, no preprocessor.
- **Errors come with a code and a fix.** Every error has a stable code, the
  source line and a fix line, and the same thing as JSON. `coax explain`
  describes any code.
- **Mistakes stop the program.** Integer overflow, an index past the end,
  a bad cast and division by zero stop with the file and line. In C the
  same mistakes are undefined behavior.
- **Tests are part of the language.** `#test` is one case. `#forall` runs
  every combination of its inputs. `#authority` refuses a file with an
  untested procedure. `coax mutate` plants bugs one at a time and reports
  any that no test catches.

For the technical listener:

- **It compiles to C11.** One readable C file a program, built by any C
  compiler. So it has C's cost model: plain structs, plain calls, nothing
  allocated behind your back.
- **It runs code while compiling.** `#run` builds tables and checks facts
  at compile time, with a step budget so it always ends.
- **One source, several targets.** Native through C, the browser through
  WebAssembly, and shader math as Metal, WGSL, SPIR-V or CUDA from the
  same checked source.
- **It sits beside C, C++ and Swift.** `#export_c` writes the header.
  `#foreign` calls a C function. You move one file at a time.
- **Why not Rust:** Rust's promise is memory safety through a borrow
  checker. Coax doesn't make that promise. It checks the specific things
  that go wrong in our code, and it aims to build much faster.

> Jai is already in the models' training data, so they pick Coax up fast.
> And when they get it wrong, the compiler tells them the fix.

## How it compares, measured

You'll be asked "is it faster than C, C++, Rust, Jai?" These are the
numbers, measured on 2026-11-13 on the 18-core Mac, with `coax0` 0.52.0,
Apple clang and rustc 1.94.1. The machine was busy with other builds (load
10 to 20), so each number is the best of three or more runs. Treat them as
ratios, not absolutes. The programs and the timing script are in
[296/](296/) (`bench-gen.py`, `bench-time.py`).

**The short answer to say out loud:**

> Is it faster than C? No. It compiles to C, so the best it can do is tie,
> and it nearly does: about 5 to 10 percent behind, with every safety check
> still on. Faster than Rust at running? About the same. Faster than Rust
> at telling you whether your code is right? Yes, about ten times.

### How fast the program runs

One program in three languages: 60 rounds of a prime sieve and a CRC-32
over a million bytes, then a 2,000 by 1,000 Mandelbrot. All three print the
same answer.

| Build | Run time |
| --- | --- |
| C, `clang -O2` | 0.58 s |
| C, `clang -O2 -ffp-contract=off` | 0.63 s |
| **Coax, `--opt 2`, all checks on** | **0.63 s** |
| Coax, `--release` (wrapping integers) | 0.63 s |
| Rust, `rustc -O` | 0.71 s |
| C, `clang -O0` | 1.37 s |
| Rust, debug | 2.06 s |
| Coax, default (checked, `-O0`) | 2.50 s |

What to take from it:

- **Checks are close to free here.** Coax with overflow and bounds checks
  on runs as fast as Coax with wrapping integers.
- **Coax ties C once C computes the same answer.** Plain `clang -O2` is 8%
  faster because on this Mac it fuses a multiply and an add into one
  instruction, and that changes the result: it counts 421,082 points inside
  the set where Coax, Rust and C without fusing all count 421,080. Coax
  gives up that speed so the answer is the same everywhere.
- **The default Coax build is slow.** It's unoptimized and fully checked,
  about four times slower than `--opt 2`. Say so.
- **This is one small program.** It isn't our signature code, where we're
  about 1.4 times behind libsecp256k1 (measured 2026-11-13).

### How fast it compiles

One generated file in each language: 3,000 small procedures with loops and
arrays and 100 callers, about 30,000 lines, compiled to an object file.

| Step | Coax | C | C++ | Rust |
| --- | --- | --- | --- | --- |
| Check only, no code generated | **0.11 s** | 0.09 s | 0.43 s | 1.1 to 1.4 s |
| Debug build | 2.4 s | 0.40 s | 0.82 s | 2.2 s |
| Optimized build | 13.1 s | 16.7 s | 14.0 s | 14.0 s |

At ten times the size (304,000 lines), check only: Coax 0.94 s, C 0.84 s,
C++ 1.5 s, Rust 11.7 s.

What to take from it:

- **Checking is where Coax wins.** About 300,000 lines a second, the same
  as a C compiler parsing C, and about ten times faster than `rustc`
  type-checking the same program. That's the loop an agent lives in: edit,
  check, read the error.
- **A full build is not faster than Rust on this test.** Coax writes C and
  then waits for clang. Its debug C carries every check, so clang has more
  to chew on (109,000 lines of C from 30,000 of Coax).
- **Optimized builds are all the same**, because all four end in the same
  LLVM optimizer.
- **This test is kind to Rust.** It has no generics, no macros, no
  dependencies and no linking, which is where our Rust builds actually went
  slow. We can't measure the old Rust engine against the Coax one: the Rust
  code is gone from the repo. So don't quote a number for "our Rust build
  took X"; say it from memory as your own experience.

Two real programs, checked from cold on the same day:

| Program | Size | Parse and type-check | Then run every check it has | Total |
| --- | --- | --- | --- | --- |
| The `openagents` CLI | 298 files, 67,000 lines | 0.5 s | 6.6 s | 7.6 s |
| Everglade's page | 217 files, 97,000 lines | 0.8 s | 20.4 s | 21.2 s |

> Under a second to know the whole game type-checks. The rest of the time
> is the compiler running the tests, which you'd have to run anyway.

### Against each language, in one line each

| Language | Where Coax is better | Where it's worse |
| --- | --- | --- |
| **C** | Overflow, bounds and bad casts stop with a line number; no headers or preprocessor; tests and compile-time code in the language | Never faster; a far smaller ecosystem; three weeks old |
| **C++** | Small enough to read whole; one way to write things; checks 4 times faster on the test above | No classes, templates or libraries; if you need those, it's not for you |
| **Rust** | Checks about 10 times faster; simpler to read and to generate; tests enforced by the compiler | No borrow checker, no memory-safety promise, no threads, no package manager |
| **Jai** | It's open and you can use it today; errors with codes and fixes; `#authority` and mutation testing | Jai has its own fast backend, pointers, a context, macros and ten years of work. We can't measure it: its compiler isn't public |
| **Zig, Odin** | Not measured. Don't compare on air. | |

## 5. What we built with it

> A language is only real if you ship something in it. Here's what's in
> Coax right now.

On screen: one folder a line, with its file count (2026-11-13).

| Folder | What | `.coax` files |
| --- | --- | --- |
| `coax/` | The compiler's own lexer, parser and type checker, the libraries, the web framework | 2,015 |
| `psionic/` | Machine learning from scratch | 1,600 |
| `metaverse-engine/` | The game engine | 1,479 |
| `apps/` | openagents.com, drawn in Coax in the browser | 555 |
| `metaverse/` | The worlds and what they share | 321 |
| `bitcoin/` | Bitcoin from scratch (Episode 294) | 234 |
| `nostr/` | Nostr from scratch (Episode 295) | 213 |
| `everglade/` | Everglade, the MMORPG (Episode 293) | 211 |

> The website you're looking at is Coax. The game is Coax. The Bitcoin
> code is Coax. The compiler is partly written in itself.

"Partly" is the accurate word. The lexer, parser and type checker are
Coax. The compile-time evaluator and the C emitter are still C++.

## 6. The terminal tour

Record in a terminal, full screen, large type. Each step is one command.
The output shown is what the command printed on 2026-11-13.

| Step | What you type | What it prints | What to say |
| --- | --- | --- | --- |
| 1. Build the compiler | `coax/build.sh` | `installed the checked build` | "One script. The compiler is small." |
| 2. Hello | `coax run hello.coax` | `Hello, Coax!` in about a quarter of a second | "Compile and run. That includes the C compiler." |
| 3. The C | `coax c hello.coax` | 183 lines of C11 | "It's just C underneath. You can always read what it made." |
| 4. An overflow | `coax run over.coax` (a `u8` at 250, add 1 ten times) | `over.coax:4: integer overflow in '+'` | "In C that wraps around to 4 and keeps going. Here it stops and tells you the line." |
| 5. An error with a fix | `coax check shadow.coax` | `error[E0303]: 'count' is already declared at line 3; Coax does not allow shadowing` then `fix: pick another name` | "A code, the line, and the fix. That's written for an agent." |
| 6. The same as JSON | `coax check shadow.coax --json` | One JSON object with `code`, `line`, `message`, `fix` | "And this is what the agent actually reads." |
| 7. Explain | `coax explain E0303` | `name already declared; no shadowing`, then a paragraph | "Every code has an explanation built in." |
| 8. Compile time | `coax run coax/examples/crc32.coax` | `crc32("123456789") = 3421780262 (computed while compiling)` | "That table was built by the compiler, running my code, before the program existed." |
| 9. A layout | `coax layout coax/examples/pulse/pulse.coax` | `Pulse size 16 align 4`, then each field's offset | "Every byte of every struct. Nothing hidden." |
| 10. Checks | `coax test coax/examples/money.coax` | `10 checks, 10 passed`, one of them 60,000 cases | "Not one example. Every combination in the range." |
| 11. Authority | `coax check auth.coax` (an `#authority` file with one untested procedure) | `error[E0380]: 'fee' is not exercised by any #test or #forall` | "Money code with no test doesn't compile." |
| 12. Mutation | `coax mutate coax/examples/money.coax` | `43 mutants, 43 killed, 0 survived` | "It broke my code 43 different ways. My tests caught all 43." |
| 13. Something to look at | `coax run coax/examples/mandelbrot.coax` | A Mandelbrot set in text | A beat to breathe. |

`hello.coax`, `over.coax`, `shadow.coax` and `auth.coax` are four-line
files that aren't in the repo. They are written out under
[Check before recording](#check-before-recording).

Steps 4 to 7 and 10 to 12 are the argument of section 4, shown. If you
cut the tour down, keep 2, 5, 11 and 12.

## 7. Coax Lab

The 294 and 295 format: a world on the Grid at
`openagents.com/worlds/coax-lab`, unlisted, walked with your Everglade
character, built on the same lab walk as Bitcoin Lab and Nostr Lab
(`metaverse/lab-walk.coax`). You come in on a plaza. Five avenues leave
it, one a district, and the stations follow the book.

It is on `main` and runs locally. It is not deployed: it goes out with the
next release. To see it now:

```sh
coax/remote/pages.sh origin/main
coax/web/serve.sh apps/openagents-web --listen 127.0.0.1:9296
# then open http://127.0.0.1:9296/worlds/coax-lab
```

Every station ends with a line that says where its values come from.
"Live" means the page works the value out as you stand there, with Coax
compiled into the page. "Recorded" means the station shows what `coax0`
0.52.0 printed on 2026-11-13, because the compiler doesn't run in the page.
The source code on the stations is coloured by the compiler's own lexer,
running in the page.

| Station | Name | What it shows | Chapter | Live or recorded |
| --- | --- | --- | --- | --- |
| 1 | A name and a value | `LIMIT :: 64`, `x := 1`, `y: int = 2`, a written conversion and a procedure. Each part lights up in turn with its value | 1, 3 | Live |
| 2 | A procedure | `progress` from the engine's camera, read a part at a time, while the same procedure moves a light along a track | 3 | Live: the engine's own procedure runs |
| 3 | Numbers that stop | Two bytes count up from 250. The checked one stops at 255. The wrapping one goes round and ends at 4 | 3, 7 | Live counters (`add_checked`, `add_wrap`); the line `over.coax:4: integer overflow in '+'` is recorded |
| 4 | A struct, byte by byte | `Pulse` (16 bytes) and the same four fields in two orders: 24 bytes with 10 of padding, 16 with 2 | 5, 13 | Live: `size_of` and `field_info` |
| 5 | An array and a view | Eight cells, a view over four of them, an index stepping along it. One past the view's end is refused, even where the array has a value | 6 | Live reads; the line `index out of bounds` is recorded |
| 6 | Cleanup you can see | Three `defer` lines set aside, then run in reverse. A parser passes an error up with `try` and its cleanup still runs | 7 | Live |
| 7 | Life | Conway's Life: a struct holding an array, copied by value every generation | 5, 6 | Live |
| 8 | The lexer | The compiler's stages, and the first one running: a line of source read into tokens one at a time, each named by its kind | 27 | Live: the compiler's own lexer, written in Coax |
| 9 | Work done while compiling | A CRC-32 table of 256 numbers built by `#run`. The page holds the table and sums a text with it as it is typed | 9 | Live: the table is in the page |
| 10 | The compiler drew this | A Mandelbrot set of 3,128 points, each worked out by the compiler. A probe calls the same procedure now and compares | 9 | Live |
| 11 | An error with a fix | The shadowing error: `E0303`, the place, the caret, the fix line. Then the JSON and `coax explain` | 4, 22 | Recorded |
| 12 | A check | A `#forall` over refunds: 1,974 cases fill a grid in the check's order, all holding | 11 | Live |
| 13 | A planted bug | `<=` becomes `<`. Forty cases go red. Then `43 mutants, 43 killed, 0 survived` | 11 | Live for the planted bug; the `coax mutate` count is recorded |
| 14 | Authority | An `#authority` file with a procedure no check runs: `E0380`. With a check added, it builds | 11, 22 | Recorded |
| 15 | One source, three targets | One procedure as Coax, as the C the compiler writes and as the WGSL it writes. Two panels run it: the page for 728 cells, the GPU for every pixel | 17, 20, 21 | Live panels; the C and WGSL text is recorded |
| 16 | Beside Jai, C and Rust | A constant, a variable, a loop, a conversion and a procedure in each language | Appendix C | From the book |
| 17 | Built with it | The folders from section 5 with their counts, then this world and the number of quads it is drawing | 22, 28 | Recorded counts; the quad count is live |

The districts: START HERE (1 to 3), DATA (4 to 7), THE COMPILER (8 to 10),
FOR AGENTS (11 to 14) and WHERE IT RUNS (15 to 17). INDEX (or M) lists them,
NEXT and PREV (] and [) step through, and every station has an address,
such as `?at=overflow` or `?at=mutant`.

How to walk it on camera:

> This is Coax Lab. Same character, same controls. Every one of these
> panels is a chapter of the book.

Stop longest at 3, 11, 13 and 14. Those four are the "for agents" argument.
Stations 10 and 15 are the ones to hold on for the picture.

The last station is the close: the world you're standing in, the
character you're walking and the page around it are all the language the
panels describe.

To record it without walking, the check that tours it writes a picture of
every station, and every frame with `--frames`:

```sh
coax/web/tool.sh coax-lab-check http://127.0.0.1:9296 OUT_DIR --size 1920x1080 --dwell 7 --frames
ffmpeg -framerate 30 -i OUT_DIR/frames/f%05d.jpg -c:v libx264 -pix_fmt yuv420p coax-lab.mp4
```

Station 17's counts were taken at commit `ed22cd4c22` on 2026-11-13. They
are a little higher than the table in section 5, which was counted earlier
that day (for example 2,020 against 2,015 for `coax/`).

## 8. The book

> There's a whole book. It's free, it's in the repo, and it's Markdown, so
> you can point your agent straight at it.

On screen: the GitBook contents, scrolled once.

- **47 chapters in nine parts, and six appendices.**
- **Parts I and II** (chapters 1 to 12) are the language: getting started,
  reading errors, structs, memory, errors and cleanup, generics, compile
  time, checks, debugging.
- **Part III** (13 to 17): data layout, jobs, determinism, GPU programs.
- **Part IV** (18 to 23): Coax in OpenAgents. Chapter 22 is "Built for
  agents". Chapter 23 says what the checks do not prove.
- **Parts V and VI** (24 to 28): larger programs, how the compiler works
  inside, and the road ahead.
- **Parts VII to IX** (29 to 47): robots and physical simulation,
  simulation compute, and finishing the toolchain. Most of this is
  planned.
- **Appendices C and D** are for Jai and C++ programmers.

> The book says what's built and what isn't. If a chapter shows output,
> the compiler printed it. If it's not built, the chapter says "planned"
> at the top.

Count on 2026-11-13: 21 chapters with no caveat, 10 marked partly
implemented, 16 marked planned.

## 9. What's missing

Say this part plainly, as in 294 and 295.

> It's three weeks old. Here's what it doesn't have.

- **No pointers and no growable arrays yet.** Everything is fixed-size
  storage and views into it. That's why so much of our code says
  "bounded".
- **No threads.** Jobs are designed and not built.
- **Not memory safe the way Rust is.** There are some borrow checks.
  There is no general one, and there won't be.
- **Not self-hosted.** Half the compiler is still C++.
- **No package manager, and one module.**
- **Speed.** Our signature check is about 1.4 times slower than
  libsecp256k1 (measured 2026-11-13; it was about 45 times at Episode 294).
- **Builds aren't fast yet.** Checking is fast. A full debug build is no
  faster than Rust's, because we go through a C compiler
  ([measured above](#how-it-compares-measured)).
- **The tests have limits.** A `#forall` covers the range you gave it and
  nothing else. And an agent that can edit the code can edit the tests.
  Chapter 23 is about exactly that.

> We're not telling you to rewrite your company in it. We're telling you
> it's working for us, and it's open.

## 10. How to help

> If you're a developer, or you've got agents: point them at this.

- **Read the book with a terminal open.** Chapter 1 builds the compiler
  and runs hello world.
- **Write something small.** A parser, a hash, a game of life. Tell us
  where the compiler's error didn't help.
- **Break a check.** Find a bug in an `#authority` file that its checks
  and `coax mutate` both miss.
- **If you know Jai:** read Appendix C and tell us what we got wrong.
- **Add a station to Coax Lab.** One per chapter is the goal.

> The repo's open. PRs welcome.

## 11. Close

> Our own engine, our own Bitcoin, our own Nostr. Underneath all of it,
> our own language, built so agents can write it and we can check it.
> Go read the book. Stay tuned.

End card: `openagents.gitbook.io/the-coax-programming-language`,
`github.com/OpenAgentsInc/openagents/tree/main/coax`, and the Coax Lab
link once it's live.

## What to say by way of introduction

If the talk is one take of about 90 seconds, as 295 was, say these five
things in this order:

1. "We kept saying Coax. Coax is our own programming language."
2. "It's three weeks old, it comes from Jai, and agents wrote it."
3. "It's built for agents: one way to write things, errors that come with
   the fix, and tests in the language that the compiler makes you write."
4. "It's real: the engine, the game, the site, Bitcoin and Nostr are all
   in it."
5. "It's open and there's a book. Point your agents at it."

Footage for each line: the repo filter (1), the changelog's first and
latest entries (2), tour steps 5, 11 and 12 or stations 11, 14 and 13 (3),
the folder table or station 17 (4), the book's contents (5).

## Check before recording

- **Coax Lab isn't deployed.** It is on `main` and runs locally
  ([section 7](#7-coax-lab) has the command). Record it from a local
  server, or wait for the next release so the link works for viewers.
- **What Coax Lab doesn't show.** The compiler doesn't run in the page, so
  errors, `coax mutate` and the generated C and WGSL are recorded text.
  On an upright phone a station is wider than the screen: turn the phone, or
  drag to look along it.
- **The Jai story.** "Closed beta", "ten years" and "in the training
  data" are from Episode 290 and were not checked again. Say "from what's
  public" if you'd rather be careful. Don't say Coax is Jai or is
  compatible with Jai.
- **"Agents wrote it."** True of most of it, reviewed by people (book
  chapter 22). Don't say no person touched it.
- **"Three weeks old."** 2026-10-27 to the recording date. Recount if you
  record later.
- **The counts** (7,448 files, about 1.5 million lines, the folder table)
  are from `git ls-files '*.coax'` on 2026-11-13 and include tests,
  examples and generated compiler seeds. Say "about 7,000 files" out loud.
- **"1.4 times slower"** was measured on 2026-11-13 with
  `coax/tests/bitcoin/secp_bench.sh` (issue #13450). Episode 294 said 45
  times.
- **Timing in step 2.** A quarter of a second was one run on this Mac.
  Run it once before recording so the first run isn't the slow one.
- **The book's own introduction is behind the compiler.** It says generics
  aren't built; chapter 8 and the status table say they partly are. Go by
  the chapter.
- **The four small files for the tour:**

  ```jai
  // hello.coax
  #import "Basic";
  main :: () {
    print("Hello, Coax!\n");
  }
  ```

  ```jai
  // over.coax
  #import "Basic";
  main :: () {
    x: u8 = 250;
    for i: 0..<10 { x += 1; }
    print("%\n", x);
  }
  ```

  ```jai
  // shadow.coax
  #import "Basic";
  main :: () {
    count := 3;
    for i: 0..<2 {
      count := i;
      print("%\n", count);
    }
  }
  ```

  ```jai
  // auth.coax
  #import "Basic";
  #authority;
  fee :: (amount: int) -> int { return amount / 100; }
  main :: () { print("%\n", fee(500)); }
  ```

## Sources

- [The Coax Programming Language, the book](../../coax/book/README.md):
  the [foreword](../../coax/book/foreword.md), the
  [introduction](../../coax/book/introduction.md),
  [chapter 11](../../coax/book/ch11-writing-checks.md),
  [chapter 22](../../coax/book/ch22-agents.md),
  [chapter 23](../../coax/book/ch23-verification-boundaries.md),
  [chapter 28](../../coax/book/ch28-road-ahead.md) and
  [Appendix C](../../coax/book/appendix-c-jai.md)
- [The Coax README](../../coax/README.md), the
  [status table](../../coax/STATUS.md) and the
  [changelog](../../coax/CHANGELOG.md)
- [Episode 290, *Enter the Verse*](290.md): 14:00, the Jai story and the book
- [Episode 294 show notes](294-show-notes.md): what Coax is missing
- The lab walk the labs share: `metaverse/lab-walk.coax`
- Coax Lab: `apps/openagents-web/coax-lab-walk.coax` (the stations),
  `apps/openagents-web/coax-lab-lesson.coax` (their values and checks),
  `apps/openagents-web/realms/coax-lab/index.md` (its page text) and
  `coax/web/coax-lab-check.coax` (the headless tour)
- The examples: `coax/examples/`
