import { z } from "zod";
import { githubRequest, buildUrl } from "../common/utils.js";

export const GetIssueSchema = z.object({
  owner: z.string(),
  repo: z.string(),
  issue_number: z.number(),
});

export const IssueCommentSchema = z.object({
  owner: z.string(),
  repo: z.string(),
  issue_number: z.number(),
  body: z.string(),
});

export const CreateIssueOptionsSchema = z.object({
  title: z.string(),
  body: z.string().optional(),
  assignees: z.array(z.string()).optional(),
  milestone: z.number().optional(),
  labels: z.array(z.string()).optional(),
});

export const CreateIssueSchema = z.object({
  owner: z.string(),
  repo: z.string(),
  ...CreateIssueOptionsSchema.shape,
});

export const ListIssuesOptionsSchema = z.object({
  owner: z.string(),
  repo: z.string(),
  direction: z.enum(["asc", "desc"]).optional(),
  labels: z.array(z.string()).optional(),
  page: z.number().optional(),
  per_page: z.number().optional(),
  since: z.string().optional(),
  sort: z.enum(["created", "updated", "comments"]).optional(),
  state: z.enum(["open", "closed", "all"]).optional(),
});

export const UpdateIssueOptionsSchema = z.object({
  owner: z.string(),
  repo: z.string(),
  issue_number: z.number(),
  title: z.string().optional(),
  body: z.string().optional(),
  assignees: z.array(z.string()).optional(),
  milestone: z.number().optional(),
  labels: z.array(z.string()).optional(),
  state: z.enum(["open", "closed"]).optional(),
});

export async function getIssue(owner: string, repo: string, issue_number: number, authOptions?: { token?: string }) {
  return githubRequest(`https://api.github.com/repos/${owner}/${repo}/issues/${issue_number}`, {
    token: authOptions?.token
  });
}

export async function addIssueComment(
  owner: string,
  repo: string,
  issue_number: number,
  body: string,
  githubToken: string,
) {
  return githubRequest(`https://api.github.com/repos/${owner}/${repo}/issues/${issue_number}/comments`, {
    method: "POST",
    body: { body },
    headers: {
      Authorization: `Bearer ${githubToken}`
    }
  });
}

export async function createIssue(
  owner: string,
  repo: string,
  options: z.infer<typeof CreateIssueOptionsSchema>,
  authOptions?: { token?: string }
) {
  return githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/issues`,
    {
      method: "POST",
      body: options,
      token: authOptions?.token
    }
  );
}

export async function listIssues(
  owner: string,
  repo: string,
  options: Omit<z.infer<typeof ListIssuesOptionsSchema>, "owner" | "repo">,
  authOptions?: { token?: string }
) {
  const urlParams: Record<string, string | undefined> = {
    direction: options.direction,
    labels: options.labels?.join(","),
    page: options.page?.toString(),
    per_page: options.per_page?.toString(),
    since: options.since,
    sort: options.sort,
    state: options.state
  };

  return githubRequest(
    buildUrl(`https://api.github.com/repos/${owner}/${repo}/issues`, urlParams),
    { token: authOptions?.token }
  );
}

export async function updateIssue(
  owner: string,
  repo: string,
  issue_number: number,
  options: Omit<z.infer<typeof UpdateIssueOptionsSchema>, "owner" | "repo" | "issue_number">,
  authOptions?: { token?: string }
) {
  return githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/issues/${issue_number}`,
    {
      method: "PATCH",
      body: options,
      token: authOptions?.token
    }
  );
}
