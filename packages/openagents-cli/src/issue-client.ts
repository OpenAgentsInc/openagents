/**
 * The issue API client.
 *
 * The routes it calls answer with the GitHub-compatible shapes this
 * repository publishes at `GET /api/v1`, so the client keeps the server's
 * bodies intact and adds only what a terminal caller cannot do for itself:
 * paging a list that has no `per_page` parameter, and reporting a rejected
 * write by the field the server named.
 */

import { Effect, Layer } from "effect";
import * as Context from "effect/Context";

import { ApiTransport } from "./api-transport.js";
import { InputError, type CliError } from "./errors.js";
import type { AuthenticatedApi, RepositoryTarget } from "./repository-client.js";
import {
  asNumber,
  asRecord,
  makeTrackerRequest,
  repositoryPath,
  type TrackerRequest,
} from "./tracker-request.js";

/** The largest list the CLI will page for; the server holds 25 to a page. */
export const MAXIMUM_ISSUE_LIST_LIMIT = 1_000;

export type IssueState = "all" | "closed" | "open";

export interface IssueListInput extends AuthenticatedApi, RepositoryTarget {
  readonly limit: number;
  readonly state?: IssueState;
  readonly label?: string;
  readonly assignee?: string;
  readonly milestone?: string;
  readonly search?: string;
  readonly blocked?: boolean;
}

export interface IssueListResult {
  /** The server's own pagination object, so the reported total is its total. */
  readonly pagination: Record<string, unknown>;
  readonly issues: ReadonlyArray<unknown>;
}

export interface IssueCreateInput extends AuthenticatedApi, RepositoryTarget {
  readonly title: string;
  readonly body?: string;
  readonly labels?: ReadonlyArray<string>;
  readonly assignees?: ReadonlyArray<string>;
  readonly milestone?: number;
}

export interface IssueNumberInput extends AuthenticatedApi, RepositoryTarget {
  readonly number: number;
}

interface IssueClientInterface {
  readonly list: (input: IssueListInput) => Effect.Effect<IssueListResult, CliError>;
  readonly view: (input: IssueNumberInput) => Effect.Effect<unknown, CliError>;
  readonly create: (input: IssueCreateInput) => Effect.Effect<unknown, CliError>;
  readonly setState: (
    input: IssueNumberInput & { readonly state: "closed" | "open" },
  ) => Effect.Effect<unknown, CliError>;
  readonly comments: (input: IssueNumberInput) => Effect.Effect<unknown, CliError>;
  readonly comment: (
    input: IssueNumberInput & { readonly body: string },
  ) => Effect.Effect<unknown, CliError>;
  readonly labels: (input: IssueNumberInput) => Effect.Effect<unknown, CliError>;
  readonly addLabels: (
    input: IssueNumberInput & { readonly labels: ReadonlyArray<string> },
  ) => Effect.Effect<unknown, CliError>;
  readonly removeLabel: (
    input: IssueNumberInput & { readonly label: string },
  ) => Effect.Effect<unknown, CliError>;
  readonly assignees: (input: IssueNumberInput) => Effect.Effect<unknown, CliError>;
  readonly addAssignees: (
    input: IssueNumberInput & { readonly assignees: ReadonlyArray<string> },
  ) => Effect.Effect<unknown, CliError>;
  readonly removeAssignees: (
    input: IssueNumberInput & { readonly assignees: ReadonlyArray<string> },
  ) => Effect.Effect<unknown, CliError>;
  readonly dependencies: (input: IssueNumberInput) => Effect.Effect<unknown, CliError>;
  readonly addDependencies: (
    input: IssueNumberInput & { readonly blockedBy: ReadonlyArray<number> },
  ) => Effect.Effect<unknown, CliError>;
  readonly removeDependency: (
    input: IssueNumberInput & { readonly blockedBy: number },
  ) => Effect.Effect<unknown, CliError>;
}

export class IssueClient extends Context.Service<IssueClient, IssueClientInterface>()(
  "@openagentsinc/cli/IssueClient",
) {}

const issuesPath = (input: RepositoryTarget) => `${repositoryPath(input.owner, input.repo)}/issues`;

const issuePath = (input: RepositoryTarget & { readonly number: number }) =>
  `${issuesPath(input)}/${input.number}`;

const listQuery = (input: IssueListInput, page: number): string => {
  const parameters = new URLSearchParams({ state: input.state ?? "open", page: String(page) });
  // The list route names its search parameter `q` and its label parameter
  // `labels`; the flags read the way a person says them.
  if (input.label !== undefined) parameters.set("labels", input.label);
  if (input.assignee !== undefined) parameters.set("assignee", input.assignee);
  if (input.milestone !== undefined) parameters.set("milestone", input.milestone);
  if (input.search !== undefined) parameters.set("q", input.search);
  if (input.blocked !== undefined) parameters.set("blocked", String(input.blocked));
  return parameters.toString();
};

