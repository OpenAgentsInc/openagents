/**
 * Tools the session declares to the model.
 *
 * Delegation started as a `/delegate` line the reader typed. That was the wrong
 * shape: it made the reader the planner, it had to be remembered, and the model
 * — asked to split work across agents — would answer that it could not, because
 * as far as it knew that was true. A tool fixes both halves. The model asks for
 * a fan-out mid-sentence when the work is parallel, and the reader can say
 * "check these three things at once" in prose.
 *
 * The tool runtime is the client's. The inference proxy forwards the tool
 * declarations and returns the calls the model asks for; nothing runs
 * server-side. So this module is the whole contract: a JSON schema the model
 * reads, and a function that runs on this machine.
 */

import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

import type { CoderDelegation } from "./coder-session.js";
import type { CoderTaskRegistry } from "./coder-tasks.js";
import {
  DEFAULT_TIMEOUT_MS,
  MAXIMUM_TIMEOUT_MS,
  refusalFor as shellRefusalFor,
  renderShell,
  runShell,
} from "./coder-shell.js";
import { catalogEntry, renderSkill, type CoderSkill } from "./coder-skills.js";
import { describePrompt, MAX_DELEGATE_COUNT } from "./coder-delegate.js";
import type { DelegationOutcome } from "./coder-delegate.js";

/** One tool the model may call. */
export interface CoderTool {
  readonly name: string;
  /** What the model reads to decide whether to call it. */
  readonly description: string;
  /** JSON Schema for the arguments. */
  readonly parameters: Record<string, unknown>;
  /**
   * Run the call and return what the model should see.
   *
   * A tool reports a refusal as text rather than by throwing: the model can
   * act on "that needs a prompt" and cannot act on a turn that died.
   */
  run(args: Record<string, unknown>, signal: AbortSignal): Promise<string>;
}

/** How much of a child's answer the model is shown, per child. */
const CHILD_RESULT_LIMIT = 2_000;

/**
 * The delegate tool: run one prompt on many child coding agents at once.
 *
 * Awaited rather than launched and forgotten. A model that is told "three
 * children are running" has nothing to say next and will either invent their
 * findings or ask the reader to wait, so the call returns when the children
 * have answered and the answers are the tool's output. The fleet block keeps
 * moving while that happens, because children report through the registry and
 * the renderer reads the registry, not this call.
 */
