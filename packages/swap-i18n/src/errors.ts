// Resolution from protocol error identifiers to rendered messages.
//
// `messageForSwpError` is total over the typed identifier set, so no mapped
// identifier can ever render blank. `messageForReportedError` is the boundary
// for untrusted input: an unrecognized report renders the typed
// "unrecognized" message — the raw counterparty string is never shown.

import type { Catalog } from "./catalog.js";
import { UNRECOGNIZED_ERROR_KEY, swpErrorMessageKey } from "./error-table.js";
import { isSwpErrorIdentifier, type SwpErrorIdentifier } from "./identifiers.js";

/** The rendered message for a typed MKT-SWP §17 error identifier. */
export const messageForSwpError = (catalog: Catalog, identifier: SwpErrorIdentifier): string =>
  catalog[swpErrorMessageKey(identifier)];

/**
 * The rendered message for an error report whose identifier has not been
 * validated — for example one taken directly from a counterparty record.
 * Never renders the input itself.
 */
export const messageForReportedError = (catalog: Catalog, reported: string): string =>
  isSwpErrorIdentifier(reported)
    ? messageForSwpError(catalog, reported)
    : catalog[UNRECOGNIZED_ERROR_KEY];
