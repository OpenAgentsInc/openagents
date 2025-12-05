#!/usr/bin/env bun
import { bundleFiles, parseArgs } from "./parser.js";
import { renderPromptPreview, resolveBasePrompt, type PromptPreviewOptions } from "./prompt-preview.js";
import { exportSessionToHtml } from "./session-export.js";

const printHelp = () => {
  console.log(`
Prompt utilities

Usage:
  bun src/cli/prompt-cli.ts --print [options]        # Show composed prompt/context/tools
  bun src/cli/prompt-cli.ts --export session.jsonl   # Export session JSONL to HTML

Common options:
  --provider <name>     Provider to display in preview
  --model <id>          Model to display in preview
  --thinking <level>    Thinking level (off|minimal|low|medium|high)
  --tools a,b,c         Comma-separated tool names
  --system-prompt TEXT  Override base system prompt or point to a file
  -p, --print           Print prompt preview and exit
  --export <path>       Export a session JSONL file to HTML
`);
};

const args = parseArgs(process.argv.slice(2));

if (args.export) {
  const outPath = exportSessionToHtml(args.export);
  console.log(`Exported HTML transcript to ${outPath}`);
  process.exit(0);
}

if (args.print) {
  const { prompt: basePrompt } = resolveBasePrompt(args.systemPrompt);
  const files = args.files.length ? bundleFiles(args.files) : [];

  const previewOptions: PromptPreviewOptions = {
    basePrompt,
    cwd: process.cwd(),
    mode: args.mode,
    messages: args.messages,
    files,
  };

  if (args.provider) previewOptions.provider = args.provider;
  if (args.model) previewOptions.model = args.model;
  if (args.thinking) previewOptions.thinking = args.thinking;
  if (args.tools) previewOptions.tools = args.tools;

  const preview = renderPromptPreview(previewOptions);
  console.log(preview);
  process.exit(0);
}

printHelp();
