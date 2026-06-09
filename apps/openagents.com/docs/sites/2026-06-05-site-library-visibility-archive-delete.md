# Site Library Visibility, Archive, And Delete Contract

Date: 2026-06-05

Status: implemented first backend/API slice for issue #205.

## Purpose

The Site library gives authenticated users and operators a safe way to find and
manage generated Sites without exposing archived, disabled, or stale builder
session data.

The first slice is intentionally conservative. It adds server-side controls and
customer-safe projections before richer browser library navigation.

## API Surface

Authenticated browser sessions may call:

- `GET /api/sites?scope=mine`
- `GET /api/sites?scope=public`
- `GET /api/sites?scope=recent`
- `POST /api/sites/:siteId/access`
- `POST /api/sites/:siteId/archive`
- `POST /api/sites/:siteId/delete`

Mutating archive/delete calls require `Idempotency-Key`.

`mine` lists active Sites owned by the current user. `public` lists active
public Sites only. `recent` lists the current user's active Sites unless the
session is an OpenAgents admin, in which case it returns active recent Sites
across owners.

## Projection

The Site library projection includes:

- Site id, slug, title, status, access mode, visibility, owner, and order ref;
- active deployment id/status/url and active version id;
- version/deployment counts;
- creation/update/archive timestamps;
- `canManage`, derived from owner/admin authority.

It does not include source archives, build logs, raw prompts, secrets, customer
private data, runtime credentials, or provider payloads.

## Authority

Owners and OpenAgents admins may hide, archive, or delete a Site. Admins may
manage any Site.

Non-admin owners may move a Site to private/team visibility. Restoring a hidden
Site to public requires operator review unless the Site is already public. This
keeps customer controls from bypassing the existing public-launch checklist.

## Archive Semantics

Archive is reversible in future product work, but this slice treats archived
Sites as hidden from active customer/public projections:

- `site_projects.status = 'archived'`;
- `site_projects.archived_at` is set;
- access is reduced to `owner_admins`;
- visibility is reduced to `private`;
- active version/deployment pointers are cleared;
- active deployments are disabled;
- linked builder sessions are archived.

Because the runtime resolver requires `archived_at IS NULL`, active public
slugs and dedicated version URLs stop resolving after archive.

## Delete Semantics

Delete is a soft delete:

- `site_projects.status = 'disabled'`;
- `site_projects.archived_at` is set;
- access is reduced to `owner_admins`;
- visibility is reduced to `private`;
- active version/deployment pointers are cleared;
- active deployments are disabled;
- linked builder sessions are archived.

Physical deletion of source artifacts, build logs, receipts, and historical
records is intentionally not part of this slice. Retention policy needs a
separate reviewed issue because those records may be needed for customer
support, auditability, receipts, and abuse investigation.

## Builder Session Guardrail

Customer builder-session reads, event streams, file lists, file reads, and
message appends now re-check the linked Site lifecycle. If the linked Site is
archived or disabled, the route returns the same not-found response used for an
unauthorized or missing session.

This prevents stale session URLs from exposing archived Site details.

## Follow-Up

The next browser UX pass should add a first-class Site library page with richer
filtering, favorites, restore flows, admin retention controls, and clearer
version/deployment history. The current API is enough for safe product wiring
and automated tests.
