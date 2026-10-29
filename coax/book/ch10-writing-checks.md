# 10. Writing checks

A program can compile, run and print plausible numbers while still being
wrong. The compiler catches type errors and out-of-range indexes, but it
can't know that you meant "the first people get the extra sats" when you
wrote code that gives them to everyone. For that, you have to say what
you meant in a form the machine can test. In Coax that form is a
*check*.

Coax has two kinds of check. A `#test` is a block of code that runs once
and asserts things about the results. A `#forall` runs its block once for
every combination of some small inputs, so instead of picking examples
you state a rule and let the compiler try all of them. Both run inside
the compiler, in the same sealed evaluator as `#run` from Chapter 9, and
neither ends up in the program you ship.

In this chapter we'll write checks for a small procedure that splits a
payment between people. We'll start with one example, watch it fail when
we break the code, and then grow it into rules that cover hundreds of
cases. After that we'll look at how to control the way checks run, how to
organize them, what `#authority` adds for code that guards money, and
finally how `coax mutate` checks the checks themselves.

> Note: everything in this chapter runs on `coax0` 0.10.0. The checks
> arrived in 0.5.0 and are specified in `coax/spec.md` (section 10.1.1).

## How to write a check

Let's say we're writing the part of a wallet that splits a bill. Ten sats
between three people can't divide evenly, so someone gets an extra sat.
We'll give the extra sats to the first people, one each: 4, 3 and 3.

Make a file called `split.coax`.

Filename: split.coax

```jai
#import "Basic";

// How many of `total` sats person `i` of `people` gets. The sats that
// don't divide evenly go to the first people, one each.
share :: (total: int, people: int, i: int) -> int {
  base := total / people;
  if i < total % people { return base + 1; }
  return base;
}

#test "ten sats between three people" {
  #assert share(10, 3, 0) == 4;
  #assert share(10, 3, 1) == 3;
  #assert share(10, 3, 2) == 3;
}

main :: () {
  for i: 0..<3 {
    print("person % gets % sats\n", i, share(10, 3, i));
  }
}
```

*Listing 10-1: A procedure that splits a payment, and one test for it*

The procedure is ordinary Coax. `total / people` is integer division, so
`base` is 3, and `total % people` is the remainder, 1. Person 0 is below
the remainder and gets `base + 1`; everyone else gets `base`.

The new part is the `#test` block. Let's go through it a line at a time.

```jai
#test "ten sats between three people" {
```

`#test` starts a check. The string is its name, which `coax test` prints
when it runs it. Pick a name that says what is being checked, in words
someone reading a failure will understand. The braces hold ordinary
statements: declarations, loops, calls and so on.

```jai
  #assert share(10, 3, 0) == 4;
```

`#assert` takes a `bool`. If it's true, nothing happens. If it's false,
the check fails at this line. You met `#assert` in Chapter 9 as a
top-level declaration that checks a constant while compiling. Inside a
block it's a statement, and the condition can be anything the block can
compute.

Run the program first, to see that the check is not part of it:

```shell
$ coax run split.coax
person 0 gets 4 sats
person 1 gets 3 sats
person 2 gets 3 sats
```

`coax run` compiled `main` and everything it calls, and left the test
out. To run the checks, use `coax test`:

```shell
$ coax test split.coax
ok   #test ten sats between three people (1 case, 64 steps)
1 check, 1 passed, 0 failed
```

Each check gets one line: `ok` or `FAIL`, its kind and name, how many
cases it ran (always 1 for a `#test`), and how many steps the evaluator
took. The last line is the tally. `coax test` exits with status 0 when
every check passes and 1 otherwise, so a script or CI job can use it
directly.

> Note: Rust programmers will recognize `#test` as `#[test]` and
> `#assert` as `assert!`. The difference is where it runs. A Coax check
> never becomes a binary: the compiler evaluates it while compiling, with
> the same limits as `#run`.

### Seeing a check fail

A check that has never failed hasn't shown you that it can. Let's break
`share` on purpose and watch. Change the `<` to `<=`:

```jai
  if i <= total % people { return base + 1; }
```

and run the checks again:

```shell
$ coax test split.coax
FAIL #test ten sats between three people
split.coax:13:3: error[E0406]: assertion failed
    #assert share(10, 3, 1) == 3;
    ^
  note: in #test ten sats between three people
1 check, 0 passed, 1 failed
```

The failure points at the exact `#assert` that was false: line 13, the
check for person 1. The first assertion, for person 0, still passed,
because person 0 gets 4 either way. A check stops at its first failing
assertion, so if several lines are wrong you'll see them one at a time.

Now look at what `coax run` does with the broken code:

