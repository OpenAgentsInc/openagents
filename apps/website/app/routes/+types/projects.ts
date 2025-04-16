import type { Location, Params, LoaderFunctionArgs, ActionFunctionArgs } from "react-router";

// Define the Project interface to match what's returned from the database
export interface Project {
  id: string;
  name: string;
  description: string;
  icon?: string;
  color: string;
  slugId: string;
  percentComplete: number;
  startDate?: string;
  targetDate?: string;
  content?: string;
  priority: {
    id: string;
    name: string;
    color: string;
  };
  status: {
    id: string;
    name: string;
    color: string;
    type: string;
  };
  health: {
    id: string;
    name: string;
    color: string;
    description: string;
  };
  lead?: {
    id: string;
    name: string;
    email?: string;
    image?: string | null;
  };
  creator?: {
    id: string;
    name: string;
    email?: string;
    image?: string | null;
  };
  members?: Array<{
    id: string;
    name: string;
    email?: string;
    image?: string | null;
  }>;
  teams?: Array<{
    id: string;
    name: string;
    icon?: string;
    color?: string;
    key?: string;
  }>;
  issues?: Array<any>; // Simplified for now, could be expanded if needed
  createdAt: string;
  updatedAt: string;
}

// Define the loader data structure
export interface ProjectLoaderData {
  project: Project;
  options?: {
    workflowStates?: Array<any>;
    labels?: Array<any>;
    teams?: Array<any>;
    users?: Array<any>;
  };
  user?: any;
  error?: string;
}

export namespace Route {
  export interface MetaArgs {
    params: Params;
    data: ProjectLoaderData | Record<string, unknown>;
    location: Location;
  }

  export type LoaderArgs = LoaderFunctionArgs;
  export type ActionArgs = ActionFunctionArgs;
}
