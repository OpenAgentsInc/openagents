# Test a plugin

A test shows whether a plugin helps. Coder does the same tasks twice:
once with the plugin and once without it. Each test is a task (a prompt
and the files it starts with) and the checks on what the run did.

## Run the tests

In chat, ask to test a plugin, and tap **Start the test** on the card:
we run the tests on our computers and the card shows the result. On a
computer, from the plugin's directory:

```sh
openagents plugin test init               # write the tests with us, step by step
openagents plugin test run . --runs 1     # try each test once
openagents plugin test run .              # the full run: three times with, three without
```

Write at least four tests where the plugin should help and one or two
where it should stay out of the way. Check for facts that are only in the
test's files, never in the prompt, so a run without the plugin can't pass
by repeating the question. Every run happens in a fresh sandbox that
starts with the test's files and nothing else. Nothing leaves your
computer until you publish.

## What the result means

A result reads like "passed 7 of 7 with it, 2 of 7 without", with one of
three verdicts:

- **Better.** With the plugin, Coder passed more tests, still passed every
  test where the plugin should stay out of the way, raised the average
  score by more than runs of the same setup vary, and wasn't much slower
  or costlier.
- **Worse.** With the plugin, Coder passed fewer tests, or failed a test
  where the plugin should have stayed out of the way.
- **No clear change.** Anything else. With one run each way, the verdict
  is always this, because there's no way to tell a change from noise.

Faster or cheaper never makes a plugin **Better** on its own; it's noted
beside the verdict.

## What our computers can and can't see

When you start a test from chat, it runs on our hosted runner:

- It runs only plugins in our catalog and plugins made in chat.
- Coder there can't run commands or use the network, so **without a
  plugin it can't read what's in the test's files.** A test whose answer
  is in the files fails without the plugin. That is a real difference,
  but it is bigger than the same comparison on a computer with a shell.
- Both runs include Coder's defaults (today, Project map), so a result
  says what the plugin adds on top of them.
- A test set run there has 8 tests or fewer, each run up to three times
  with the plugin and three times without.

## The three examples

On 2026-10-01, on our computers, all three example plugins were
**Better**: Explain this error passed 7 of 7 with it and 2 of 7 without,
Release notes 6 of 6 and 2 of 6, and Dependency check 6 of 6 and 2 of 6.
Read those numbers with the caveats: most of the change is the reach
described above; two Release notes tests count a paraphrased line as a
failure, which is stricter than understanding; and in one of three runs,
Dependency check wasn't picked for "Is our requirements file pinned well
enough to deploy from?". One person's keys sent every request, and no
one else has rerun them yet. The full record is on
[GitHub](https://github.com/OpenAgentsInc/openagents/blob/main/docs/extensions/measurements/2026-10-01-example-plugins.md).

Next: [Publish and share](/docs/publish-and-share).