```shell
$ coax run split.coax
person 0 gets 4 sats
person 1 gets 4 sats
person 2 gets 3 sats
```

It runs and gives away 11 sats out of 10. Checks in an ordinary file run
only when you ask for them with `coax test`. Later in this chapter we'll
see `#authority`, which makes them run on every build. Change `<=` back
to `<` before we go on.

### What a check can do

A check runs in the compile-time evaluator, so it follows the evaluator's
rules from Chapter 9. It can call any procedure in the file, declare
locals, loop and build arrays and structs. Overflow, division by zero and
out-of-bounds indexes are errors, in every build mode. What it can't do
is reach outside the compiler. If you try to print from a check:

```jai
#test "print" { print("hi\n"); #assert f(1) == 1; }
```

the check fails instead of printing:

```jai
p.coax:3:17: error[E0404]: compile-time execution cannot perform output or host I/O
  #test "print" { print("hi\n"); #assert f(1) == 1; }
                  ^
  note: in #test print
```

This is what makes checks reproducible. A check can't read a file, the
clock or the network, so it gives the same answer on every machine every
time.

### Checking a rule with `#forall`

Our test checks one bill. The rule we care about is broader: whatever
the total and however many people, the shares should add up to the
total. We could write more examples, but we'd be guessing which ones
matter. Instead, let's state the rule and have the compiler try it on
every small case. Add this after the test:

```jai
// ...

#forall "the shares add up" total: 0..<50, people: 0..<8 {
  sum := 0;
  for i: 0..people-1 { sum += share(total, people, i); }
  #assert sum == total;
}

// ...
```

*Listing 10-2: A first try at a rule over every small bill*

`#forall` is followed by an optional name and then one or more
*bindings*. `total: 0..<50` means `total` takes every value from 0 up to
49, and `people: 0..<8` every value from 0 to 7. The block runs once for
every combination, 50 × 8 = 400 times.

If you've written C, the loop inside may look right to you. The compiler
disagrees:

```shell
$ coax test split.coax
split.coax:19:11: error[E0216]: ranges are half-open: write '..<'
    for i: 0..people-1 { sum += share(total, people, i); }
            ^
  fix: for i: 0..<n { }
1 error
```

Coax ranges are always half-open (Chapter 3), and the error even gives
the fix. A check is type-checked like any other code, so a mistake like
this is caught before anything runs. Write `0..<people` and run it
again:

```shell
$ coax test split.coax
ok   #test ten sats between three people (1 case, 64 steps)
FAIL #forall the shares add up
split.coax:20:3: error[E0406]: assertion failed
    #assert sum == total;
    ^
  note: in #forall the shares add up
  counterexample: total = 1, people = 0
2 checks, 1 passed, 1 failed
```

Now we've learned something. The `counterexample` line gives the first
combination of bindings that failed: one sat split between nobody. The
loop runs zero times, `sum` stays 0, and one sat disappears. The bindings
are tried with the last one changing fastest, so `total = 0` came first
and passed for every `people`, and then `total = 1, people = 0` failed.

This is the main reason to write a `#forall`. We didn't think of zero
people when we wrote the example, and we didn't have to. The rule and
the domain found it.

### Red first

