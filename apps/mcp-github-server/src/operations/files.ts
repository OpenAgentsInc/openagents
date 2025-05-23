import { z } from "zod";
import { githubRequest } from "../common/utils.js";
import {
  GitHubContentSchema,
  GitHubAuthorSchema,
  GitHubTreeSchema,
  GitHubCommitSchema,
  GitHubReferenceSchema,
  GitHubFileContentSchema,
} from "../common/types.js";

// Schema definitions
export const FileOperationSchema = z.object({
  path: z.string(),
  content: z.string(),
});

export const CreateOrUpdateFileSchema = z.object({
  owner: z.string().describe("Repository owner (username or organization)"),
  repo: z.string().describe("Repository name"),
  path: z.string().describe("Path where to create/update the file"),
  content: z.string().describe("Content of the file"),
  message: z.string().describe("Commit message"),
  branch: z.string().describe("Branch to create/update the file in"),
  sha: z.string().optional().describe("SHA of the file being replaced (required when updating existing files)"),
});

export const GetFileContentsSchema = z.object({
  owner: z.string().describe("Repository owner (username or organization)"),
  repo: z.string().describe("Repository name"),
  path: z.string().describe("Path to the file or directory"),
  branch: z.string().optional().describe("Branch to get contents from"),
});

export const PushFilesSchema = z.object({
  owner: z.string().describe("Repository owner (username or organization)"),
  repo: z.string().describe("Repository name"),
  branch: z.string().describe("Branch to push to (e.g., 'main' or 'master')"),
  files: z.array(FileOperationSchema).describe("Array of files to push"),
  message: z.string().describe("Commit message"),
});

export const GitHubCreateUpdateFileResponseSchema = z.object({
  content: GitHubFileContentSchema.nullable(),
  commit: z.object({
    sha: z.string(),
    node_id: z.string(),
    url: z.string(),
    html_url: z.string(),
    author: GitHubAuthorSchema,
    committer: GitHubAuthorSchema,
    message: z.string(),
    tree: z.object({
      sha: z.string(),
      url: z.string(),
    }),
    parents: z.array(
      z.object({
        sha: z.string(),
        url: z.string(),
        html_url: z.string(),
      })
    ),
  }),
});

// Type exports
export type FileOperation = z.infer<typeof FileOperationSchema>;
export type GitHubCreateUpdateFileResponse = z.infer<typeof GitHubCreateUpdateFileResponseSchema>;

// Function implementations
export async function getFileContents(
  owner: string,
  repo: string,
  path: string,
  branch?: string,
  token?: string
) {
  let url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;
  if (branch) {
    url += `?ref=${branch}`;
  }

  const response = await githubRequest(url, { token });
  const data = GitHubContentSchema.parse(response);

  // If it's a file, decode the content
  if (!Array.isArray(data) && data.content) {
    // Replace newlines and spaces that GitHub adds to base64
    const cleanContent = data.content.replace(/\n/g, '');
    data.content = atob(cleanContent);
  }

  return data;
}

export async function createOrUpdateFile(
  owner: string,
  repo: string,
  path: string,
  content: string,
  message: string,
  branch: string,
  sha?: string,
  authOptions?: { token?: string }
) {
  const encodedContent = btoa(content);

  let currentSha = sha;
  if (!currentSha) {
    try {
      const existingFile = await getFileContents(owner, repo, path, branch, authOptions?.token);
      if (!Array.isArray(existingFile)) {
        currentSha = existingFile.sha;
      }
    } catch (error) {
      console.error("Note: File does not exist in branch, will create new file");
    }
  }

  const url = `https://api.github.com/repos/${owner}/${repo}/contents/${path}`;
  const body = {
    message,
    content: encodedContent,
    branch,
    ...(currentSha ? { sha: currentSha } : {}),
  };

  const response = await githubRequest(url, {
    method: "PUT",
    body,
    token: authOptions?.token
  });

  return GitHubCreateUpdateFileResponseSchema.parse(response);
}

async function createTree(
  owner: string,
  repo: string,
  files: FileOperation[],
  baseTree?: string,
  authOptions?: { token?: string }
) {
  const tree = files.map((file) => ({
    path: file.path,
    mode: "100644" as const,
    type: "blob" as const,
    content: file.content,
  }));

  const response = await githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/git/trees`,
    {
      method: "POST",
      body: {
        tree,
        base_tree: baseTree,
      },
      token: authOptions?.token
    }
  );

  return GitHubTreeSchema.parse(response);
}

async function createCommit(
  owner: string,
  repo: string,
  message: string,
  tree: string,
  parents: string[],
  authOptions?: { token?: string }
) {
  const response = await githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/git/commits`,
    {
      method: "POST",
      body: {
        message,
        tree,
        parents,
      },
      token: authOptions?.token
    }
  );

  return GitHubCommitSchema.parse(response);
}

async function updateReference(
  owner: string,
  repo: string,
  ref: string,
  sha: string,
  authOptions?: { token?: string }
) {
  const response = await githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/git/refs/${ref}`,
    {
      method: "PATCH",
      body: {
        sha,
        force: true,
      },
      token: authOptions?.token
    }
  );

  return GitHubReferenceSchema.parse(response);
}

export async function pushFiles(
  owner: string,
  repo: string,
  branch: string,
  files: FileOperation[],
  message: string,
  authOptions?: { token?: string }
) {
  const refResponse = await githubRequest(
    `https://api.github.com/repos/${owner}/${repo}/git/refs/heads/${branch}`,
    { token: authOptions?.token }
  );

  const ref = GitHubReferenceSchema.parse(refResponse);
  const commitSha = ref.object.sha;

  const tree = await createTree(owner, repo, files, commitSha, authOptions);
  const commit = await createCommit(owner, repo, message, tree.sha, [commitSha], authOptions);
  return await updateReference(owner, repo, `heads/${branch}`, commit.sha, authOptions);
}