const paged = (request: TrackerRequest) =>
  Effect.fn("IssueClient.list")(function* (input: IssueListInput) {
    if (!Number.isInteger(input.limit) || input.limit < 1) {
      return yield* new InputError({ message: "--limit must be a positive integer." });
    }
    if (input.limit > MAXIMUM_ISSUE_LIST_LIMIT) {
      return yield* new InputError({
        message: `--limit must be at most ${MAXIMUM_ISSUE_LIST_LIMIT}.`,
      });
    }

    const collected: Array<unknown> = [];
    let pagination: Record<string, unknown> = {};
    let page = 1;

    // The route publishes no `per_page`, so a limit above one page is only
    // reachable by asking for the next page until the server's own total is
    // covered.
    while (collected.length < input.limit) {
      const body = yield* request("list issues", {
        origin: input.origin,
        token: input.token,
        method: "GET",
        path: `${issuesPath(input)}?${listQuery(input, page)}`,
        acceptedStatuses: [200],
      });
      const envelope = asRecord(body);
      pagination = asRecord(envelope["pagination"]);
      const rows = envelope["issues"];
      const issues = Array.isArray(rows) ? rows : [];
      collected.push(...issues);
      if (issues.length === 0) break;
      const total = asNumber(pagination["total"]);
      if (total !== undefined && collected.length >= total) break;
      const totalPages = asNumber(pagination["total_pages"]);
      if (totalPages !== undefined && page >= totalPages) break;
      page += 1;
    }

    return { pagination, issues: collected.slice(0, input.limit) } satisfies IssueListResult;
  });

export const issueClientLayer = Layer.effect(
  IssueClient,
  Effect.gen(function* () {
    const transport = yield* ApiTransport;
    const request = makeTrackerRequest(transport);

    return IssueClient.of({
      list: paged(request),

      view: (input) =>
        request("view an issue", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: issuePath(input),
          acceptedStatuses: [200],
        }),

      create: (input) =>
        request("create an issue", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: issuesPath(input),
          body: {
            title: input.title,
            ...(input.body === undefined ? {} : { body: input.body }),
            ...(input.labels === undefined || input.labels.length === 0
              ? {}
              : { labels: input.labels }),
            ...(input.assignees === undefined || input.assignees.length === 0
              ? {}
              : { assignees: input.assignees }),
            ...(input.milestone === undefined ? {} : { milestone: input.milestone }),
          },
          acceptedStatuses: [201],
        }),

      // A `PATCH` that carries `body` replaces the issue text, so a state
      // change sends `state` and nothing else.
      setState: (input) =>
        request("change issue state", {
          origin: input.origin,
          token: input.token,
          method: "PATCH",
          path: issuePath(input),
          body: { state: input.state },
          acceptedStatuses: [200],
        }),

      comments: (input) =>
        request("list issue comments", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${issuePath(input)}/comments`,
          acceptedStatuses: [200],
        }),

      comment: (input) =>
        request("comment on an issue", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${issuePath(input)}/comments`,
          body: { body: input.body },
          acceptedStatuses: [201],
        }),

      labels: (input) =>
        request("list issue labels", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${issuePath(input)}/labels`,
          acceptedStatuses: [200],
        }),

      addLabels: (input) =>
        request("label an issue", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${issuePath(input)}/labels`,
          body: { labels: input.labels },
          acceptedStatuses: [200, 201],
        }),

      removeLabel: (input) =>
        request("remove an issue label", {
          origin: input.origin,
          token: input.token,
          method: "DELETE",
          path: `${issuePath(input)}/labels/${encodeURIComponent(input.label)}`,
          acceptedStatuses: [200],
        }),

      assignees: (input) =>
        request("list issue assignees", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${issuePath(input)}/assignees`,
          acceptedStatuses: [200],
        }),

      addAssignees: (input) =>
        request("assign an issue", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${issuePath(input)}/assignees`,
          body: { assignees: input.assignees },
          acceptedStatuses: [200, 201],
        }),

      // The route reads the logins from a body rather than the path, so this
      // `DELETE` carries one.
      removeAssignees: (input) =>
        request("unassign an issue", {
          origin: input.origin,
          token: input.token,
          method: "DELETE",
          path: `${issuePath(input)}/assignees`,
          body: { assignees: input.assignees },
          acceptedStatuses: [200],
        }),

      dependencies: (input) =>
        request("read issue prerequisites", {
          origin: input.origin,
          token: input.token,
          method: "GET",
          path: `${issuePath(input)}/dependencies`,
          acceptedStatuses: [200],
        }),

      addDependencies: (input) =>
        request("add issue prerequisites", {
          origin: input.origin,
          token: input.token,
          method: "POST",
          path: `${issuePath(input)}/dependencies`,
          body: { blocked_by: input.blockedBy },
          acceptedStatuses: [200, 201],
        }),

      // The prerequisite is a path segment here, not a body key.
      removeDependency: (input) =>
        request("remove an issue prerequisite", {
          origin: input.origin,
          token: input.token,
          method: "DELETE",
          path: `${issuePath(input)}/dependencies/${input.blockedBy}`,
          acceptedStatuses: [200],
        }),
    });
  }),
);
