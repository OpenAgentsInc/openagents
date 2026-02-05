import http from "node:http";
import path from "node:path";
import crypto from "node:crypto";
import { spawn } from "node:child_process";
import { mkdir, readFile, stat, writeFile } from "node:fs/promises";

const PORT = Number(process.env.PORT ?? 8787);
const ROOT = process.env.LITECLAW_LOCAL_ROOT;
const TOKEN = process.env.LITECLAW_TUNNEL_TOKEN;
const MAX_BYTES = Number(process.env.LITECLAW_LOCAL_MAX_BYTES ?? 200000);
const MAX_BASH_BYTES = Number(
  process.env.LITECLAW_LOCAL_BASH_MAX_BYTES ?? MAX_BYTES
);
const MAX_BODY_BYTES = Number(
  process.env.LITECLAW_LOCAL_MAX_BODY_BYTES ?? 1000000
);
const ALLOWED_TOOLS = parseAllowlist(
  process.env.LITECLAW_LOCAL_ALLOWED_TOOLS ??
    "workspace.read,workspace.write,workspace.edit"
);

if (!ROOT) {
  console.error("LITECLAW_LOCAL_ROOT is required.");
  process.exit(1);
}

if (!TOKEN) {
  console.error("LITECLAW_TUNNEL_TOKEN is required.");
  process.exit(1);
}

const ROOT_PATH = path.resolve(ROOT);

function parseAllowlist(value) {
  return value
    .split(",")
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function isToolAllowed(toolName) {
  return ALLOWED_TOOLS.includes("*") || ALLOWED_TOOLS.includes(toolName);
}

function normalizeRelativePath(inputPath) {
  const trimmed = inputPath.trim().replace(/\\/g, "/");
  const cleaned = trimmed.replace(/^\/+/, "");
  if (!cleaned || cleaned.includes("..")) {
    throw new Error("Invalid path.");
  }
  return cleaned;
}

function resolvePath(relativePath) {
  const cleaned = normalizeRelativePath(relativePath);
  const resolved = path.resolve(ROOT_PATH, cleaned);
  if (!resolved.startsWith(`${ROOT_PATH}${path.sep}`)) {
    throw new Error("Path escapes root.");
  }
  return { cleaned, resolved };
}

function hashText(text) {
  return crypto.createHash("sha256").update(text).digest("hex");
}

function hashJson(value) {
  return hashText(JSON.stringify(value));
}

function signReceipt(receipt) {
  return crypto
    .createHmac("sha256", TOKEN)
    .update(JSON.stringify(receipt))
    .digest("hex");
}

function sendJson(res, status, payload) {
  res.writeHead(status, { "content-type": "application/json" });
  res.end(JSON.stringify(payload));
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on("data", (chunk) => {
      size += chunk.length;
      if (size > MAX_BODY_BYTES) {
        reject(new Error("Request body too large."));
        req.destroy();
        return;
      }
      chunks.push(chunk);
    });
    req.on("end", () => {
      const buffer = Buffer.concat(chunks);
      resolve(buffer.toString("utf8"));
    });
    req.on("error", reject);
  });
}

async function readWorkspaceFile(args) {
  if (!args || typeof args.path !== "string") {
    throw new Error("Missing path.");
  }
  const { cleaned, resolved } = resolvePath(args.path);
  let stats;
  try {
    stats = await stat(resolved);
  } catch (error) {
    const notFound = new Error("File not found.");
    notFound.statusCode = 404;
    throw notFound;
  }
  if (!stats.isFile()) {
    throw new Error("Path is not a file.");
  }
  if (stats.size > MAX_BYTES) {
    throw new Error("File exceeds max size.");
  }
  const content = await readFile(resolved, "utf8");
  return {
    output: {
      executor_kind: "tunnel",
      path: cleaned,
      content,
      bytes: content.length,
      updated_at: stats.mtimeMs
    },
    patchHash: null
  };
}

