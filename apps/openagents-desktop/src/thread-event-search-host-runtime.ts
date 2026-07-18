import { Effect, Schema } from "effect";

import { openDesktopThreadExportArtifactStore } from "./thread-export-artifact-store.ts";
import { searchDesktopPersistedCanonicalThreadEvents } from "./thread-event-search-artifact-source.ts";
import {
  unavailableDesktopThreadEventSearchResult,
  type DesktopThreadEventSearchResult,
} from "./thread-event-search-bridge-contract.ts";
import {
  registerDesktopThreadEventSearchMainHandler,
  type DesktopThreadEventSearchMainHandlerDependencies,
} from "./thread-event-search-main-handler.ts";
import { openDesktopThreadEventSearchReceiptCatalog } from "./thread-event-search-receipt-catalog.ts";

export class DesktopThreadEventSearchHostRuntimeUnavailable extends Schema.TaggedErrorClass<DesktopThreadEventSearchHostRuntimeUnavailable>()(
  "DesktopThreadEventSearchHostRuntimeUnavailable",
  { stage: Schema.Literal("registration") },
) {}

export type DesktopThreadEventSearchHostRuntimeDependencies = Readonly<{
  artifactStoreDirectory: string;
  receiptCatalogDirectory: string;
  register: DesktopThreadEventSearchMainHandlerDependencies["register"];
  isTrustedSender: DesktopThreadEventSearchMainHandlerDependencies["isTrustedSender"];
}>;

/**
 * Compose private canonical-export evidence behind the fixed bounded search
 * handler. Receipts, artifact bytes, store paths, and native errors remain
 * inside this main-process lifetime and never enter its returned resource.
 */
export const openDesktopThreadEventSearchHostRuntime = Effect.fn(
  "DesktopThreadEventSearchHostRuntime.open",
)(function* (dependencies: DesktopThreadEventSearchHostRuntimeDependencies) {
  const store = openDesktopThreadExportArtifactStore(dependencies.artifactStoreDirectory);
  const catalog = openDesktopThreadEventSearchReceiptCatalog(dependencies.receiptCatalogDirectory);

  return yield* Effect.try({
    try: () =>
      registerDesktopThreadEventSearchMainHandler({
        register: dependencies.register,
        isTrustedSender: dependencies.isTrustedSender,
        search: async (request): Promise<DesktopThreadEventSearchResult> => {
          const listed = catalog.list();
          if (listed.status === "rejected") {
            return unavailableDesktopThreadEventSearchResult();
          }
          return searchDesktopPersistedCanonicalThreadEvents(
            { loadArtifact: store.load },
            {
              receipts: listed.receipts,
              query: request.query,
              ...(request.limit === undefined ? {} : { limit: request.limit }),
            },
          );
        },
      }),
    catch: () => new DesktopThreadEventSearchHostRuntimeUnavailable({ stage: "registration" }),
  });
});