export function delegateTool(delegation: CoderDelegation): CoderTool {
  const models = delegation.fleetFor === undefined ? [] : (delegation.models ?? []);
  return {
    name: "delegate",
    description:
      "Run one prompt on independent child coding agents in parallel, in this repository, and " +
      "return what each one found or did. Use it whenever work splits into parts that do not " +
      "depend on each other: several files to change the same way, several hypotheses to check, " +
      "several tests to run down. Each child is a full coding agent with its own file and shell " +
      "tools, it starts with no context from this conversation, and it cannot ask questions, so " +
      "the prompt has to be self-contained. Children run on this session's budget. Every child " +
      "runs the same prompt, and each is told separately which number it is, so write the prompt " +
      'for whichever child reads it: say "read the file at your own number" rather than naming ' +
      'one child ("you are child 1"), which gives every child the same work and wastes the ' +
      "fan-out. Prefer one call with a count over several calls. At most " +
      `${String(MAX_DELEGATE_COUNT)} children.` +
      (models.length === 0
        ? ""
        : ` Children run on ${delegation.label} unless \`model\` names another: ${models.join(", ")}.`),
    parameters: {
      type: "object",
      properties: {
        prompt: {
          type: "string",
          description:
            "The complete, self-contained instruction every child performs. Name the files, the " +
            "command, and what to report back.",
        },
        count: {
          type: "integer",
          minimum: 1,
          maximum: MAX_DELEGATE_COUNT,
          description: "How many children run this prompt. Defaults to 1.",
        },
        description: {
          type: "string",
          description: "Three to five words naming the task, shown in the fleet.",
        },
        ...(models.length === 0
          ? {}
          : {
              model: {
                type: "string",
                enum: [...models],
                description:
                  "Which model the children run on. Defaults to " +
                  `${delegation.label}. Straightforward engineering suits a fast model; ` +
                  "work whose shape is the question suits a stronger one.",
              },
            }),
      },
      required: ["prompt"],
      additionalProperties: false,
    },
    run: async (args, signal) => {
      const prompt = typeof args["prompt"] === "string" ? args["prompt"].trim() : "";
      if (prompt.length === 0) {
        return "No children were started: `prompt` is required and must say what the child does.";
      }

      // The lane is chosen per call. A session that had to be restarted to
      // change models is a session that will not change them.
      const named = typeof args["model"] === "string" ? args["model"].trim() : "";
      const lane =
        named.length === 0
          ? { fleet: delegation.fleet, label: delegation.label }
          : delegation.fleetFor?.(named);

      if (lane === undefined) {
        return models.length === 0
          ? `This session runs children one way only, on ${delegation.label}, so \`model\` cannot be chosen here.`
          : `There is no \`${named}\` lane. This session can run children on: ${models.join(", ")}.`;
      }

      const requested = typeof args["count"] === "number" ? Math.trunc(args["count"]) : 1;
      const count = Math.min(MAX_DELEGATE_COUNT, Math.max(1, requested));
      const described = typeof args["description"] === "string" ? args["description"].trim() : "";
      const description = described.length > 0 ? described : describePrompt(prompt);

      // An interrupted turn must not leave children spending. The reader's
      // escape key is the only stop signal a running fan-out has.
      const onAbort = () => delegation.registry.stopAll();
      signal.addEventListener("abort", onAbort, { once: true });

      try {
        const outcomes = await Promise.all(
          Array.from({ length: count }, (_unused, index) =>
            lane.fleet.submit({
              description,
              prompt: identify(prompt, index + 1, count),
              background: true,
            }),
          ),
        );
        return report(outcomes, delegation.registry, lane.label);
      } finally {
        signal.removeEventListener("abort", onAbort);
      }
    },
  };
}

/**
 * Tell a child which of the fan-out it is.
 *
 * Every child gets the same prompt, so a prompt that says "your own file"
 * otherwise has no way to mean anything and the whole fleet writes the same
 * one. A single child is told nothing, because there is nothing to
 * distinguish.
 */
function identify(prompt: string, index: number, count: number): string {
  if (count === 1) return prompt;
  return `You are child ${String(index)} of ${String(count)}.\n\n${prompt}`;
}

/** Every child's outcome, in the order they were launched. */
// The registry is the session's and is shared by every lane, so the children of
// two models still render as one fleet. The label is the lane's own, because
// that is what answered.
function report(
  outcomes: ReadonlyArray<DelegationOutcome>,
  registry: CoderTaskRegistry,
  label: string,
): string {
  const lines: string[] = [];
  let completed = 0;

  for (const outcome of outcomes) {
    if (outcome.status === "refused") {
      lines.push(`refused (${outcome.code}): ${outcome.reason}`);
      continue;
    }

    const task = registry.get(outcome.taskId);
    const label = `${outcome.taskId}${task === undefined ? "" : ` ${task.description}`}`;
    if (outcome.status === "completed") {
      completed += 1;
      const result = outcome.result.trim();
      lines.push(
        `${label} completed:\n${result.length === 0 ? "(no output)" : clip(result, CHILD_RESULT_LIMIT)}`,
      );
    } else if (outcome.status === "failed") {
      lines.push(`${label} failed: ${outcome.error}`);
    } else {
      lines.push(`${label} stopped before finishing.`);
    }
    registry.markRead(outcome.taskId);
  }

  const header =
    `${String(completed)} of ${String(outcomes.length)} ` +
    `${outcomes.length === 1 ? "child" : "children"} completed on ${label}.`;
  return [header, "", ...lines].join("\n");
}

function clip(text: string, limit: number): string {
  return text.length <= limit ? text : `${text.slice(0, limit)}\n…[truncated]`;
}

/**
 * The skill tool: read one of this session's skills.
 *
 * The catalog is the description rather than a system prompt, so the same tool
 * carries it on the local lane, where the client writes the prompt, and on the
 * thread lane, where the server does. Bodies are not sent until asked for: the
 * catalog is a line each, and the skills on one machine already run to tens of
 * kilobytes.
 *
 * Returned as text rather than thrown for a name that does not match, and the
 * text lists what does: a model that misremembers a name can correct itself on
 * the next call, and cannot correct a turn that died.
 */
