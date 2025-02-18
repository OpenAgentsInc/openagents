import type { MetaFunction } from "react-router";

export namespace Route {
  export type MetaArgs = Parameters<MetaFunction>[0];
}

export type RepoMapResponse = {
  repo_map: string;
};
