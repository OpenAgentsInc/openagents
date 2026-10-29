# 19. Built for agents

Most of the code in the OpenAgents repositories is written by coding
agents and reviewed by people. That includes most of the Coax in
`verse/`: the camera from Chapter 17, the wallet's payment rules, the
compiler's own lexer, parser and type checker. An agent's working loop is simple. It
edits a file, runs a tool, reads what the tool printed, and decides what
to do next. Whatever the compiler prints is, in effect, the agent's whole
view of whether the code is right.

In this chapter we'll walk through that loop with the tools `coax0` has
today. We'll read diagnostics as JSON lines, look codes up with
`coax explain`, and then spend most of the chapter on the part that
matters most when the author isn't a person: checks that fail a build
under the reviewed source and verification configuration. We'll build a small spending rule,
watch `#authority` refuse it, watch `coax mutate` show that its tests
prove almost nothing, fix that, and then watch the build stop a plausible
"cleanup" that would have broken the rule.

> Note: Stable error codes, fix lines, `--json`, `coax explain`,
> `coax check`, `coax test`, `#authority`, `coax mutate`, `coax layout`
> and readable C output all work in `coax0` 0.10.0. A formatter, a
> compiler server that answers queries and expansion traces are designed
> (spec §17.3) and marked where they come up.

The agent can edit the checks, directives and build scripts as well as the
implementation. Compiler gates do not protect their own configuration from
that author. Review those changes and preserve independent expectations;
[Chapter 25](ch25-verification-boundaries.md) explains the trust boundary.

## What makes a language easier for an agent

Agents learn by imitation. They copy the patterns in the files around
them, and they reach for whatever they've seen most often in training,
which for a young language is mostly outdated text. Two properties of a
language help a lot with that.

The first is having one way to write each thing. If a language has five
spellings for a declaration, agents will produce all five, and reviewers
have to read all five. Coax has one declaration form (`name :: value` for
constants, `name := value` or `name: T = value` for variables), named
struct fields, and conversions that are always written out.

The second is that nothing happens off the line you're reading. Coax has
no implicit conversions, no shadowing, no constructors, no exceptions and
no operator overloading (the overloads planned for later may not allocate
or do I/O behind math syntax). That rules out a whole class of agent mistakes,
the ones where code looks right and quietly means something else. Here's
one an agent might easily write:

Filename: shadow.coax

```jai
#import "Basic";
main :: () {
  count := 3;
  for i: 0..<2 {
    count := i;
    print("%\n", count);
  }
}
```

In many languages the inner `count := i` declares a new variable that
hides the outer one, and the program runs. Maybe that was intended,
maybe the author meant `count = i`. Coax refuses to guess:

```shell
$ coax check shadow.coax
shadow.coax:5:5: error[E0303]: 'count' is already declared at line 3; Coax does not allow shadowing
      count := i;
      ^
  fix: pick another name
1 error
```

For a person this is a small convenience. For an agent it's the
difference between a bug that ships and an error it fixes on the next
step.

## Diagnostics as JSON lines

A person reads the compiler's human output well enough. A program parsing
it has to deal with carets, indentation and wording that may change. So
every `coax` command that reports errors also takes `--json`. Here's a
file with two mistakes in it:

Filename: two.coax

```jai
#import "Basic";

main :: () {
  total := 40;
  tip: s32 = total / 5;
  print("tip %\n", tipp);
}
```

First the human form:

```shell
$ coax check two.coax
two.coax:5:20: error[E0325]: 'tip' is s32 but is initialized with int
    tip: s32 = total / 5;
                     ^
  fix: convert it: cast(s32) x
two.coax:6:20: error[E0306]: 'tipp' is not declared
    print("tip %\n", tipp);
                     ^
2 errors
```

And with `--json`:

```shell
$ coax check two.coax --json
{"file": "two.coax", "line": 5, "col": 20, "code": "E0325", "message": "'tip' is s32 but is initialized with int", "fix": "convert it: cast(s32) x"}
{"file": "two.coax", "line": 6, "col": 20, "code": "E0306", "message": "'tipp' is not declared", "fix": ""}
```

Each diagnostic is one JSON object on one line, with the same six fields
every time. `fix` is always present; it's an empty string when the
compiler has no specific suggestion, as for the misspelled `tipp`. There's
no summary line, so a tool can read objects until the stream ends. Like
the human form, the JSON goes to standard error, and the exit status is
1 when there were errors, 0 when there weren't.

That makes the output easy to feed into other tools. With `jq`, for
example:

```shell
$ coax check two.coax --json 2>&1 | jq -r '"\(.code) line \(.line): \(.fix)"'
E0325 line 5: convert it: cast(s32) x
E0306 line 6: 
```