export function skillTool(skills: ReadonlyArray<CoderSkill>): CoderTool {
  const catalog = skills.map((skill) => catalogEntry(skill)).join("\n");
  return {
    name: "skill",
    description:
      "Read one of this repository's skills: a written procedure for a kind of work, with the " +
      "conventions, commands, and rules it needs. Call it before doing work a skill covers, and " +
      "follow what it says over your own habits. Skills available:\n" +
      catalog,
    parameters: {
      type: "object",
      properties: {
        name: {
          type: "string",
          enum: skills.map((skill) => skill.name),
          description: "The skill to read.",
        },
      },
      required: ["name"],
      additionalProperties: false,
    },
    run: (args) => {
      const name = typeof args["name"] === "string" ? args["name"].trim() : "";
      const skill = skills.find((candidate) => candidate.name === name);
      if (skill === undefined) {
        return Promise.resolve(
          `There is no \`${name}\` skill. This session has: ${skills
            .map((candidate) => `\`${candidate.name}\``)
            .join(", ")}.`,
        );
      }
      return Promise.resolve(renderSkill(skill));
    },
  };
}

/**
 * How much of one command's output the model is shown.
 *
 * Enough for a help page or a page of issues. A command that produces more than
 * this is one to narrow with a flag or `--json`, not one to read whole.
 */
const CLI_OUTPUT_LIMIT = 16_000;

/** How long a command may run before it is given up on. */
const CLI_TIMEOUT_MS = 120_000;

/**
 * Commands that wait for a terminal, and what to do instead.
 *
 * The tool has no terminal to give them: a session that started one would hang
 * with nothing on screen to say why. Refused by name, with the alternative,
 * rather than left to time out.
 */
const NEEDS_A_TERMINAL: ReadonlyArray<readonly [ReadonlyArray<string>, string]> = [
  [["coder"], "You are already a coder session. Use the `delegate` tool to run work in parallel."],
  [
    ["auth", "login"],
    "Run `auth login --headless` instead: it returns a URL and a code for the person to " +
      "approve, then `auth login --resume` completes it.",
  ],
  [["computer", "up"], "It serves until stopped, so it belongs in a terminal of its own."],
];

/**
 * The compiled entry to run, in whichever layout this module is loaded from.
 *
 * Not `PATH`, and not `process.argv[1]`: what answers has to be the build this
 * session is part of, or `--help` describes a different CLI than the one
 * running. `main.js` sits beside this module in a build and one directory over
 * in a source tree, and a test runs from the second.
 */
const cliEntry = (): string | undefined => {
  for (const candidate of ["main.js", "../dist/main.js"]) {
    const path = fileURLToPath(new URL(candidate, import.meta.url));
    if (existsSync(path)) return path;
  }
  return undefined;
};

const refusalFor = (args: ReadonlyArray<string>): string | undefined => {
  const words = args.filter((word) => !word.startsWith("-"));
  for (const [prefix, alternative] of NEEDS_A_TERMINAL) {
    const matches = prefix.every((word, at) => words[at] === word);
    // `auth login --headless` does not wait, so it is not the refused command.
    if (matches && !(prefix[0] === "auth" && args.includes("--headless"))) {
      return `\`openagents ${prefix.join(" ")}\` needs a terminal, which this tool has not got. ${alternative}`;
    }
  }
  return undefined;
};

/**
 * The command tree, read out of the CLI's own completion script.
 *
 * A session that did not know `issue list` existed spent two turns finding out:
 * `issue --help`, then `issue list --help`, then the command it wanted. Two
 * round-trips of a model's time to learn two words.
 *
 * So the tree goes in the tool description. It is not written down here -- it
 * is asked for once, from the binary that is running, through the completion
 * script that already enumerates every command and subcommand. One process at
 * session start, and it cannot describe a CLI other than this one.
 *
 * Flags are still left to `--help`. There are hundreds and they change; the
 * command names are few and are what the round-trips were being spent on.
 */
