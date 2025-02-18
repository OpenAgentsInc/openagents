import type { MetaFunction } from "react-router";

export namespace Route {
  export type MetaArgs = Parameters<MetaFunction>[0];
}

export type ChangelogEntry = {
  version: string;
  date: string;
  changes: string[];
};

export const CHANGELOG: ChangelogEntry[] = [
  {
    version: "v0.1.0",
    date: "January 3, 2025",
    changes: [
      "Basic Bitcoin Lightning wallet (experimental)",
      "Improved error handling",
      "Performance optimizations"
    ]
  },
  {
    version: "v0.0.9",
    date: "January 1, 2025",
    changes: [
      "Added settings page",
      "Dark mode toggle",
      "Chat history search"
    ]
  },
  {
    version: "v0.0.8",
    date: "December 31, 2024",
    changes: [
      "Chat history export",
      "Voice recording improvements",
      "Bug fixes"
    ]
  },
  {
    version: "v0.0.7",
    date: "December 30, 2024",
    changes: [
      "Chat history persistence",
      "UI polish",
      "Performance improvements"
    ]
  },
  {
    version: "v0.0.6",
    date: "December 29, 2024",
    changes: [
      "New chat/recording UI",
      "Multiple GitHub tokens support",
      "Better error messages"
    ]
  },
  {
    version: "v0.0.5",
    date: "December 27, 2024",
    changes: [
      "Voice recording stability",
      "Chat history fixes",
      "UI improvements"
    ]
  },
  {
    version: "v0.0.4",
    date: "December 26, 2024",
    changes: [
      "Coder tools via GitHub API",
      "Upgraded to Gemini 1.5 Pro model",
      "Initial voice chat support"
    ]
  },
  {
    version: "v0.0.3",
    date: "December 24, 2024",
    changes: [
      "Basic chat functionality",
      "Initial GitHub integration",
      "Bug fixes"
    ]
  },
  {
    version: "v0.0.2",
    date: "December 22, 2024",
    changes: [
      "UI improvements",
      "Performance optimizations",
      "Bug fixes"
    ]
  },
  {
    version: "v0.0.1",
    date: "December 20, 2024",
    changes: [
      "Initial beta release",
      "Basic chat interface",
      "Local Llama 3.2 1B model"
    ]
  }
];