`--json` works the same way with `coax c`, `coax build`, `coax run` and
`coax test`. One exception today: `coax mutate` prints its survivors as
text even with `--json`, in the format we'll see shortly.

## Codes, not messages

The `code` field is the part a tool should match on. Messages get
reworded as the compiler improves; codes don't change meaning. When a
code stops being used, it's retired rather than reused, and
`coax explain` still knows it:

```shell
$ coax explain E0102
E0102: retired in 0.3.0
  Was: floating-point literals are not supported. Floats arrived in 0.3.0.
```

For a live code, `coax explain` gives a short title and the reasoning
behind the rule, which is often what an agent needs to choose the right
fix rather than the first one that compiles:

```shell
$ coax explain E0325
E0325: initializer has the wrong type
  The variable's declared type and its initializer differ. There are no implicit conversions.
```

`coax explain --all` prints every code (126 of them in 0.10.0, three
retired), and `coax explain --all --markdown` prints them as the tables
in Appendix B, which is how that appendix is produced. An unknown code is
an error with exit status 1, so a tool can tell "no such code" from an
explanation:

```shell
$ coax explain E9999
coax: no error code 'E9999'
```

The first two digits of every code name the compiler stage that found the
problem (`E01` lexer, `E02` parser, `E03` checker, `E04` compile-time
evaluation, `E05` descriptor imports, `E06` source loading), which Chapter 4 covers along with
how to read a diagnostic in general.

## Checks as guardrails

Type errors are the easy part. The mistakes that matter in an agent's
code are the ones that compile: an off-by-one in a limit, a branch that's
never taken, a test that passes no matter what the code does. Chapter 10
taught `#test`, `#forall`, `#authority` and `coax mutate` from the
programmer's side. Here we'll look at them as an agent meets them,
because these commands provide concrete failures instead of asking the
author to judge its own output. Their configuration still needs review.

Suppose an agent is asked to write the daily spending rule for a wallet.
It writes two procedures and, being diligent, one test:

Filename: spend.coax

```jai
#import "Basic";
#authority;

// May a payment of `amount` sats go out, with `spent` already sent today
// and a daily limit of `limit`?
may_send :: (amount: int, spent: int, limit: int) -> bool {
  if amount <= 0 { return false; }
  return spent + amount <= limit;
}

// What is left to spend today; never negative.
left_today :: (spent: int, limit: int) -> int {
  if spent >= limit { return 0; }
  return limit - spent;
}

#test "a payment within the limit goes" {
  #assert may_send(100, 0, 1000);
}

main :: () {
  print("send 250 after 800? %\n", may_send(250, 800, 1000));
  print("left after 800: %\n", left_today(800, 1000));
}
```

*Listing 19-1: An agent's first attempt at a spending rule*

The second line, `#authority;`, marks this as a file whose rules guard
money. That changes what the compiler accepts:

```shell
$ coax check spend.coax
spend.coax:12:1: error[E0380]: 'left_today' is not exercised by any #test or #forall
  left_today :: (spent: int, limit: int) -> int {
  ^
  fix: #authority files check every procedure: add a #test or #forall that calls it
1 error
```

In an `#authority` file, every procedure except `main` and `#foreign`
imports has to actually run during some check. Not merely be mentioned:
run. An agent can't satisfy this by writing a test that names the
procedure in a branch that's never taken. Here's an attempt:

```jai
#authority;
double :: (x: int) -> int { return x * 2; }
#test "looks like it calls double" { #assert true || double(2) == 4; }
main :: () {}
```

```shell
$ coax check sc.coax
sc.coax:2:1: error[E0380]: 'double' was never executed by a check
  double :: (x: int) -> int { return x * 2; }
  ^
  fix: calls in dead branches or pre-folded #run results do not exercise the procedure
1 error
```

`||` stops at `true`, so `double` never runs, and the compiler knows it
because it ran the check to find out.

So our agent adds a test for `left_today`, and the file compiles and
runs:

Filename: spend.coax

```jai
// ...

#test "a payment within the limit goes" {
  #assert may_send(100, 0, 1000);
}

#test "what is left" {
  #assert left_today(300, 1000) == 700;
}

// ...
```

*Listing 19-2: The spending rule with one test per procedure*

```shell
$ coax run spend.coax
send 250 after 800? false
left after 800: 200
$ coax test spend.coax
ok   #test a payment within the limit goes (1 case, 17 steps)
ok   #test what is left (1 case, 16 steps)
2 checks, 2 passed, 0 failed
```

Everything passes, and an agent that stopped here would report success.
But two test cases can't say much about rules with this many edges. How
would we know? We'd break the code on purpose and see whether the tests
notice. That's what `coax mutate` does, one small change at a time:

```shell
$ coax mutate spend.coax
spend.coax:7:13: mutant survived: '<=' became '<'
    if amount <= 0 { return false; }
              ^
spend.coax:7:16: mutant survived: 0 became 1
    if amount <= 0 { return false; }
                 ^
spend.coax:7:27: mutant survived: false became true
    if amount <= 0 { return false; }
                            ^
spend.coax:8:25: mutant survived: '<=' became '<'
    return spent + amount <= limit;
                          ^
spend.coax:8:16: mutant survived: '+' became '-'
    return spent + amount <= limit;
                 ^
spend.coax:13:12: mutant survived: '>=' became '>'
    if spent >= limit { return 0; }
             ^
spend.coax:13:30: mutant survived: 0 became 1
    if spent >= limit { return 0; }
                               ^
10 mutants, 3 killed, 7 survived
```

Each line is a version of the program that is wrong and still passes
every check. Look at the third one: `return false` became `return true`,
so the wallet would send zero and negative amounts, and the tests didn't
notice. Look at the fifth: `spent + amount` became `spent - amount`, so
the limit would barely apply, and the tests didn't notice that either.
`coax mutate` exits with status 1 when anything survives, so this is a
failure an agent sees, not a hint it can ignore.

The fix isn't more single examples. It's to state the rule itself and
check it over a whole range of inputs, with `#forall`:

```jai
// ...

#forall "sends exactly what the rule allows" amount: -2..<12, spent: 0..<12 {
  #assert may_send(amount, spent, 10) == (amount > 0 && spent + amount <= 10);
}

#forall "what is left never goes negative" spent: 0..<15 {
  left := left_today(spent, 10);
  #assert left >= 0;
  #assert left == ifx spent >= 10 then 0 else 10 - spent;
}

main :: () {
  // ...
}
```

*Listing 19-3: The rule stated over a range of inputs*

The ranges are chosen to cross every boundary: amounts from -2 through
11, spent from 0 through 11, against a limit of 10, so the cases include
a zero amount, a payment that lands exactly on the limit and one that
goes one sat past it.

```shell
$ coax test spend.coax
ok   #test a payment within the limit goes (1 case, 17 steps)
ok   #test what is left (1 case, 16 steps)
ok   #forall sends exactly what the rule allows (168 cases, 4252 steps)
ok   #forall what is left never goes negative (15 cases, 407 steps)
4 checks, 4 passed, 0 failed
$ coax mutate spend.coax
spend.coax:13:12: mutant survived: '>=' became '>'
    if spent >= limit { return 0; }
             ^
10 mutants, 9 killed, 1 survived
```

Nine of ten mutants are now caught. The one that survives is worth
thinking about. If `spent >= limit` becomes `spent > limit`, then at
`spent == limit` the code skips the early return and computes
`limit - spent`, which is 0, exactly what the early return gives. No
input can tell the two programs apart. This is an equivalent mutant, and
no test can kill it.

Coax lets a reviewer say so in the source:

```jai
  if spent >= limit { return 0; } // mutant: equivalent (at spent == limit both branches return 0)
```

```shell
$ coax mutate spend.coax
10 mutants, 9 killed, 0 survived, 1 marked equivalent
```

The marker must be written exactly as `// mutant: equivalent`, followed
by a space or the end of the line. `// mutant: equivalent: because…`, with
a colon straight after, isn't recognized, and the mutant still counts as
a survivor.

The marked mutant is counted apart, not hidden, and the mark is a claim a
person has to stand behind. The repository's rules for agents make that
explicit: an agent may not mark a mutant equivalent to make a run pass.
When an agent is left with a survivor like this one, it reports it and
lets the reviewer decide, or rewrites the code so the question doesn't
arise. That keeps the gate meaningful: the only way through it is checks
that would catch a real change.

## The build that refuses

Now the payoff. Some weeks later another agent, tidying up, decides that
a payment landing exactly on the limit should count as over it, and
changes one character:

```jai
  return spent + amount < limit;
```

The first test still passes: 0 + 100 is comfortably below 1000. If the
file had only Listing 19-2's tests, the change would ship. But
`#authority` runs every check on every compile, so this doesn't even get
as far as a test run:

```shell
$ coax c spend.coax
spend.coax:26:3: error[E0406]: assertion failed
    #assert may_send(amount, spent, 10) == (amount > 0 && spent + amount <= 10);
    ^
  note: in #forall sends exactly what the rule allows
  counterexample: amount = 1, spent = 9
1 error
```

The compiler doesn't just say the check failed; it names the smallest
input it found that breaks the rule. 9 sats spent, 1 more requested,
limit 10: the old code allowed it, the new code doesn't, and the rule
says it should go. That's exactly what an agent needs to decide whether
its change was wrong or the rule should change (in which case the person
reviewing it should see the `#forall` change too). In JSON, the check's
name and the counterexample travel in the message:

