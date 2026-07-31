// EP263-LK: a participant-admission authority that only its own test calls is
// not an authority. `recordSarahLiveKitParticipantJoin` shipped for months
// reachable from nothing but `sarah-realtime-voice-routes.test.ts`, so the
// `duplicate_participant_refused` release-gate drill could not be satisfied by
// any credential or permission — the refusal it implements was never on a live
// path. This guard fails the build if one of these functions loses its last
// production caller again.
//
// A reference from a test file never counts. Neither does the file that defines
// the function: a self-reference is what dead code looks like.
import { globSync, readFileSync } from "node:fs";
import { join } from "node:path";

const root = process.argv[2] ?? ".";

const guarded = [
  {
    symbol: "recordSarahLiveKitParticipantJoin",
    definedIn: "apps/openagents.com/workers/api/src/sarah-realtime-voice-routes.ts",
    records: "the first admission of a dispatched participant into its bound room",
  },
  {
    symbol: "recordLiveKitParticipantJoin",
    definedIn: "packages/khala-sync-server/src/sarah-realtime-voice-store.ts",
    records: "the owner/Sarah seat on a LiveKit room binding, refusing a duplicate",
  },
  {
    symbol: "bindParticipant",
    definedIn: "packages/khala-sync-server/src/sarah-livekit-room-authority-store.ts",
    records: "one community room seat under one client, refusing a duplicate",
  },
];

const isTestSource = (file) =>
  /(^|\/)(tests?|__tests__)\//u.test(file) || /\.(test|spec)\.[cm]?tsx?$/u.test(file);

const sources = globSync(["apps/**/*.ts", "packages/**/*.ts", "clients/**/*.ts"], {
  cwd: root,
  exclude: ["**/node_modules/**", "**/dist/**", "**/build/**"],
})
  .map((file) => file.split("\\").join("/"))
  .filter((file) => !isTestSource(file));

const failures = [];
for (const entry of guarded) {
  const pattern = new RegExp(`\\b${entry.symbol}\\b`, "u");
  const callers = sources.filter(
    (file) => file !== entry.definedIn && pattern.test(readFileSync(join(root, file), "utf8")),
  );
  if (callers.length === 0) {
    failures.push(
      `${entry.symbol} (${entry.definedIn}) records ${entry.records}, but no production ` +
        "source outside its own file references it. Wire it back into the live path, or " +
        "delete it and this entry — do not leave a refusal that nothing can reach.",
    );
  }
}

if (failures.length > 0) {
  console.error("Sarah participant-join authority guard failed:");
  for (const failure of failures) console.error(`  - ${failure}`);
  process.exit(1);
}

console.log(`Sarah participant-join authority guard: ${guarded.length} authorities have callers.`);
