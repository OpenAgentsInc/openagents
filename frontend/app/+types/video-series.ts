import type { MetaFunction } from "react-router";

export namespace Route {
  export type MetaArgs = Parameters<MetaFunction>[0];
}

export type VideoEntry = {
  title: string;
  date: string;
  description: string;
  tweetUrl: string;
};

export const VIDEOS: VideoEntry[] = [
  {
    title: "Open Knowledge Graph",
    date: "January 7, 2025",
    description: "We review a demo and spec for an open knowledge graph paired with an uncensorable chat channel allowing any contributors - both human and agent, anon or not - to earn bitcoin for building the knowledge base of agents that will help us make sense of all the noise.",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1876857763504046458"
  },
  {
    title: "Agentic OSINT",
    date: "January 3, 2025",
    description: "We revisit our UAP/drone data marketplace idea and brainstorm what an agentic open-source intelligence effort might look like.",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1875380960658681918"
  },
  {
    title: "High-Velocity Bitcoin",
    date: "December 31, 2024",
    description: "We discuss bitcoin as store of value (SoV) vs. medium of exchange (MoE).",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1874317831497462144"
  },
  {
    title: "Code by Voice",
    date: "December 26, 2024",
    description: "What if you had a coding agent in your pocket? Now you do. (Live in v0.0.4) We demo Onyx fixing a bug in its own codebase -- without us typing a single word or line of code.",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1872388364013379949"
  },
  {
    title: "Speak to Onyx",
    date: "December 23, 2024",
    description: "We demonstrate voice chat capabilities in the latest Onyx beta release.",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1871390476705947913"
  },
  {
    title: "Onyx Beta Launch",
    date: "December 20, 2024",
    description: "We launch Onyx v0.0.1, our first beta build for Android and iOS! This build includes a local version of Llama 3.2 1B that runs privately on your phone.",
    tweetUrl: "https://x.com/OpenAgentsInc/status/1870030269916340610"
  }
];