async function writeWorkspaceFile(args) {
  if (!args || typeof args.path !== "string" || typeof args.content !== "string") {
    throw new Error("Missing path or content.");
  }
  const { cleaned, resolved } = resolvePath(args.path);
  let before = null;
  try {
    before = await readFile(resolved, "utf8");
  } catch {
    before = null;
  }
  await mkdir(path.dirname(resolved), { recursive: true });
  await writeFile(resolved, args.content, "utf8");
  const stats = await stat(resolved);

  const beforeBytes = before?.length ?? 0;
  const afterBytes = args.content.length;
  const beforeHash = before ? hashText(before) : null;
  const afterHash = hashText(args.content);
  const patch = {
    op: "write",
    path: cleaned,
    before_hash: beforeHash,
    after_hash: afterHash,
    before_bytes: beforeBytes,
    after_bytes: afterBytes
  };
  const patchHash = hashJson(patch);

  return {
    output: {
      executor_kind: "tunnel",
      path: cleaned,
      created: before === null,
      before_hash: beforeHash,
      after_hash: afterHash,
      before_bytes: beforeBytes,
      after_bytes: afterBytes,
      updated_at: stats.mtimeMs,
      patch
    },
    patchHash
  };
}

async function editWorkspaceFile(args) {
  if (
    !args ||
    typeof args.path !== "string" ||
    typeof args.find !== "string" ||
    typeof args.replace !== "string"
  ) {
    throw new Error("Missing path, find, or replace.");
  }
  if (args.find.length === 0) {
    throw new Error("Find text must not be empty.");
  }
  const { cleaned, resolved } = resolvePath(args.path);
  let before;
  try {
    before = await readFile(resolved, "utf8");
  } catch (error) {
    const notFound = new Error("File not found.");
    notFound.statusCode = 404;
    throw notFound;
  }

  let after = before;
  let replacements = 0;
  if (args.all) {
    const parts = before.split(args.find);
    if (parts.length === 1) {
      throw new Error("Find text not found.");
    }
    replacements = parts.length - 1;
    after = parts.join(args.replace);
  } else {
    const index = before.indexOf(args.find);
    if (index === -1) {
      throw new Error("Find text not found.");
    }
    replacements = 1;
    after =
      before.slice(0, index) + args.replace + before.slice(index + args.find.length);
  }

  await writeFile(resolved, after, "utf8");
  const stats = await stat(resolved);

  const beforeHash = hashText(before);
  const afterHash = hashText(after);
  const patch = {
    op: "edit",
    path: cleaned,
    find: args.find,
    replace: args.replace,
    all: Boolean(args.all),
    replacements,
    before_hash: beforeHash,
    after_hash: afterHash
  };
  const patchHash = hashJson(patch);

  return {
    output: {
      executor_kind: "tunnel",
      path: cleaned,
      replacements,
      before_hash: beforeHash,
      after_hash: afterHash,
      updated_at: stats.mtimeMs,
      patch
    },
    patchHash
  };
}

