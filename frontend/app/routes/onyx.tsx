import type { Route, ChangelogEntry } from "../+types/onyx";
import { CHANGELOG } from "../+types/onyx";

export function meta({}: Route.MetaArgs) {
  return [
    { title: "OpenAgents - Mobile App" },
    { name: "description", content: "OpenAgents Mobile App - Onyx" },
  ];
}

function ChangelogEntry({ version, date, changes }: ChangelogEntry) {
  return (
    <div className="border-l-4 border-white pl-4">
      <p className="text-base font-semibold">
        {version} ({date})
      </p>
      <ul className="list-disc ml-6 space-y-2 text-sm">
        {changes.map((change, index) => (
          <li key={index}>{change}</li>
        ))}
      </ul>
    </div>
  );
}

export default function MobileApp() {
  return (
    <div className="container mx-auto px-4 max-w-4xl">
      <h1 className="text-lg font-bold mb-6">
        Meet Onyx - Your Personal AI Agent
      </h1>

      <div className="mb-8">
        <h2 className="text-lg font-bold mb-4">Download Beta v0.1.0</h2>
        <div className="space-y-3">
          <p className="text-sm">
            <strong>Android:</strong>{" "}
            <a
              href="https://github.com/OpenAgentsInc/onyx/releases/download/v0.1.0/Onyx_v0.1.0.apk"
              className="text-white hover:text-white/80 underline"
            >
              Download APK
            </a>
            {" | "}
            <a
              href="https://github.com/OpenAgentsInc/onyx/releases/tag/v0.1.0"
              className="text-white hover:text-white/80 underline"
            >
              Release notes
            </a>
          </p>
          <p className="text-sm">
            <strong>iOS:</strong>{" "}
            <a
              href="https://testflight.apple.com/join/9WfqDTgH"
              className="text-white hover:text-white/80 underline"
            >
              Download via TestFlight
            </a>
          </p>
          <p className="text-sm text-white/70 mt-2">
            Note: The most recent iOS build may still be awaiting approval. The
            TestFlight link will give you access to the previous build and
            automatically download the latest version once it passes review.
          </p>
        </div>
      </div>

      <div className="mb-8">
        <h2 className="text-lg font-bold mb-4">Key Capabilities</h2>
        <ul className="list-disc ml-6 space-y-2 text-sm">
          <li>Voice-powered coding assistance</li>
          <li>Chat history saved to device</li>
          <li>Voice chat via Whisper</li>
          <li>Over-the-air updates</li>
          <li>Experimental Lightning wallet integration</li>
        </ul>
      </div>

      <div className="mb-8">
        <h2 className="text-lg font-bold mb-4">Changelog</h2>
        <div className="space-y-4">
          {CHANGELOG.map((entry, index) => (
            <ChangelogEntry key={index} {...entry} />
          ))}
        </div>
      </div>
    </div>
  );
}
