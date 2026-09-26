---
id: tool.react-testing-library-async
version: 1
kind: tool
title: Test React components through user-visible behavior with async-aware queries
summary: >-
  Query by role and accessible name, drive input with user-event awaited calls,
  wait for asynchronous UI with findBy queries or waitFor, mock the network at
  the fetch boundary, and remember jsdom has no layout. Tests written this way
  match how a grader drives a form or page.
tags: [react, testing-library, jest, vitest, jsdom, frontend-testing, forms, accessibility]
applies_when: >-
  Writing or fixing tests for React components or pages (forms, validation,
  submission, loading and error states), or building components that a
  hidden test suite will exercise through the DOM.
status: admitted
author: claflampernton (hand-written, Claude Opus 5.5)
provenance:
  written_from:
    - reference
  cites:
    - "Testing Library documentation: About Queries (priority), Async Methods (findBy, waitFor), ByRole"
    - "Testing Library user-event v14 documentation: userEvent.setup and awaited interactions"
    - "React documentation: act; react-dom/test-utils deprecation"
    - "W3C, WAI-ARIA 1.2 and HTML-AAM: roles and accessible name computation"
evidence:
  - "admitted 2026-09-26 by review: round3-oos-review"
---

## Details

**Queries.** Prefer `getByRole("button", { name: /submit/i })`,
`getByLabelText`, then `getByText`; fall back to `getByTestId` only when
nothing user-visible identifies the element. `getBy*` throws when absent,
`queryBy*` returns null (use it to assert absence), `findBy*` returns a promise
that retries until the element appears. Role queries work only if the markup
is accessible: inputs need an associated `<label>` (or `aria-label`), error
messages are found reliably when rendered with `role="alert"` or linked by
`aria-describedby`, and buttons need text.

**Interaction.** `const user = userEvent.setup()` then `await user.type(input,
"text")`, `await user.click(button)`, `await user.keyboard("{Enter}")`.
Unawaited interactions and state updates after the test ends produce "not
wrapped in act(...)" warnings and flaky assertions. `fireEvent` dispatches a
single event and skips the focus, keydown, and input sequence a real user
produces.

**Async.** After an action that triggers a fetch or timer, `await
screen.findByText(...)` or `await waitFor(() => expect(...))`; put only the
assertion inside `waitFor`, not the action. With fake timers, advance them
(`vi.advanceTimersByTime` / `jest.advanceTimersByTime`) and set up
user-event with `advanceTimers`.

**Environment limits.** jsdom has no layout (sizes are 0), no
`IntersectionObserver`, `ResizeObserver`, or `matchMedia` unless stubbed, and
no real navigation. Mock the network at `fetch` or with a request-mocking
library rather than mocking your own components. Run the suite once, not in
watch mode (`vitest run`, `CI=true npm test`, or `npx jest`).

**Forms specifically.** Cover: submitting empty, each invalid field (message
shown, focus or `aria-invalid` set), a valid submission (request payload and
success state), the submit button disabled or guarded against double submit
while pending, and server error handling.

## How to check

Delete or break the behavior a test claims to cover (remove the validation,
return an error from the mock) and confirm the test fails. Run the suite
twice in a row and in random order if supported to catch leaked state
between tests (clean up mocks and timers in `afterEach`).
