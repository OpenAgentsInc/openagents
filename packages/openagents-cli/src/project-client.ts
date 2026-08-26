/**
 * The Projects V2 API client.
 *
 * Every project route this repository publishes is repository-scoped, so the
 * client takes an owner and a repository the same way the issue client does
 * and never pins a board to one namespace.
 */

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import type { CliError } from "./errors.js";
import type { AuthenticatedApi, RepositoryTarget } from "./repository-client.js";
import { makeTrackerRequest, repositoryPath } from "./tracker-request.js";

export interface ProjectListInput extends AuthenticatedApi, RepositoryTarget {
  readonly archived: boolean;
}

export interface ProjectNumberInput extends AuthenticatedApi, RepositoryTarget {
  readonly number: number;
}

export interface ProjectCreateInput extends AuthenticatedApi, RepositoryTarget {
  readonly title: string;
  readonly description?: string;
}

export interface ProjectEditInput extends ProjectNumberInput {
  readonly title?: string;
  readonly description?: string;
  readonly state?: string;
  readonly archived?: boolean;
}

export interface ProjectItemInput extends ProjectNumberInput {
  readonly itemId: string;
}

interface ProjectClientInterface {
  readonly list: (input: ProjectListInput) => Effect.Effect<unknown, CliError>;
  readonly view: (input: ProjectNumberInput) => Effect.Effect<unknown, CliError>;
  readonly create: (input: ProjectCreateInput) => Effect.Effect<unknown, CliError>;
  readonly edit: (input: ProjectEditInput) => Effect.Effect<unknown, CliError>;
  readonly delete: (input: ProjectNumberInput) => Effect.Effect<unknown, CliError>;
  readonly fields: (input: ProjectNumberInput) => Effect.Effect<unknown, CliError>;
  readonly items: (input: ProjectNumberInput) => Effect.Effect<unknown, CliError>;
  readonly addItem: (
    input: ProjectNumberInput & { readonly issueNumber: number },
  ) => Effect.Effect<unknown, CliError>;
  readonly setItemValues: (
    input: ProjectItemInput & { readonly values: Readonly<Record<string, string>> },
  ) => Effect.Effect<unknown, CliError>;
  readonly moveItem: (
    input: ProjectItemInput & {
      readonly values: Readonly<Record<string, string>>;
      readonly position?: number;
    },
  ) => Effect.Effect<unknown, CliError>;
  readonly removeItem: (input: ProjectItemInput) => Effect.Effect<unknown, CliError>;
}

export class ProjectClient extends Context.Service<ProjectClient, ProjectClientInterface>()(
  "@openagentsinc/cli/ProjectClient",
) {}

const projectsPath = (input: RepositoryTarget) =>
  `${repositoryPath(input.owner, input.repo)}/projectsV2`;

const projectPath = (input: RepositoryTarget & { readonly number: number }) =>
  `${projectsPath(input)}/${input.number}`;

const itemPath = (input: ProjectItemInput) =>
  `${projectPath(input)}/items/${encodeURIComponent(input.itemId)}`;

export const projectClientLayer = Layer.effect(
  ProjectClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;
    const request = makeTrackerRequest(transport);

    return ProjectClient.of({
      list: (input) =>
        request("list projects", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${projectsPath(input)}${input.archived ? "?archived=true" : ""}`,
          acceptedStatuses: [200],
        }),

      view: (input) =>
        request("view a project", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: projectPath(input),
          acceptedStatuses: [200],
        }),

      create: (input) =>
        request("create a project", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: projectsPath(input),
          body: {
            title: input.title,
            ...(input.description === undefined ? {} : { description: input.description }),
          },
          acceptedStatuses: [201],
        }),

      edit: (input) =>
        request("edit a project", {
          origin: input.origin,
          token: input.token,
          method: "PATCH",
          path: projectPath(input),
          body: {
            ...(input.title === undefined ? {} : { title: input.title }),
            ...(input.description === undefined ? {} : { description: input.description }),
            ...(input.state === undefined ? {} : { state: input.state }),
            ...(input.archived === undefined ? {} : { archived: input.archived }),
          },
          acceptedStatuses: [200],
        }),

      // The API refuses a board that is not archived, so the two-step is the
      // server's policy rather than a client convention. See `project archive`.
      delete: (input) =>
        request("delete a project", {
          origin: input.origin,
          token: input.token,
          method: "DELETE",
          path: projectPath(input),
          acceptedStatuses: [200, 204],
        }),

      fields: (input) =>
        request("list project fields", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${projectPath(input)}/fields`,
          acceptedStatuses: [200],
        }),

      items: (input) =>
        request("list project items", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${projectPath(input)}/items`,
          acceptedStatuses: [200],
        }),

      // A repeated add answers 200 with the membership the board already has,
      // so both statuses are the same success.
      addItem: (input) =>
        request("add a project item", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${projectPath(input)}/items`,
          body: { issue_number: input.issueNumber },
          acceptedStatuses: [200, 201],
        }),

      setItemValues: (input) =>
        request("set project item values", {
          origin: input.origin,
          token: input.token,
          method: "PATCH",
          path: itemPath(input),
          body: { values: input.values },
          acceptedStatuses: [200],
        }),

      moveItem: (input) =>
        request("move a project item", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${itemPath(input)}/move`,
          body: {
            values: input.values,
            ...(input.position === undefined ? {} : { position: input.position }),
          },
          acceptedStatuses: [200],
        }),

      removeItem: (input) =>
        request("remove a project item", {
          origin: input.origin,
          token: input.token,
          method: "DELETE",
          path: itemPath(input),
          acceptedStatuses: [200, 204],
        }),
    });
  }),
);
