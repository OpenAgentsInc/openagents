import type { Location, Params, LoaderFunctionArgs, ActionFunctionArgs } from "react-router";
import type { Issue, Status, User } from "@/store/issues-store";
import type { LabelInterface } from "@/mock-data/labels";
import type { Project } from "@/mock-data/projects";

// Define the loader data structure
export interface IssueLoaderData {
  issue: Issue;
  options?: {
    workflowStates?: Status[];
    labels?: LabelInterface[];
    teams?: any[];
    users?: User[];
    projects?: Project[];
  };
  user?: any;
  error?: string;
}

export namespace Route {
  export interface MetaArgs {
    params: Params;
    data: IssueLoaderData | Record<string, unknown>;
    location: Location;
  }

  export type LoaderArgs = LoaderFunctionArgs;
  export type ActionArgs = ActionFunctionArgs;
}