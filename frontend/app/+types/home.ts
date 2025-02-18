import type { MetaFunction } from "react-router";

export namespace Route {
  export type MetaArgs = Parameters<MetaFunction>[0];
}

export type BlogPost = {
  date: string;
  title: string;
  content: string;
};

export const BLOG_POSTS: BlogPost[] = [
  {
    date: "February 2, 2025",
    title: "Chains of Thought and Action",
    content: "We wrote a post introducing chains of thought and action. Read it [here](/cota)."
  },
  {
    date: "January 16, 2025",
    title: "Repo Map Tool",
    content: "We've released a small tool for generating a concise map of a GitHub repo, optimized for LLM chats & agents, using code from [Aider](https://aider.chat). [Try it here!](/repomap)"
  },
  {
    date: "January 14, 2025",
    title: "New Website",
    content: "We have a new website! It uses Rust, HTMX, and a new font Berkeley Mono from the US Graphics Company, from which we also drew inspiration on design."
  }
];
