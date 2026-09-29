# Authoring interview: live run, 2026-09-29

What we ran: the chat driver of the authoring interview
([#9937](https://github.com/OpenAgentsInc/openagents/issues/9937),
`coder::eval_author` over `ext_eval::author`) against the live model door
(the Vercel AI Gateway, `google/gemini-3.8-flash`, reasoning effort `low`)
and live Jev (`POST /v1/systemone`), for the three starter tools: Project
map, Code finder, and Test reader. A scripted person answered each step;
Jev decided every tool pick and every approval. The taps on **Try it once**
and the full run went to the engine's fake runner
(`ext_eval::author::runner::fake`), because the hosted runner is
[#9935](https://github.com/OpenAgentsInc/openagents/issues/9935): the try
and full-run numbers below are the fake's, not Coder's. Each finished test
set was written out as case files and loaded by the engine
(`Suite::load`): all three loaded, every test runs three times per arm, and
each set has tests where the tool should stay out of the way.

To reproduce:

```sh
CODER_ENV_FILE=/path/to/door.env TYPESAFE_ENV_FILE=/path/to/typesafe.env \
EVAL_AUTHOR_LIVE_OUT=/tmp/interviews \
  cargo test -p coder --test eval_author_live -- --ignored --nocapture
```

## Latency

A model step (steps 1, 3, 4, and 5) took 2.0 to 8.2 seconds with reasoning
effort `low`, inside the router's 12-second author budget. A first run with
the door's default reasoning took 7 to 33 seconds per step, so the chat
driver's door asks for `low` (`coder::eval_author::door`). Steps the
machine's own words carry (2, the start of 5, 6, and 7) take no model call.

## What the run showed

- Jev read each **Looks good** as an approval and each change request
  ("Add one test about finding every caller of a function", "Also check
  that the answer names the failing test") as a change: the gate was asked
  again with the change made, and never passed.
- The model wrote self-contained tasks that create their own files, since
  every run starts in an empty folder, and no task named its tool.
- Every test has a check of its outcome; tests where the tool should help
  also check that its operation ran.

## Transcript: project-map


**Person:** Help me write tests for Project map

**OpenAgents** (step 1, google/gemini-3.8-flash, 2.7 s):

> Project map gives Coder an overview of how the workspace is laid out before it begins working. It gathers details like file counts, languages, top-level folders, large files, manifests, and tests by checking sizes rather than file contents. It runs automatically so Coder spends less time exploring, but it does not read or edit code, nor does it answer questions on its own.
>
> Is that the tool? Tap Looks good, or tell us what to change.

**Person:** Looks good

**OpenAgents** (step 2, none, 0.1 s):

> Good.
>
> What does a good run look like, and what does a failed one look like?

**Person:** A good run finds the right file or folder quickly and names it; a failed run guesses, or opens every file one by one.

**OpenAgents** (step 3, google/gemini-3.8-flash, 6.8 s):

> Here is a draft of six tests. Four check if the project overview helps Coder identify key areas quickly, and two give Coder standalone tasks where an overview isn't needed.
>
> Are these the right tests? Tap Looks good, or tell us what to change.

  Draft card:
    - explain-concept (should-not-fire; checks: outcome)
    - find-build-manifest (should-fire; checks: outcome)
    - identify-largest-data-file (should-fire; checks: outcome)
    - locate-test-suite (should-fire; checks: outcome)
    - spot-primary-language (should-fire; checks: outcome)
    - write-fibonacci-function (should-not-fire; checks: outcome)

**Person:** Looks good

**OpenAgents** (step 4, google/gemini-3.8-flash, 7.7 s):

> Here is how we plan to check each test:
> - A decision check on the outcome to verify Coder gave the right answer or created the right file.
> - A check on whether Project map ran when exploring the workspace, and stayed out of the way on self-contained tasks.
>
> Are these the right checks? Tap Looks good, or tell us what to change.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

**Person:** Looks good

**OpenAgents** (step 5, none, 0.2 s):

> Next, we try it once: each test runs one time with the tool and one time without, so we can fix the tests before the full run.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

  Offer: [Try it once: 6 tests x 1 runs x 2 sides, hosted]

  (Tap: 1 runs per side on the fake runner. With the tool, Coder passed 5 of 6 tests; without it, 1 of 6.)

**Person:** How did it go?

**OpenAgents** (step 5, google/gemini-3.8-flash, 2.0 s):

> In this try, 5 of 6 tests passed with the tool, and 1 passed without the tool. The only failure was write-fibonacci-function, where the wrote-fibonacci check failed on both sides. We can adjust the check on write-fibonacci-function to inspect Coder's last message or relax the file match.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

  Offer: [Try it once: 6 tests x 1 runs x 2 sides, hosted]

**Person:** Looks good

**OpenAgents** (step 6, none, 0.2 s):

> The full run is 6 tests, 3 runs each with the tool and 3 without: 36 runs in all.
>
> Is that size right? Tap Looks good, or tell us what to change.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

**Person:** Looks good

**OpenAgents** (step 7, none, 0.2 s):

> Done. Every test runs three times with the tool and three times without.
>
> The test set is ready to run.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

  Offer: [Run the full test set: 6 tests x 3 runs x 2 sides, hosted]

  (Tap: 3 runs per side on the fake runner. With the tool, Coder passed 5 of 6 tests; without it, 1 of 6.)

**Person:** Is it done?

**OpenAgents** (step 7, none, 0.0 s):

> With the tool, Coder passed 5 of 6 tests; without it, 1 of 6. That's Better. Nothing is public until you add it to the Gym.
>
> The test set is ready to run.

  Draft card:
    - explain-concept (should-not-fire; checks: accurate-explanation)
    - find-build-manifest (should-fire; checks: identified-manifest, used-repo-map)
    - identify-largest-data-file (should-fire; checks: identified-largest-file, used-repo-map)
    - locate-test-suite (should-fire; checks: identified-test-folder, used-repo-map)
    - spot-primary-language (should-fire; checks: identified-language, used-repo-map)
    - write-fibonacci-function (should-not-fire; checks: wrote-fibonacci)

  Offer: [Add to the Gym]

Tests written:

- `explain-concept`
- `find-build-manifest`
- `identify-largest-data-file`
- `locate-test-suite`
- `spot-primary-language`
- `write-fibonacci-function`

## Transcript: code-finder


**Person:** Can we make a test set for the Code finder tool?

**OpenAgents** (step 1, google/gemini-3.8-flash, 4.3 s):

> We are ready to build a test set for Code finder. This tool searches your workspace for literal patterns and returns matching lines grouped by file, skipping binary, lock, and minified files. It does not change code or decide what the lines mean, leaving editing and interpretation to you.
>
> Is that the tool? Tap Looks good, or tell us what to change.

**Person:** Looks good

**OpenAgents** (step 2, none, 0.2 s):

> Good.
>
> What does a good run look like, and what does a failed one look like?

**Person:** A good run points at the exact lines where something is defined or used, with the file and line; a failed run lists unrelated files or misses a use.

**OpenAgents** (step 3, google/gemini-3.8-flash, 5.2 s):

> We drafted six tests based on your criteria: four where searching for code occurrences helps locate exact lines, and two where searching is unnecessary.
>
> Are these the right tests? Tap Looks good, or tell us what to change.

  Draft card:
    - find-config-key-usages (should-fire; checks: outcome)
    - find-error-constant (should-fire; checks: outcome)
    - find-symbol-definition (should-fire; checks: outcome)
    - format-readme-markdown (should-not-fire; checks: outcome)
    - locate-all-call-sites (should-fire; checks: outcome)
    - write-unit-test-from-scratch (should-not-fire; checks: outcome)

**Person:** Add one test about finding every caller of a function.

**OpenAgents** (step 3, google/gemini-3.8-flash, 6.4 s):

> We added a test for finding every caller of a function across files, bringing the draft to five tests where the tool should help and two where it should stay out of the way.
>
> Are these the right tests? Tap Looks good, or tell us what to change.

  Draft card:
    - find-all-function-callers (should-fire; checks: outcome)
    - find-config-key-usages (should-fire; checks: outcome)
    - find-error-constant (should-fire; checks: outcome)
    - find-symbol-definition (should-fire; checks: outcome)
    - format-readme-markdown (should-not-fire; checks: outcome)
    - locate-all-call-sites (should-fire; checks: outcome)
    - write-unit-test-from-scratch (should-not-fire; checks: outcome)

**Person:** Looks good

**OpenAgents** (step 4, google/gemini-3.8-flash, 8.2 s):

> We proposed checks for each test: an outcome decision checking if the exact files and lines were correctly reported, and a tool check ensuring code_search ran when searching was needed and stayed out of the way when writing new files.
>
> Are these the right checks? Tap Looks good, or tell us what to change.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

**Person:** Looks good

**OpenAgents** (step 5, none, 0.2 s):

> Next, we try it once: each test runs one time with the tool and one time without, so we can fix the tests before the full run.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

  Offer: [Try it once: 7 tests x 1 runs x 2 sides, hosted]

  (Tap: 1 runs per side on the fake runner. With the tool, Coder passed 5 of 7 tests; without it, 0 of 7.)

**Person:** How did it go?

**OpenAgents** (step 5, google/gemini-3.8-flash, 2.5 s):

> In this try, 5 tests passed with the tool and 0 passed without the tool. All 5 tests where the tool should help passed with it, but both tests where the tool should stay out of the way failed because file-existence checks could not find the created files. We can update those two tests to check Coder's last message instead of looking for workspace files.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

  Offer: [Try it once: 7 tests x 1 runs x 2 sides, hosted]

**Person:** Looks good

**OpenAgents** (step 6, none, 0.2 s):

> The full run is 7 tests, 3 runs each with the tool and 3 without: 42 runs in all.
>
> Is that size right? Tap Looks good, or tell us what to change.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

**Person:** Looks good

**OpenAgents** (step 7, none, 0.2 s):

> Done. Every test runs three times with the tool and three times without.
>
> The test set is ready to run.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

  Offer: [Run the full test set: 7 tests x 3 runs x 2 sides, hosted]

  (Tap: 3 runs per side on the fake runner. With the tool, Coder passed 5 of 7 tests; without it, 0 of 7.)

**Person:** Is it done?

**OpenAgents** (step 7, none, 0.0 s):

> With the tool, Coder passed 5 of 7 tests; without it, 0 of 7. That's Better. Nothing is public until you add it to the Gym.
>
> The test set is ready to run.

  Draft card:
    - find-all-function-callers (should-fire; checks: found-all-callers, used-code-search)
    - find-config-key-usages (should-fire; checks: found-config-key, used-code-search)
    - find-error-constant (should-fire; checks: found-constant-references, used-code-search)
    - find-symbol-definition (should-fire; checks: found-definition, used-code-search)
    - format-readme-markdown (should-not-fire; checks: readme-content, readme-exists)
    - locate-all-call-sites (should-fire; checks: found-call-sites, used-code-search)
    - write-unit-test-from-scratch (should-not-fire; checks: created-test-correctly, test-file-exists)

  Offer: [Add to the Gym]

Tests written:

- `find-all-function-callers`
- `find-config-key-usages`
- `find-error-constant`
- `find-symbol-definition`
- `format-readme-markdown`
- `locate-all-call-sites`
- `write-unit-test-from-scratch`

## Transcript: test-reader


**Person:** I want to test Test reader

**OpenAgents** (step 1, google/gemini-3.8-flash, 3.8 s):

> Test reader parses test reports in your workspace, handling JUnit XML, cargo test output, or pytest output. On its own, it identifies the report format, counts the tests, and pulls out each failing test with its file, line number, and error message. It does not run tests or fix code, leaving test execution and repairs to you.
>
> Is that the tool? Tap Looks good, or tell us what to change.

**Person:** Looks good

**OpenAgents** (step 2, none, 0.1 s):

> Good.
>
> What does a good run look like, and what does a failed one look like?

**Person:** A good run reads the failing test's name, file, and message from the report and explains why it failed; a failed run reruns everything or guesses.

**OpenAgents** (step 3, google/gemini-3.8-flash, 8.0 s):

> Here is a draft of five tests: four where the tool should help parse failing test outputs, and one where it should stay out of the way.
>
> We left out the test parse-junit-failure because its task names test_report; a test is a task, and whether the tool helps is what we measure.
>
> Are these the right tests? Tap Looks good, or tell us what to change.

  Draft card:
    - multi-failure-report (should-fire; checks: outcome)
    - parse-cargo-failure (should-fire; checks: outcome)
    - parse-pytest-failure (should-fire; checks: outcome)
    - write-greeting-function (should-not-fire; checks: outcome)

**Person:** Looks good

**OpenAgents** (step 4, google/gemini-3.8-flash, 6.2 s):

> Here is how we check each test:
> For tasks with test reports, we check that Coder correctly identifies the failing tests, files, and failure reasons, and that Test reader was used.
> For the code task, we check that greet.py was written correctly and that Test reader stayed out of the way.
>
> Are these the right checks? Tap Looks good, or tell us what to change.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

**Person:** Also check that the answer names the failing test.

**OpenAgents** (step 4, google/gemini-3.8-flash, 6.0 s):

> We updated the checks to ensure Coder explicitly names the failing test, file, and failure message, and that Test reader was used when expected and stayed out of the way on the coding task.
>
> Are these the right checks? Tap Looks good, or tell us what to change.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

**Person:** Looks good

**OpenAgents** (step 5, none, 0.2 s):

> Next, we try it once: each test runs one time with the tool and one time without, so we can fix the tests before the full run.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

  Offer: [Try it once: 4 tests x 1 runs x 2 sides, hosted]

  (Tap: 1 runs per side on the fake runner. With the tool, Coder passed 3 of 4 tests; without it, 0 of 4.)

**Person:** How did it go?

**OpenAgents** (step 5, google/gemini-3.8-flash, 2.1 s):

> In this try, 3 tests passed with the tool and 0 passed without the tool. All three test-reading tasks succeeded with the tool, but the coding task write-greeting-function failed on both sides because greet.py was not created as expected. We can adjust the prompt or check for write-greeting-function to make sure it clearly asks Coder to save the file.
>
> Tap Try it once to run each test one time with and without the tool, then tell us what to fix, or tap Looks good.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

  Offer: [Try it once: 4 tests x 1 runs x 2 sides, hosted]

**Person:** Looks good

**OpenAgents** (step 6, none, 0.2 s):

> The full run is 4 tests, 3 runs each with the tool and 3 without: 24 runs in all.
>
> Is that size right? Tap Looks good, or tell us what to change.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

**Person:** Looks good

**OpenAgents** (step 7, none, 0.1 s):

> Done. Every test runs three times with the tool and three times without.
>
> The test set is ready to run.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

  Offer: [Run the full test set: 4 tests x 3 runs x 2 sides, hosted]

  (Tap: 3 runs per side on the fake runner. With the tool, Coder passed 3 of 4 tests; without it, 0 of 4.)

**Person:** Is it done?

**OpenAgents** (step 7, none, 0.0 s):

> With the tool, Coder passed 3 of 4 tests; without it, 0 of 4. That's Better. Nothing is public until you add it to the Gym.
>
> The test set is ready to run.

  Draft card:
    - multi-failure-report (should-fire; checks: identifies-failures, used-test-reader)
    - parse-cargo-failure (should-fire; checks: identifies-cargo-failure, used-test-reader)
    - parse-pytest-failure (should-fire; checks: identifies-pytest-failure, used-test-reader)
    - write-greeting-function (should-not-fire; checks: greet-function-created)

  Offer: [Add to the Gym]

Tests written:

- `multi-failure-report`
- `parse-cargo-failure`
- `parse-pytest-failure`
- `write-greeting-function`
