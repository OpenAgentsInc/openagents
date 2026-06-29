// Public, agent-readable free-tier data-sharing disclosure route (#6296).
//
// GET /api/public/free-tier-data-sharing serves the canonical, code-accurate
// data-sharing terms for the free Khala API so the disclosure is discoverable
// over the documented API surface, not only in human UI. Read-only, no auth, no
// DB, no secrets — it re-projects the static disclosure object.

import { Effect } from 'effect'

import { methodNotAllowed, noStoreJsonResponse } from '../http/responses'
import { freeTierDataSharingDisclosure } from './free-tier-data-sharing-disclosure'

export const handleFreeTierDataSharingDisclosureApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(noStoreJsonResponse(freeTierDataSharingDisclosure()))
