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

import type { CoderDelegation } from "./coder-session.js";
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
      `${String(MAX_DELEGATE_COUNT)} children.`,
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
      },
      required: ["prompt"],
      additionalProperties: false,
    },
    run: async (args, signal) => {
      const prompt = typeof args["prompt"] === "string" ? args["prompt"].trim() : "";
      if (prompt.length === 0) {
        return "No children were started: `prompt` is required and must say what the child does.";
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
            delegation.fleet.submit({
              description,
              prompt: identify(prompt, index + 1, count),
              background: true,
            }),
          ),
        );
        return report(outcomes, delegation);
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
function report(outcomes: ReadonlyArray<DelegationOutcome>, delegation: CoderDelegation): string {
  const lines: string[] = [];
  let completed = 0;

  for (const outcome of outcomes) {
    if (outcome.status === "refused") {
      lines.push(`refused (${outcome.code}): ${outcome.reason}`);
      continue;
    }

    const task = delegation.registry.get(outcome.taskId);
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
    delegation.registry.markRead(outcome.taskId);
  }

  const header =
    `${String(completed)} of ${String(outcomes.length)} ` +
    `${outcomes.length === 1 ? "child" : "children"} completed on ${delegation.label}.`;
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