```shell
$ coax check spend.coax --json
{"file": "spend.coax", "line": 26, "col": 3, "code": "E0406", "message": "assertion failed [#forall sends exactly what the rule allows]; counterexample: amount = 1, spent = 9", "fix": ""}
```

This is how the real wallet is built. `verse/wallet.coax` is
`#authority`, and its checks are strong enough that every mutant dies:

```shell
$ coax mutate verse/wallet.coax
155 mutants, 155 killed, 0 survived
```

As Chapter 17 showed, the iOS app's build phase compiles it with
`coax c`, so a change that breaks a payment rule fails the Xcode build.
No app with the broken rule can be produced, by an agent or anyone else.
The compiler's own lexer, parser and protobuf importer are held to the
same gate: `coax/build.sh` runs `coax test` and `coax mutate` on each of
them before it will publish a compiler.

> Note: These gates don't prove a program correct, and the spec says so.
> Running a procedure in a check isn't branch coverage, `#forall` covers
> only the finite ranges you give it, and mutation tries only the
> operators it implements. What they do is make a weak test suite visible
> and a broken rule unbuildable, which is most of what a reviewer needs.

## Reading what the compiler made

When an agent needs to know what the code actually does, rather than
what it looks like, the compiler can show it. `coax layout` prints struct
sizes, alignment, offsets and padding, so an agent writing a struct that
crosses into C or Metal doesn't have to compute them (Chapter 11).
`coax c` prints the generated C, and `--keep-c` keeps it next to a build,
for when performance or behavior matters and the question is "what will
the machine run?" The C is meant to be read: names are kept, checks are
explicit calls, and every trap names a source line.

> Note: The compiler is planned to run as a server that answers questions
> directly: the type of this expression, where a symbol is used, a job's
> effects, a struct's layout on a given target, what a macro expanded to
> and from which generator, what changed since the last build. Generated
> declarations will keep the chain that produced them, so an error in
> generated code points at the call that caused it and an agent doesn't
> "fix" code that will be regenerated over the fix. A canonical formatter
> is planned as well. None of these exist in `coax0` yet.

## The book is checked too

There's one more source of truth an agent reads: documentation. For a
young language, most of what an agent has seen about it is out of date.
So the examples in this book aren't copied into the text by hand and left
to drift. Every program lives in `coax/book/examples/`, and
`coax/book/check-examples.sh` compiles each one and compares its output
to the `.out` file the chapter shows. If the compiler changes and the book
doesn't, that script fails.

The compiler's own tests use the same idea for errors. Each file in
`coax/tests/fail` begins with the code it must produce:

```jai
// expect: E0406
#authority;
double :: (x: int) -> int { return x + 2; }
#forall x: 0..<10 { #assert double(x) == x * 2; }
main :: () { }
```

The test runner checks that compiling it fails with `error[E0406]`. A
change that makes a diagnostic disappear, or turns it into a different
one, breaks the suite.

## Writing Coax well, as an agent

Here are the habits this chapter has been building, short enough to keep
in a prompt:

1. Run `coax check FILE --json` after every edit, and fix the first error
   first; later errors are often caused by it.
2. Match on `code`, never on `message`. When a code is unfamiliar, run
   `coax explain CODE` before guessing at a fix.
3. Don't write features `coax0` refuses as not supported yet. Restructure
   the code instead.
4. Write the checks before the code. Put `#authority;` on any file that
   decides money, admission or policy.
5. Run `coax test FILE`, then `coax mutate FILE`, and treat any survivor as
   a missing check. Don't mark mutants equivalent yourself; report them.
6. Prefer one `#forall` that states the rule over many single examples, and
   choose its ranges to cross every boundary in the code.
7. Read the generated C (`coax c`) or the layout (`coax layout`) when
   performance or behavior matters, rather than reasoning from the source.
8. When you add a feature or fix a bug in the compiler, add a test: a
   `tests/pass` program with exact output, or a `tests/fail` program whose
   first line names the expected code.

## Summary

An agent sees a program through what its tools print, so Coax makes that
output precise and machine-readable: one JSON object per diagnostic, with
stable codes that `coax explain` can describe and that are retired rather
than reused. The language leaves out shadowing, implicit conversions and
the other places where code means something other than what it says.
`#authority` makes every procedure in a money file run under a check,
`coax mutate` exposes tests that don't constrain anything, and because an
`#authority` file runs its checks on every compile, a change that breaks
the rule can't be built at all, with the counterexample in the error.

That's the end of Part IV. In Part V we turn to the compiler itself:
Chapter 20 takes `coax0` apart, and Chapter 21 looks at what comes after
it.