const commandTree = (entry: string): string | undefined => {
  const shown = spawnSync(process.execPath, [entry, "--completions", "zsh"], {
    encoding: "utf8",
    timeout: 10_000,
  });
  if (shown.status !== 0 || typeof shown.stdout !== "string") return undefined;

  const lines: string[] = [];
  // Each `commands=( … )` block belongs to the function it sits in, and that
  // function is named for the command whose subcommands it lists.
  for (const match of shown.stdout.matchAll(/_openagents(_[a-z_]*)?\(\)\s*\{([\s\S]*?)\n\}/g)) {
    const owner = (match[1] ?? "").replaceAll("_", " ").trim();
    const block = /commands=\(([\s\S]*?)\n\s*\)/.exec(match[2] ?? "");
    if (block === null) continue;
    const names = [...(block[1] ?? "").matchAll(/'([a-z][a-z0-9-]*):/g)].map((found) => found[1]);
    if (names.length === 0) continue;
    lines.push(owner.length === 0 ? `openagents ${names.join(" | ")}` : `  ${owner} ${names.join(" | ")}`);
  }
  return lines.length === 0 ? undefined : lines.join("\n");
};

/**
 * The openagents tool: run the CLI this session is part of.
 *
 * The same binary that is running answers, found through `process.argv` rather
 * than through `PATH`, so what the model reads is this machine's build and not
 * whichever copy happens to be installed. That also means `--help` is a live
 * answer: the model discovers the command tree by asking it, and no list here
 * can go stale.
 */
export function openagentsTool(): CoderTool {
  const entry = cliEntry();
  const tree = entry === undefined ? undefined : commandTree(entry);
  return {
    name: "openagents",
    description:
      "Run the OpenAgents CLI: issues, projects, repositories, the forum, authentication, and " +
      "any API route through `api`. Pass the arguments after `openagents` as a list, without " +
      "`openagents` itself.\n\n" +
      (tree === undefined ? "" : `Commands:\n${tree}\n\n`) +
      "Run `<command> --help` when you need a flag you do not know; the commands above are the " +
      "whole set, so you do not need to go looking for them.\n\n" +
      "Read the plain output. It is what a person reads and it is small: a list of three issues " +
      "is 442 bytes plain and 20,000 as JSON, because the JSON carries every issue's whole body. " +
      "Add `--json` only when you need one field out of one record, and prefer a narrower " +
      "command over a wider one you then have to read past.\n\n" +
      "Reads are free; a write is visible to other people at once, so say what you are about to " +
      "write before the first one. Read the `openagents-cli` skill for the auth model and what " +
      "works with no credential.",
    parameters: {
      type: "object",
      properties: {
        args: {
          type: "array",
          items: { type: "string" },
          description:
            'The arguments after `openagents`, one per element. For example ["issue", "view", ' +
            '"21", "-R", "OpenAgentsInc/openagents", "--json"].',
        },
      },
      required: ["args"],
      additionalProperties: false,
    },
    run: async (rawArgs, signal) => {
      const args = Array.isArray(rawArgs["args"])
        ? rawArgs["args"].filter((word): word is string => typeof word === "string")
        : [];
      if (args.length === 0) {
        return "No command was run: `args` is required, such as [\"--help\"].";
      }

      const refusal = refusalFor(args);
      if (refusal !== undefined) return refusal;

      const { spawn } = await import("node:child_process");
      if (entry === undefined) return "This session cannot find the CLI it is running from.";

      return await new Promise<string>((resolve) => {
        const child = spawn(process.execPath, [entry, ...args], {
          // No terminal, so a command that would prompt gets end-of-file rather
          // than a wait nobody can see.
          stdio: ["ignore", "pipe", "pipe"],
        });
        let output = "";
        let done = false;

        const finish = (text: string) => {
          if (done) return;
          done = true;
          clearTimeout(timer);
          signal.removeEventListener("abort", onAbort);
          resolve(text);
        };

        const timer = setTimeout(() => {
          child.kill("SIGKILL");
          finish(`The command did not finish within ${String(CLI_TIMEOUT_MS / 1000)}s.\n\n${output}`);
        }, CLI_TIMEOUT_MS);

        const onAbort = () => {
          child.kill("SIGKILL");
          finish("The command was interrupted.");
        };
        signal.addEventListener("abort", onAbort, { once: true });

        const collect = (chunk: Buffer) => {
          if (output.length < CLI_OUTPUT_LIMIT) output += chunk.toString("utf8");
        };
        child.stdout.on("data", collect);
        child.stderr.on("data", collect);

        child.on("error", (cause) => {
          finish(`The command could not be started: ${cause.message}`);
        });

        child.on("close", (code) => {
          // The advice has to fit what was run. This used to say "or use
          // --json" on every truncation, which for a list is the thing that
          // caused it: three issues are 442 bytes plain and 20,000 as JSON,
          // because the JSON carries every body.
          const advice = args.includes("--json")
            ? "drop --json and read the plain output, or ask for one record"
            : "narrow it with a flag such as --limit, --label, or --state";
          const bounded =
            output.length > CLI_OUTPUT_LIMIT
              ? `${output.slice(0, CLI_OUTPUT_LIMIT)}\n\n[The output was cut off here. Run it again and ${advice}; what you have above is incomplete and must not be summarized as if it were the whole answer.]`
              : output;
          const body = bounded.trim();
          // The exit code is reported on failure because it is what the CLI
          // says about itself, and an empty failure reads as an empty success.
          finish(
            code === 0
              ? body.length === 0
                ? "The command succeeded and printed nothing."
                : body
              : `The command exited with code ${String(code ?? -1)}.\n\n${body}`,
          );
        });
      });
    },
  };
}

/**
 * The shell tool: run a command on this machine.
 *
 * The session had no way to look at anything. Asked for its working directory
 * it started a child coding agent on a hosted model to run `pwd` -- minutes and
 * real money for one line, reported second-hand. Reading a file, listing a
 * directory, running the tests: all of it went through a whole agent or through
 * the reader.
 *
 * So `delegate` goes back to what it is for. A fan-out is worth an agent each;
 * `pwd` is worth a process.
 */
export function shellTool(cwd: string): CoderTool {
  return {
    name: "shell",
    description:
      `Run a shell command on this machine. The working directory is ${cwd}, so paths are ` +
      "relative to it and you do not need to ask where you are. Returns what the command " +
      "printed. Use it for anything you would type at a terminal: reading files, listing " +
      "directories, searching, git, running builds and tests. For the `openagents` CLI use the " +
      "`openagents` tool instead — it carries the list of commands, so running it through here " +
      "costs a turn finding out what exists. Prefer it over `delegate` for " +
      "single commands -- a child agent is for work worth a whole agent, not for one line of " +
      "output. Both output streams come back together with the exit code. There is no terminal, " +
      "so a command that would prompt gets end-of-file instead of waiting; pass a flag that " +
      "answers the prompt. A few commands that cannot be undone are refused, such as erasing a " +
      "root or a home directory, reformatting a disk, or halting the machine.",
    parameters: {
      type: "object",
      properties: {
        command: {
          type: "string",
          description: "The command line, as you would type it. Runs through `/bin/sh -c`.",
        },
        timeout_seconds: {
          type: "integer",
          minimum: 1,
          maximum: MAXIMUM_TIMEOUT_MS / 1000,
          description: `How long to wait. Defaults to ${String(DEFAULT_TIMEOUT_MS / 1000)}; raise it for a build or a test run.`,
        },
      },
      required: ["command"],
      additionalProperties: false,
    },
    run: async (args, signal) => {
      const command = typeof args["command"] === "string" ? args["command"].trim() : "";
      if (command.length === 0) {
        return "No command was run: `command` is required and must say what to run.";
      }

      const refusal = shellRefusalFor(command);
      if (refusal !== undefined) return refusal;

      const asked = typeof args["timeout_seconds"] === "number" ? args["timeout_seconds"] : undefined;
      const timeoutMs = Math.min(
        asked === undefined ? DEFAULT_TIMEOUT_MS : Math.max(1, Math.trunc(asked)) * 1000,
        MAXIMUM_TIMEOUT_MS,
      );

      const result = await runShell(command, { cwd, timeoutMs, signal });
      return renderShell(result, timeoutMs);
    },
  };
}