What should `share` do with nobody to split between? There's no right
number of sats to return. We'll follow the convention the real wallet
uses (we'll look at it later in this chapter): a negative total, nobody
to split between, or a person who isn't one of them gets -1, meaning
"this question has no answer".

Before changing `share`, let's write that decision down as a test. Add it
after the first one:

```jai
#test "nobody to split between" {
  #assert share(10, 0, 0) == -1;
}
```

Run the checks:

```shell
$ coax test split.coax
ok   #test ten sats between three people (1 case, 64 steps)
FAIL #test nobody to split between
split.coax:6:17: error[E0407]: division by zero during compile-time evaluation
    base := total / people;
                  ^
  note: in #test nobody to split between
FAIL #forall the shares add up
split.coax:24:3: error[E0406]: assertion failed
    #assert sum == total;
    ^
  note: in #forall the shares add up
  counterexample: total = 1, people = 0
3 checks, 1 passed, 2 failed
```

The new test fails, and not on its own `#assert`: `share` divided by
zero, which is always an error in the evaluator. The error points into
`share`, and the note says which check was running. Notice too that
`coax test` keeps going after a failure. Each check runs separately, so
one failure doesn't hide another.

Writing the test before the fix is the habit called *red first*: you see
the check fail for the reason you expect, then make it pass. If you write
the fix first, you never learn whether the test could have caught the
bug.

Now let's make it pass. Here's the whole file with the guard, the new
test, the `#forall` narrowed to bills that have people, and a second
`#forall` for the questions with no answer.

Filename: split.coax

```jai
#import "Basic";

// How many of `total` sats person `i` of `people` gets. The sats that
// don't divide evenly go to the first people, one each. -1 means the
// question has no answer: a negative total, nobody to split between, or
// a person who isn't one of them.
share :: (total: int, people: int, i: int) -> int {
  if total < 0 || people <= 0 || i < 0 || i >= people { return -1; }
  base := total / people;
  if i < total % people { return base + 1; }
  return base;
}

#test "ten sats between three people" {
  #assert share(10, 3, 0) == 4;
  #assert share(10, 3, 1) == 3;
  #assert share(10, 3, 2) == 3;
}

#test "nobody to split between" {
  #assert share(10, 0, 0) == -1;
}

#forall "the shares add up" total: 0..<50, people: 1..<8 {
  sum := 0;
  for i: 0..<people { sum += share(total, people, i); }
  #assert sum == total;
}

#forall "questions with no answer" total: -2..<3, people: -2..<4, i: -2..<5 {
  if total < 0 || people <= 0 || i < 0 || i >= people {
    #assert share(total, people, i) == -1;
  } else {
    #assert share(total, people, i) >= 0;
  }
}

main :: () {
  for i: 0..<3 {
    print("person % gets % sats\n", i, share(10, 3, i));
  }
}
```

*Listing 10-3: `share` with a guard, and checks for both sides of it*

```shell
$ coax test split.coax
ok   #test ten sats between three people (1 case, 112 steps)
ok   #test nobody to split between (1 case, 22 steps)
ok   #forall the shares add up (350 cases, 55449 steps)
ok   #forall questions with no answer (210 cases, 7254 steps)
4 checks, 4 passed, 0 failed
```

Four checks and 562 cases, all passing. The second `#forall` has three
bindings and runs 5 × 6 × 7 = 210 cases. Its ranges start below zero on
purpose: the interesting inputs for a guard are the ones just outside
it.

### Choosing what to bind

A binding can be any of these:

- `bool`, which takes `false` and `true`.
- An enum type, which takes every value the enum declares.
- An integer type of at most 16 bits, which takes every value of the
  type. `x: u8` is 256 cases and `x: u16` is 65,536.
- A half-open range of `int` with constant bounds, like `-2..<5`.

What it can't be is a wide type. If you try to check the rule for every
`int`:

```jai
#forall "the shares add up" total: int, people: 1..<8 {
```

the compiler refuses:

```shell
$ coax test split.coax
split.coax:24:29: error[E0379]: #forall cannot enumerate int
  #forall "the shares add up" total: int, people: 1..<8 {
                              ^
  fix: bind bool, an enum, an integer of at most 16 bits, or a range: total: 0..<100
1 error
```

There are 2^64 values of `int`, and a `#forall` really does try every
case it's given, so it only takes domains it can finish. The total
across all bindings is capped too, at 2^24 (16,777,216) cases. Two `u16`
bindings would be 2^32:

```jai
FAIL #forall a: u16, b: u16
big.coax:2:1: error[E0400]: #forall has more than 16777216 cases
  #forall a: u16, b: u16 { #assert f(a) == a && f(b) == b; }
  ^
  note: in #forall a: u16, b: u16
```

An empty range like `5..<5` is an error as well, since a check that runs
no cases would pass without checking anything.

So how do you check code that works on 64-bit numbers? You test a *small
instance*. The bugs in `share` live in the relationship between `total`,
`people` and `i`: the remainder, the boundaries of the guard, the first
and last person. All of them show up with totals under 50 and up to 7
people. Pick ranges that cross every boundary in the code, and then
write ordinary `#test`s for the few values at the far end, like the
largest amount there is. We'll see the real wallet do exactly that.

### Several properties

One more rule is worth stating. "The first people get the extra sats"
means shares never go up as `i` increases, and no two shares differ by
more than one. Add a third `#forall`:

```jai
#forall "no one gets more than a sat over anyone else" total: 0..<50, people: 1..<8 {
  first := share(total, people, 0);
  last := share(total, people, people - 1);
  #assert first - last <= 1;
  for i: 1..<people {
    #assert share(total, people, i) <= share(total, people, i - 1);
  }
}
```

*Listing 10-4: A rule about fairness*

A check can assert as many times as it likes and inside loops. This one
makes up to seven assertions per case.

Notice that none of our rules repeat the implementation. "The shares add
up" doesn't say how to compute a share; it says what must be true of all
of them together. That matters. A check that reimplements the code with
the same formula is likely to repeat the same mistake. When you can,
state the property you need, not the steps that get you there.

## Controlling how checks run

`coax test` has one option and a few behaviors worth knowing.

### The step budget

Every check gets its own budget of 20,000,000 steps. A step is one unit
of evaluator work: an expression or statement visited, or an array
element created, as in Chapter 9. A `#forall` spends one budget across
all its cases, which is why `coax test` reports steps per check.

You can set the budget with `--steps`. A smaller budget is a quick way
to see how expensive each check is, or to make sure a check stays cheap.
Here's the finished file (Listing 10-6, later in this chapter) with a
budget of 60,000:

```shell
$ coax test split.coax --steps 60000
ok   #test ten sats between three people (1 case, 100 steps)
ok   #test nobody to split between (1 case, 24 steps)
ok   #forall the shares add up (350 cases, 49849 steps)
FAIL #forall no one gets more than a sat over anyone else
split.coax:34:17: error[E0400]: compile-time evaluation exceeded 60000 steps
    #assert first - last <= 1;
                  ^
  note: in #forall no one gets more than a sat over anyone else
  counterexample: total = 31, people = 7
ok   #forall questions with no answer (210 cases, 6990 steps)
5 checks, 4 passed, 1 failed
```

The fairness rule needs about 94,000 steps, so it ran out partway
through. Read the `counterexample` line carefully here: it says where the
budget ran out, not an input that broke the rule. The error code, E0400
rather than E0406, tells you which happened.

The budget is also what keeps a check from hanging. If you write a loop
that never ends, the check fails when it runs out of steps instead of
spinning forever. `--steps` must be a positive whole number; `--steps 0`
is refused.

### Exit status and JSON

`coax test` exits with 0 when every check passes and 1 when any check
fails or the file doesn't compile. A file with nothing to check is an
error too, so a typo in a CI script can't pass silently:

```shell
$ coax test hello.coax
coax: hello.coax has no #test or #forall to run
```

Like every `coax` command, `test` takes `--json`, which prints each
diagnostic as one line of JSON for a tool or an agent to read. Here's the
`<=` bug from earlier:

```shell
$ coax test split.coax --json
FAIL #test ten sats between three people
{"file": "split.coax", "line": 13, "col": 3, "code": "E0406", "message": "assertion failed [#test ten sats between three people]", "fix": ""}
1 check, 0 passed, 1 failed
```

The message keeps the check's name, and for a `#forall` it keeps the
counterexample, so a tool reading the JSON knows which rule broke and
for which values.

### Checks are never part of the program

`coax run`, `coax build`, `coax c` and `coax lib` leave checks out: no
`#test` or `#forall` block appears in the generated C. You can also write
ordinary procedures just for checks to use, such as a nearly-equal
comparison for floating-point results. Those are procedures like any
other, so they are still compiled into the C, even if nothing in the
program calls them.

`#assert` is different. Inside a procedure that the program runs, it
stays in the program and traps when it's false:

```jai
#import "Basic";
half :: (x: int) -> int { #assert x % 2 == 0; return x / 2; }
main :: () { print("%\n", half(8)); print("%\n", half(7)); }
```

```shell
$ coax run rt.coax
4
rt.coax:2: assertion failed
```

The program stops with status 134. It does the same with `--release`; no
build mode removes an `#assert`. Inside a check, the same `#assert`
fails the check instead.

## Organizing checks

### Next to the code

Coax has no separate test directory or test module. Checks go in the
same file as the code they check, usually right after it, which is what
we've done with `share`. When you read a procedure, its rules are just
below it.

Checks may also go before the code. The compiler doesn't care about the
order, since procedures can be referred to before they're declared. Some
people like to write the checks first in the file, as a statement of what
the code must do.

A file that is a library, built with `coax lib` and called from C or
Swift (Chapter 17), has no `main`. That's fine for `coax test`: a file
with `#export_c` procedures and checks can be tested without one.

### Names

Name checks in plain words, as a claim: "the shares add up", "a broken
shot is not followed". A failure then reads as a sentence about what
went wrong. Unnamed `#forall`s are allowed, and `coax test` prints their
bindings instead, as in `#forall a: u16, b: u16`. That's fine for a quick
experiment and hard to read in a list of thirty.

### Regression tests from counterexamples

When a `#forall` finds a counterexample, it gives you the exact values.
Once you've fixed the bug, it's often worth adding a `#test` with those
values. The `#forall` would catch the bug again, but only as long as its
ranges still cover that case. Our `"nobody to split between"` test is
exactly this: the counterexample `people = 0`, written down as a test.

### `#authority`: checks that always run

Some code must never ship with a failing check: code that moves money,
verifies signatures or grants permissions. For these files Coax has
`#authority`. Put it near the top of the file:

```jai
#import "Basic";
#authority;
```

Two things change.

First, every compile of the file runs its checks, and a failing check is
a failed compile. Here's the `<=` bug again, in a file marked
`#authority`:

```shell
$ coax run split.coax
split.coax:14:3: error[E0406]: assertion failed
    #assert share(10, 3, 1) == 3;
    ^
  note: in #test ten sats between three people
1 error
```

`coax run`, `coax build`, `coax check` and the rest all stop the same
way. There's no build that skips the checks.

Second, every procedure in the file must actually run during some check.
Say we add a helper for the receipt screen and forget to test it:

```jai
// What's left over after an even split, for the receipt.
leftover :: (total: int, people: int) -> int {
  return total % people;
}
```

```shell
$ coax run split.coax
split.coax:16:1: error[E0380]: 'leftover' is not exercised by any #test or #forall
  leftover :: (total: int, people: int) -> int {
  ^
  fix: #authority files check every procedure: add a #test or #forall that calls it
1 error
```

"Exercised" means executed, not mentioned. A call inside a branch no
check takes doesn't count, and neither does a call on the right of an
`&&` that never gets evaluated. Only `main` and `#foreign` imports are
exempt. To fix the error, delete `leftover` or write a check that calls
it.

`#authority` doesn't make the checks good. It makes sure they exist,
reach every procedure and pass on every build. Whether they would notice
a bug is the next question.

## Checking the checks with `coax mutate`

A check can pass and still pin down very little. Here's a weaker version
of our first test that checks only person 0:

Filename: split.coax

```jai
#import "Basic";

// How many of `total` sats person `i` of `people` gets. The sats that
// don't divide evenly go to the first people, one each.
share :: (total: int, people: int, i: int) -> int {
  base := total / people;
  if i < total % people { return base + 1; }
  return base;
}

#test "ten sats between three people" {
  #assert share(10, 3, 0) == 4;
}

main :: () {
  for i: 0..<3 {
    print("person % gets % sats\n", i, share(10, 3, i));
  }
}
```

*Listing 10-5: A test that passes but checks too little*

It passes. But the `<=` bug from earlier would pass it too, since person
0 gets 4 either way. How would we notice that a test is this weak?

`coax mutate` answers that question by breaking the code for you. It
makes one small change at a time, a *mutant*, and runs the checks
against each one. If some check fails, the mutant is *killed*: the checks
noticed. If every check still passes, the mutant *survived*, and you've
found a change to the code that your checks can't see.

```shell
$ coax mutate split.coax
split.coax:7:8: mutant survived: '<' became '<='
    if i < total % people { return base + 1; }
         ^
split.coax:7:16: mutant survived: '%' became '/'
    if i < total % people { return base + 1; }
                 ^
6 mutants, 4 killed, 2 survived
```

Two survivors. The first is our `<=` bug. The second turns the remainder
into a quotient: `i < total / people` is `0 < 3` for person 0, which is
true either way. Both survive because the test only asks about person 0.
The exit status is 1, so a script can treat survivors as failure.

The changes `coax mutate` makes are the ones that cause real bugs:

- comparison boundaries: `<` and `<=`, `>` and `>=`
- equality: `==` and `!=`
- arithmetic, logical and bitwise operators: `+` and `-`, `&&` and `||`,
  `%` and `/`, and so on, including `+=` and `-=`
- integer literals, changed from n to n + 1, and `true` and `false`
- the condition of an `if` or `while`, negated
- an assignment or a call, removed

The compiler mutates every site statically reachable from a check, even if a
baseline run never enters it. Such a mutant can survive. A mutant that loops
forever doesn't hang the run: each check gets ten times its unmutated steps,
with a minimum of 10,000, and running out counts as a kill.

To avoid repeating independent work, Coax records which procedures each
baseline check entered and can skip a check for a mutant in an unentered
procedure. It keeps the complete mutant set. If the graph can retain views
or allocate strings across checks, contains unknown elements, or changes
tracked storage during the baseline, it runs the full check suite for every
mutant instead. This depends on constants, defaults and `#run` expressions
being folded before mutation; future shared or lazy state needs a new review.
External memory pressure and host allocation failure are outside this
deterministic isolation model.

Use `coax mutate split.coax --mutation-reference` to force the full runner.
`--mutation-stats` reports check counts and fallback facts;
`--mutation-trace` reports the first killing check for each mutant.

### Running mutants in parallel

Mutants are independent: each one changes one site, runs the checks in a
fresh evaluator and is undone. So `coax mutate` evaluates them in worker
processes, one per available core by default. `--jobs N` sets the number
(1 to 1,024; `--jobs 1` keeps everything in one process), and so does the
`COAX_MUTATE_JOBS` environment variable when `--jobs` is absent. Each worker
starts from a copy of the compiler made after the unmutated checks pass, and
the report is printed only after every mutant is done, in source order. The
mutant set, every kill or survival, the trace, the statistics and the exit
status are the same for every N:

```shell
$ coax mutate split.coax --jobs 1
6 mutants, 6 killed, 0 survived
$ coax mutate split.coax --jobs 8
6 mutants, 6 killed, 0 survived
```

To split one file's mutants across shells or machines, give each run a
shard. `--shard I/N` evaluates the mutants whose index leaves remainder
I - 1 when divided by N, so the N shards are disjoint and cover every
mutant. A shard prints a record on standard output and its own survivors on
standard error. `--merge` combines all N records into the full report:

```shell
$ coax mutate split.coax --shard 1/2 > one.rec
$ coax mutate split.coax --shard 2/2 > two.rec
$ coax mutate split.coax --merge one.rec --merge two.rec
6 mutants, 6 killed, 0 survived
```

A record names the compiler, the file's text and every mutant, so a merge
refuses records from another file, another compiler, a missing or repeated
shard, or a truncated record. The merged report, trace and statistics are
byte for byte the serial ones. `coax/tests/mutation_parallel.py` checks that
for synthetic files and real `#authority` units: pass it `.coax` paths to
compare any file.

To kill a survivor, write the check that would have caught it. Restore
the two assertions for persons 1 and 2 (that's Listing 10-1 again):

```shell
$ coax mutate split.coax
6 mutants, 6 killed, 0 survived
```

### Survivors that can't be killed

Now let's mutate the full file: Listing 10-3 plus the fairness rule of
Listing 10-4, with `#authority` at the top. That's 5 checks and over 900
cases. Surely nothing survives?

```shell
$ coax mutate split.coax
split.coax:9:26: mutant survived: '<=' became '<'
    if total < 0 || people <= 0 || i < 0 || i >= people { return -1; }
                           ^
18 mutants, 17 killed, 1 survived
```

One does. Changing `people <= 0` to `people < 0` lets `people = 0`
through that test, but look at the rest of the guard. With no people,
every `i` is either below 0 or at least `people`, so `i < 0 || i >=
people` returns -1 anyway. No input can tell the two versions apart. This
is an *equivalent mutant*, and no check can kill it.

There are two ways to deal with one. You can mark the line, telling
`coax mutate` that a person has looked at it and decided no input can
observe the change:

```jai
  if total < 0 || people <= 0 || i < 0 || i >= people { return -1; } // mutant: equivalent
```

```shell
$ coax mutate split.coax
18 mutants, 17 killed, 0 survived, 1 marked equivalent
```

Marked survivors are counted separately and don't fail the run. Use the
marker sparingly: it covers every mutant on its line, so a real survivor
on the same line would be hidden too. And it's a claim made by a person,
not something the compiler proves, so it deserves a reviewer's eye.

The other way, often the better one, is to take the hint. An equivalent
mutant usually means the code says something twice. Here `people <= 0`
adds nothing that `i < 0 || i >= people` doesn't already cover, so we
can delete it:

```jai
  if total < 0 || i < 0 || i >= people { return -1; }
```

```shell
$ coax mutate split.coax
15 mutants, 15 killed, 0 survived
```

Here's the finished program. It's in the book's examples as
`examples/ch10_split.coax`.

Filename: split.coax

```jai
#import "Basic";
#authority;

// How many of `total` sats person `i` of `people` gets. The sats that
// don't divide evenly go to the first people, one each. -1 means the
// question has no answer: a negative total, nobody to split between, or
// a person who isn't one of them.
share :: (total: int, people: int, i: int) -> int {
  if total < 0 || i < 0 || i >= people { return -1; }
  base := total / people;
  if i < total % people { return base + 1; }
  return base;
}

#test "ten sats between three people" {
  #assert share(10, 3, 0) == 4;
  #assert share(10, 3, 1) == 3;
  #assert share(10, 3, 2) == 3;
}

#test "nobody to split between" {
  #assert share(10, 0, 0) == -1;
}

#forall "the shares add up" total: 0..<50, people: 1..<8 {
  sum := 0;
  for i: 0..<people { sum += share(total, people, i); }
  #assert sum == total;
}

#forall "no one gets more than a sat over anyone else" total: 0..<50, people: 1..<8 {
  first := share(total, people, 0);
  last := share(total, people, people - 1);
  #assert first - last <= 1;
  for i: 1..<people {
    #assert share(total, people, i) <= share(total, people, i - 1);
  }
}

#forall "questions with no answer" total: -2..<3, people: -2..<4, i: -2..<5 {
  if total < 0 || people <= 0 || i < 0 || i >= people {
    #assert share(total, people, i) == -1;
  } else {
    #assert share(total, people, i) >= 0;
  }
}

main :: () {
  for i: 0..<3 {
    print("person % gets % sats\n", i, share(10, 3, i));
  }
}
```

*Listing 10-6: The finished split, every check passing and every mutant
killed*

```shell
$ coax run split.coax
person 0 gets 4 sats
person 1 gets 3 sats
person 2 gets 3 sats
$ coax test split.coax
ok   #test ten sats between three people (1 case, 100 steps)
ok   #test nobody to split between (1 case, 24 steps)
ok   #forall the shares add up (350 cases, 49849 steps)
ok   #forall no one gets more than a sat over anyone else (350 cases, 94444 steps)
ok   #forall questions with no answer (210 cases, 6990 steps)
5 checks, 5 passed, 0 failed
```

The checks still mention `people <= 0`. That's fine: `coax mutate`
changes the procedures the checks call, not the checks, and the rule
"nobody to split between has no answer" is still the rule we want.

## Checks in the real world

The Grid's wallet in `verse/wallet.coax` is written this way. Swift
talks to the payment SDK and hands the Coax file plain numbers: amounts,
fees, times and what the SDK reported. Coax decides whether a quote may
be paid. The file starts like our split:

```jai
#import "Basic";
#authority;

// All the bitcoin there will ever be, in satoshis.
MAX_SAT :: 2_100_000_000_000_000;

// ---- Amounts ----

// The SDK counts satoshis; payment.v1 counts millisatoshis.
verse_wallet_sat_to_msat :: (sat: int) -> int #export_c {
  if sat < 0 || sat > MAX_SAT { return -1; }
  return sat * 1000;
}

#forall "sats to msat" sat: -3..<40 {
  m := verse_wallet_sat_to_msat(sat);
  if sat < 0 { #assert m == -1; } else { #assert m == sat * 1000; }
}

#test "sats to msat at the top" {
  #assert verse_wallet_sat_to_msat(MAX_SAT) == MAX_SAT * 1000;
  #assert verse_wallet_sat_to_msat(MAX_SAT + 1) == -1;
  #assert verse_wallet_sat_to_msat(0) == 0;
}
```

*Listing 10-7: The start of `verse/wallet.coax`*

Here is the small-instance pattern from earlier. The `#forall` covers
amounts from -3 to 39 sats, crossing the boundary at zero, and the
`#test` covers the other boundary, all the bitcoin there is, where a
`#forall` could never reach. Every procedure in the file has the same
pair, and the -1 convention we borrowed for `share` comes from here.

The wallet's payment flow is a state machine: a quote, a confirm, a send,
and then success, failure or "unknown" if the SDK doesn't answer. Its
most important rule is that one quote is never paid twice. A `#forall`
can bind enums, so the wallet checks every sequence of four events from
the start:

```jai
#forall "every four events" a: Event, b: Event, c: Event, d: Event {
  events := [4] Event.[a, b, c, d];
  f := verse_wallet_start();
  for e: events {
    before := f;
    f = verse_wallet_step(f, e);
    #assert f.sends <= 1;
    // ...
  }
}
```

*Listing 10-8: Every four-event history of a payment, from `verse/wallet.coax`*

`Event` has 8 values, so that's 8^4 = 4,096 histories, every one of
them. The whole file runs in a few hundredths of a second:

```shell
$ coax test verse/wallet.coax
ok   #forall sats to msat (43 cases, 1188 steps)
ok   #test sats to msat at the top (1 case, 54 steps)
...
ok   #forall may_send (10080 cases, 333238 steps)
ok   #test may_send near all the bitcoin (1 case, 121 steps)
ok   #test a fresh payment (1 case, 24 steps)
ok   #forall each step (96 cases, 9876 steps)
ok   #forall every four events (4096 cases, 1355356 steps)
16 checks, 16 passed, 0 failed
$ coax mutate verse/wallet.coax
155 mutants, 155 killed, 0 survived
```

Because the file is `#authority`, those 16 checks run every time
`apps/openagents-ios/generate.sh` compiles it to C for the iOS app. A
change that lets a quote be paid twice doesn't make it to a build.

Not every file needs that. `verse/camera.coax`, the camera that
circles the Grid and swoops down behind your character, has ten checks
but no `#authority`. Its checks use helpers like `near` for comparing
floats within a tolerance, and its rules are about motion: "the ease only
rises", "the dolly is smooth, above the ground, and comes in from
behind". Mutating it shows that the checks leave a lot unpinned:

```shell
$ coax mutate verse/camera.coax
...
verse/camera.coax:116:89: mutant survived: '&&' became '||'
  same :: (a: Point, b: Point, tolerance: f32) -> bool { return near(a.x, b.x, tolerance) && near(a.y, b.y, tolerance) && near(a.z, b.z, tolerance); }
                                                                                          ^
87 mutants, 63 killed, 24 survived
```

That's a reasonable place for a camera to be. A wrong camera looks wrong
on screen, and someone sees it at once. A wrong payment rule loses
money quietly. Decide per file how much assurance the code needs, and
put `#authority` on the files where a bug costs money, keys or trust.

The compiler holds itself to the same standard. `coax/build.sh` runs the
checks and the mutation gate on the Coax parts of the compiler, such as
the protocol wire runtime in `coax/lib/proto/wire.coax` (382 mutants,
all killed), before it publishes `coax/bin/coax`.

## What checks don't prove

It's worth being exact about what passing all of this means.

A `#forall` covers the cases it was given and no others. `share` passing
for totals under 50 doesn't prove it right for a total of 10^15; it shows
that the logic is right on an instance small enough to try exhaustively,
which is strong evidence when the code treats big and small numbers the
same way.

`#authority`'s coverage rule means every procedure ran. It doesn't mean
every branch ran, or that an assertion looked at what the procedure
returned.

`coax mutate` tries the changes in its list and nothing else. A bug that
isn't one of those changes, like calling the wrong procedure or getting
a formula subtly wrong, can survive a run with every mutant killed.

None of this replaces thinking about the code. Checks turn what you
decided into something the compiler re-tests on every change, and that
is a lot.

## Summary

Checks are how you write down what code must do. A `#test` runs a block
once, and a `#forall` runs it for every combination of small bindings
and reports the first counterexample. Both run in the compiler's sealed
evaluator with a step budget, so they're reproducible and can't hang,
and neither is part of the program. `coax test` runs them, `--steps`
sets the budget, and `--json` gives tools structured failures.

Put checks next to the code, name them as claims, and turn
counterexamples into regression tests. For code that guards money or
permissions, `#authority` makes every build run the checks and requires
each procedure to be exercised. `coax mutate` then tells you whether the
checks would notice a bug: kill the survivors with better checks, and
when a survivor can't be killed, simplify the code or mark the line as
equivalent.

Part III turns to performance, starting with how data is laid out in
memory. The checks come along: in Chapter 12 we'll use a `#forall` to
confirm that a parallel sum gives the same answer however the work is
split.

For the distinction between finite checks, native tests, independent oracles
and authorization, see [Chapter 25](ch25-verification-boundaries.md).

## Checks that belong to a build profile

Coax 0.17.0 adds a small conditional-compilation slice:

<!-- checked-source: examples/ch10_build_profiles.coax -->
```coax
#authority;
Sample :: struct {
 value:u32;
 #if BUILD_DEBUG {magnitude:u64;}
}
inspect :: (value:u32)->u32 #export_c {
 sample:Sample;sample.value=value;
 #if BUILD_DEBUG {sample.magnitude=1;#assert sample.magnitude==1;}
 return sample.value;
}
#test "both build profiles preserve the value with different layouts" {
 #assert inspect(3)==3;
 #if BUILD_DEBUG {#assert size_of(Sample)==16;}
 else {#assert size_of(Sample)==4;}
}
```

Run the selected checks and mutation in both profiles:

```sh
coax/bin/coax test coax/book/examples/ch10_build_profiles.coax
coax/bin/coax mutate coax/book/examples/ch10_build_profiles.coax
coax/bin/coax test coax/book/examples/ch10_build_profiles.coax --release
coax/bin/coax mutate coax/book/examples/ch10_build_profiles.coax --release
```

Both branches must parse. Only the selected fields and statement blocks reach
resolution, layout, compile-time work and output. The profile selector is fixed
by the compiler's `--release` flag; it is not a lookup of a user constant.
Arbitrary conditions, imports and top-level declarations are still planned.
Statement blocks keep lexical scope, so locals declared inside one stay there.

An ordinary #assert still executes in release. This construct lets the author
explicitly remove a development-only statement; it does not justify removing
money, admission or other required authority checks. A profile's tests and
mutation results describe that profile, so exercise both when both ship.
The native struct layout differs: every linked consumer must use the matching
profile/header. Wire schemas keep their own stable definitions. Compiler release
arithmetic and native optimization also remain separate choices; see Chapter 24.
