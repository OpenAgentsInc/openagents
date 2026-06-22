# CRM CSV Import Runbook

One-time (and repeatable) import of contacts into the native CRM in
`apps/openagents.com` (epic #5980, sub-issue #5982). This is the migration path
off the old prod DB — hand off a CSV, import once, no ongoing legacy dependency.

## Endpoint

```
POST /api/operator/crm/import      (admin-gated; Bearer OPENAGENTS_ADMIN_API_TOKEN)
```

Two body forms:

- `application/json`:
  ```json
  { "csv": "email,first_name\nada@example.com,Ada", "sourceLabel": "csv:investors",
    "tenant": "tenant.openagents", "listSlug": "investor_roster", "listName": "Investor roster" }
  ```
- `text/csv` with query params: `?sourceLabel=csv:investors&tenant=tenant.openagents&listSlug=investor_roster`

`tenant` defaults to `tenant.openagents` (our own outreach). Customers import
under their own tenant ref — the same engine, isolated.

## CSV format

First row is a header. Recognized columns (case/spacing/hyphen-insensitive;
first matching synonym wins):

| Field | Accepted headers |
|---|---|
| email (required) | `email`, `primary_email`, `email_address`, `work_email` |
| first name | `first_name`, `firstname`, `first`, `given_name` |
| last name | `last_name`, `lastname`, `last`, `surname`, `family_name` |
| full name | `full_name`, `name`, `display_name`, `contact_name` |
| job title | `job_title`, `title`, `role`, `position` |
| company | `company`, `account`, `organization`, `org`, `affiliation`, `fund` |
| secondary email | `secondary_email`, `alt_email`, `other_email` |
| notes | `notes`, `note`, `comment`, `comments` |

Quoted fields, embedded commas/newlines, and doubled-quote escapes are handled.
A `company` value derives/links a `crm_account`. If `listSlug` is given, every
imported contact is added to that list.

## Behavior + counts

- Emails are normalized (trim + lowercase) and de-duped **within the file**.
- Each row resolves to exactly one of: **imported** (new), **updated** (email
  already in this tenant), **duplicate** (seen earlier in this file), or
  **failed** (missing/invalid email, or a write error).
- A `crm_source_import_runs` audit row records the run with all counts; the
  response returns the same summary plus a small email sample and per-line
  errors. A header with no recognizable email column fails the whole run.

## Verify after import

```
GET /api/operator/crm/import-runs            # the audit row + counts
GET /api/operator/crm/contacts?limit=10      # spot-check imported rows
GET /api/operator/crm/contacts?search=<term> # find a specific contact
```
