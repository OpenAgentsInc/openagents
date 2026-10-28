# Coaxial services: email, print and schedules

Coaxial pages run in the browser. Some work has to happen on the server
anyway: sending email, running jobs on a schedule, and producing a page you
can print. These services live in the API service (`coax/web/api`, C on Cloud
Run) and share their layout code with the pages, because Coax compiles to C
as well as to WebAssembly.

## The pieces

- **Email.** The API service sends email through Resend from
  `no-reply@openagents.com`.
  - The key is in Secret Manager as `openagents-resend-api-key`.
  - Bodies are HTML with a plain-text part, and attachments are allowed.
  - Each message has an idempotency key, and failed sends are retried.
  - The send log never holds addresses beyond what delivery needs, and never
    holds bodies or the key.
- **Schedules.** Cloud Scheduler calls the API's job endpoint with an OIDC
  token, in the `America/Chicago` time zone.
  - Daylight saving is handled by the scheduler.
  - Jobs are idempotent per day and can be re-run by hand.
- **Coax in the API.** The same Coax modules that lay out a page are compiled
  into the service, so a page, its email and its printed version come from
  one layout.
- **PDF.** A small PDF writer in Coax produces one-page documents:
  - Filament as vector strokes;
  - small text in Helvetica, a PDF base font that needs no embedding and has
    the same widths as the pages' system sans;
  - lines, boxes and checkboxes.
- **Digests.** A project's daily standup, in the same order every day:
  1. where the project is;
  2. yesterday, done and not done;
  3. today's milestones and stretch goals;
  4. blockers and decisions;
  5. a few numbers;
  6. what agents did and plan.

  Unfinished items carry over or go back to the backlog. They are never
  dropped silently.
- **Sprints.** A private project can keep the lighter parts of Scrum:
  - a sprint with one goal;
  - an ordered backlog;
  - a limit on work in progress;
  - stretch goals;
  - a log of priority changes;
  - a short review and retro.

  Priorities can change any time. The change is recorded and the next
  digest says what changed and who agreed.

The tasks are in the Coax project under "Coaxial services"
(https://openagents.com/projects/coax).
