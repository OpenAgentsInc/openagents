import { readFileSync } from "fs";
import { extname, resolve } from "path";

export type Mode = "text" | "json" | "rpc";

export interface ParsedArgs {
  mode: Mode;
  provider?: string;
  model?: string;
  apiKey?: string;
  systemPrompt?: string;
  thinking?: "off" | "minimal" | "low" | "medium" | "high";
  tools?: string[];
  print?: boolean;
  export?: string;
  noSession?: boolean;
  session?: string;
  messages: string[];
  files: string[];
}

const IMAGE_MIME_TYPES: Record<string, string> = {
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".gif": "image/gif",
  ".webp": "image/webp",
};

export const parseArgs = (argv: string[]): ParsedArgs => {
  const out: ParsedArgs = { mode: "text", messages: [], files: [] };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    switch (arg) {
      case "--mode":
        out.mode = (argv[++i] as Mode) || "text";
        break;
      case "--provider":
        out.provider = argv[++i];
        break;
      case "--model":
        out.model = argv[++i];
        break;
      case "--api-key":
        out.apiKey = argv[++i];
        break;
      case "--system-prompt":
        out.systemPrompt = argv[++i];
        break;
      case "--thinking":
        out.thinking = argv[++i] as any;
        break;
      case "--tools":
        out.tools = argv[++i]?.split(",").map((s) => s.trim());
        break;
      case "--print":
      case "-p":
        out.print = true;
        break;
      case "--export":
        out.export = argv[++i];
        break;
      case "--no-session":
        out.noSession = true;
        break;
      case "--session":
        out.session = argv[++i];
        break;
      default:
        if (arg.startsWith("@")) {
          out.files.push(arg.slice(1));
        } else if (!arg.startsWith("-")) {
          out.messages.push(arg);
        }
        break;
    }
  }

  return out;
};

export interface BundledFile {
  path: string;
  content: string;
  mimeType?: string;
  isImage: boolean;
}

export const bundleFiles = (paths: string[]): BundledFile[] => {
  return paths.flatMap((p) => {
    const abs = resolve(p);
    const mime = IMAGE_MIME_TYPES[extname(abs).toLowerCase()];
    const isImg = Boolean(mime);
    const data = readFileSync(abs, isImg ? "base64" : "utf8");
    return [
      {
        path: abs,
        content: data,
        mimeType: mime,
        isImage: isImg,
      },
    ];
  });
};