async function executeBashCommand(args) {
  if (!args || typeof args.command !== "string") {
    throw new Error("Missing command.");
  }
  const timeoutSeconds =
    typeof args.timeout === "number" && args.timeout > 0
      ? args.timeout
      : null;

  const result = await new Promise((resolve, reject) => {
    const shell = process.platform === "win32" ? "cmd" : "sh";
    const shellArgs = process.platform === "win32" ? ["/c"] : ["-c"];
    const child = spawn(shell, [...shellArgs, args.command], {
      detached: true,
      stdio: ["ignore", "pipe", "pipe"]
    });

    let stdout = "";
    let stderr = "";
    let timedOut = false;
    const timeoutHandle =
      timeoutSeconds !== null
        ? setTimeout(() => {
            timedOut = true;
            try {
              process.kill(-child.pid, "SIGKILL");
            } catch {
              try {
                process.kill(child.pid, "SIGKILL");
              } catch {
                // ignore
              }
            }
          }, timeoutSeconds * 1000)
        : null;

    child.stdout?.on("data", (chunk) => {
      stdout += chunk.toString();
    });

    child.stderr?.on("data", (chunk) => {
      stderr += chunk.toString();
    });

    child.on("close", (code) => {
      if (timeoutHandle) clearTimeout(timeoutHandle);
      if (timedOut) {
        reject(new Error("Command timed out."));
        return;
      }
      resolve({ stdout, stderr, code: code ?? 0 });
    });

    child.on("error", (error) => {
      if (timeoutHandle) clearTimeout(timeoutHandle);
      reject(error);
    });
  });

  let combined = "";
  if (result.stdout) combined += result.stdout;
  if (result.stderr) {
    if (combined) combined += "\n";
    combined += result.stderr;
  }

  let outputText = combined || "(no output)";
  let truncated = false;
  let totalBytes = Buffer.byteLength(outputText, "utf8");
  if (totalBytes > MAX_BASH_BYTES) {
    truncated = true;
    outputText = outputText.slice(outputText.length - MAX_BASH_BYTES);
    totalBytes = Buffer.byteLength(outputText, "utf8");
  }

  return {
    output: {
      executor_kind: "tunnel",
      command: args.command,
      stdout: result.stdout,
      stderr: result.stderr,
      exit_code: result.code,
      output: outputText,
      truncated,
      bytes: totalBytes
    },
    patchHash: null
  };
}

async function handleInvoke(body) {
  if (!body || typeof body.tool_name !== "string") {
    throw new Error("Missing tool_name.");
  }
  const toolName = body.tool_name;
  if (!isToolAllowed(toolName)) {
    const error = new Error("Tool not allowed.");
    error.statusCode = 403;
    throw error;
  }

  const startedAt = Date.now();
  let result;
  switch (toolName) {
    case "workspace.read":
      result = await readWorkspaceFile(body.args ?? {});
      break;
    case "workspace.write":
      result = await writeWorkspaceFile(body.args ?? {});
      break;
    case "workspace.edit":
      result = await editWorkspaceFile(body.args ?? {});
      break;
    case "bash":
      result = await executeBashCommand(body.args ?? {});
      break;
    default: {
      const error = new Error("Unsupported tool.");
      error.statusCode = 404;
      throw error;
    }
  }

  const completedAt = Date.now();
  const argsHash = hashText(JSON.stringify(body.args ?? {}));
  const outputHash = hashJson(result.output);
  const receipt = {
    tool_name: toolName,
    args_hash: argsHash,
    output_hash: outputHash,
    patch_hash: result.patchHash ?? null,
    executor_kind: "tunnel",
    started_at: startedAt,
    completed_at: completedAt,
    duration_ms: completedAt - startedAt
  };

  return {
    ok: true,
    output: result.output,
    receipt,
    signature: signReceipt(receipt)
  };
}

const server = http.createServer(async (req, res) => {
  try {
    if (req.method === "GET" && req.url === "/health") {
      return sendJson(res, 200, { ok: true });
    }

    if (req.method !== "POST" || req.url !== "/tools/invoke") {
      return sendJson(res, 404, { ok: false, error: "Not found." });
    }

    const auth = req.headers.authorization ?? "";
    if (!auth.startsWith("Bearer ") || auth.slice(7) !== TOKEN) {
      return sendJson(res, 401, { ok: false, error: "Unauthorized." });
    }

    const rawBody = await readBody(req);
    const payload = JSON.parse(rawBody);
    const response = await handleInvoke(payload);
    return sendJson(res, 200, response);
  } catch (error) {
    const statusCode = error?.statusCode ?? 400;
    const message = error instanceof Error ? error.message : "Unknown error";
    return sendJson(res, statusCode, { ok: false, error: message });
  }
});

server.listen(PORT, () => {
  console.log(
    `[liteclaw-local-agent] listening on ${PORT}, root=${ROOT_PATH}`
  );
});
