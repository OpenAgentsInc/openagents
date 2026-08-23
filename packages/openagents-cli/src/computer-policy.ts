import { homedir } from "node:os";
import { isAbsolute, normalize, relative, resolve, sep } from "node:path";

export type Tier = "probe" | "curated" | "shell";

export const tierRank: Record<Tier, number> = {
  probe: 0,
  curated: 1,
  shell: 2,
};

export const tierAllows = (ceiling: Tier, requested: Tier): boolean =>
  tierRank[requested] <= tierRank[ceiling];

export interface PolicyConfig {
  readonly tier: Tier;
  readonly roots: ReadonlyArray<string>;
  readonly preApproved: ReadonlyArray<string>;
}

export interface CommandRequest {
  readonly argv: ReadonlyArray<string>;
  readonly cwd: string;
}

export type RefusalReason =
  | "empty_command"
  | "tier_insufficient"
  | "not_allowlisted"
  | "root_not_declared"
  | "denied_command"
  | "denied_argument"
  | "shell_metacharacter"
  | "confirmation_required";

export type Decision =
  | { readonly _tag: "Allowed"; readonly needsConfirmation: boolean }
  | {
      readonly _tag: "Refused";
      readonly reason: RefusalReason;
      readonly detail: string;
    };

const deniedCommands = new Set([
  "sudo",
  "doas",
  "su",
  "chmod",
  "chown",
  "mkfs",
  "dd",
  "shutdown",
  "reboot",
  "halt",
  "passwd",
  "ssh-keygen",
  "ssh-add",
  "keychain",
  "security",
  "gpg",
  "crontab",
  "systemctl",
  "launchctl",
  "nc",
  "ncat",
  "telnet",
]);

const deniedPathFragments = [
  ".ssh",
  ".aws",
  ".gnupg",
  ".kube",
  ".netrc",
  ".npmrc",
  ".pypirc",
  ".git-credentials",
  "id_rsa",
  "id_ed25519",
  ".env",
  "credentials.json",
  "Keychains",
] as const;

const shellMetacharacters = (platform: NodeJS.Platform): RegExp =>
  platform === "win32" ? /[;&|`$><\n\r]/u : /[;&|`$><\n\r\\]/u;

export const curatedAllowlist: Readonly<Record<string, ReadonlyArray<string>>> = {
  git: ["status", "log", "diff", "branch", "remote", "show", "rev-parse", "ls-files", "--version"],
  uname: [],
  date: [],
  echo: [],
  whoami: [],
  df: [],
  du: [],
  ps: [],
  uptime: [],
  file: [],
  stat: [],
  ls: [],
  cat: [],
  head: [],
  tail: [],
  wc: [],
  pwd: [],
  which: [],
  rg: [],
  grep: [],
  node: ["--version"],
  npm: ["--version", "ls"],
  pnpm: ["--version", "ls"],
  python3: ["--version"],
  cargo: ["--version"],
  go: ["version"],
  docker: ["ps", "images", "version"],
};

const ghReadActions: Readonly<Record<string, ReadonlyArray<string>>> = {
  issue: ["list", "view", "status"],
  pr: ["list", "view", "status", "diff", "checks"],
  release: ["list", "view"],
  run: ["list", "view"],
  workflow: ["list", "view"],
  repo: ["list", "view"],
  gist: ["list", "view"],
  cache: ["list"],
  label: ["list"],
  ruleset: ["list", "view"],
  auth: ["status"],
};

const ghReadTopLevel = new Set(["search", "status", "--version", "version"]);
const ghApiWriteFlags = new Set(["-f", "-F", "--field", "--raw-field", "--input"]);

export const ghReadOnlyAllowed = (args: ReadonlyArray<string>): boolean => {
  const [resource, action] = args;
  if (resource === undefined) return false;
  if (ghReadTopLevel.has(resource)) return args.length === 1;
  if (resource === "api") return false;
  return (
    args.length === 2 &&
    action !== undefined &&
    !args.some((arg) => ghApiWriteFlags.has(arg)) &&
    (ghReadActions[resource] ?? []).includes(action)
  );
};

const commandName = (value: string): string =>
  (value.split(/[\\/]/u).pop() ?? value).toLowerCase().replace(/\.exe$/u, "");

const looksLikePath = (value: string): boolean =>
  isAbsolute(value) ||
  value.startsWith("~") ||
  value.includes("../") ||
  value.includes("..\\") ||
  value.startsWith("./") ||
  value.startsWith(".\\");

export const resolveRoot = (root: string): string =>
  normalize(resolve(root.replace(/^~(?=$|[\\/])/u, homedir())));

export const resolveRoots = (roots: ReadonlyArray<string>): ReadonlyArray<string> => [
  ...new Set(roots.map(resolveRoot)),
];

export const withinRoot = (candidate: string, root: string): boolean => {
  const target = resolve(candidate);
  const base = resolve(root);
  const targetRelative = relative(base, target);
  return (
    targetRelative === "" ||
    (!isAbsolute(targetRelative) &&
      targetRelative !== ".." &&
      !targetRelative.startsWith(`..${sep}`))
  );
};

const refuse = (reason: RefusalReason, detail: string): Decision => ({
  _tag: "Refused",
  reason,
  detail,
});

export const decide = (request: CommandRequest, config: PolicyConfig): Decision => {
  const [argv0, ...rest] = request.argv;
  if (argv0 === undefined || argv0.trim() === "") {
    return refuse("empty_command", "no command was supplied");
  }
  if (request.argv.some((part) => shellMetacharacters(process.platform).test(part))) {
    return refuse("shell_metacharacter", "command arguments cannot contain shell metacharacters");
  }
  const name = commandName(argv0);
  if (deniedCommands.has(name)) {
    return refuse("denied_command", `${name} is denied on this machine`);
  }
  for (const argument of request.argv) {
    const fragment = deniedPathFragments.find((candidate) => argument.includes(candidate));
    if (fragment !== undefined) {
      return refuse("denied_argument", `argument references a protected path: ${fragment}`);
    }
  }
  if (config.roots.length === 0 || !config.roots.some((root) => withinRoot(request.cwd, root))) {
    return refuse("root_not_declared", "the working directory is outside every declared root");
  }
  if (
    rest.some(
      (argument) =>
        looksLikePath(argument) &&
        !config.roots.some((root) =>
          withinRoot(
            argument.startsWith("~") ? resolveRoot(argument) : resolve(request.cwd, argument),
            root,
          ),
        ),
    )
  ) {
    return refuse("denied_argument", "a path argument is outside every declared root");
  }
  const allowsCurated = tierAllows(config.tier, "curated");
  const allowsShell = tierAllows(config.tier, "shell");
  if (!allowsCurated) {
    return refuse("tier_insufficient", "probe tier permits fixed discovery only");
  }
  if (!allowsShell) {
    if (name === "gh") {
      return ghReadOnlyAllowed(rest)
        ? { _tag: "Allowed", needsConfirmation: false }
        : refuse("not_allowlisted", "this gh command is not a permitted read-only operation");
    }
    const permitted = curatedAllowlist[name];
    const argumentRule = curatedArgumentRules[name];
    if (permitted === undefined || argumentRule === undefined) {
      return refuse("not_allowlisted", `${name} is not in the curated allowlist`);
    }
    if (!argumentRule(rest)) {
      const first = rest[0];
      return refuse(
        "not_allowlisted",
        `${name} ${first ?? ""} is not in the curated allowlist`.trim(),
      );
    }
    return { _tag: "Allowed", needsConfirmation: false };
  }
  return { _tag: "Allowed", needsConfirmation: !config.preApproved.includes(name) };
};

const noArguments = (args: ReadonlyArray<string>): boolean => args.length === 0;
const nonOptionArguments = (args: ReadonlyArray<string>): boolean =>
  args.length > 0 && args.every((argument) => argument === "--" || !argument.startsWith("-"));
const boundedWords = (args: ReadonlyArray<string>): boolean =>
  args.length <= 16 &&
  args.every((argument) => !argument.startsWith("-") && argument.length <= 256);

const gitArguments = (args: ReadonlyArray<string>): boolean => {
  const [subcommand, ...rest] = args;
  if (subcommand === "--version") return rest.length === 0;
  if (subcommand === undefined) return false;
  if (
    !["status", "log", "diff", "branch", "remote", "show", "rev-parse", "ls-files"].includes(
      subcommand,
    )
  ) {
    return false;
  }
  const denied = new Set([
    "--exec-path",
    "--ext-diff",
    "--no-ext-diff",
    "--textconv",
    "--no-textconv",
    "--delete",
    "-D",
    "-d",
    "--set-upstream",
    "--unset-upstream",
    "set-url",
    "set-head",
    "set-branches",
    "update-ref",
  ]);
  if (subcommand === "branch") {
    const branchReadOptions = new Set([
      "-a",
      "-r",
      "--all",
      "--remotes",
      "--contains",
      "--merged",
      "--no-merged",
      "--list",
      "--verbose",
      "-v",
    ]);
    return rest.every(
      (argument) =>
        branchReadOptions.has(argument) ||
        (argument.startsWith("--sort=") && argument.length <= 128),
    );
  }
  if (subcommand === "remote") {
    return rest.every((argument) => ["-v", "--verbose"].includes(argument));
  }
  return rest.every(
    (argument) =>
      !denied.has(argument) &&
      !argument.startsWith("--upload-pack=") &&
      !argument.startsWith("--output"),
  );
};

const curatedArgumentRules: Readonly<Record<string, (args: ReadonlyArray<string>) => boolean>> = {
  git: gitArguments,
  uname: noArguments,
  date: noArguments,
  echo: boundedWords,
  whoami: noArguments,
  df: noArguments,
  du: nonOptionArguments,
  ps: noArguments,
  uptime: noArguments,
  file: nonOptionArguments,
  stat: nonOptionArguments,
  ls: nonOptionArguments,
  cat: nonOptionArguments,
  head: nonOptionArguments,
  tail: nonOptionArguments,
  wc: nonOptionArguments,
  pwd: noArguments,
  which: boundedWords,
  rg: nonOptionArguments,
  grep: nonOptionArguments,
  node: (args) => args.length === 1 && args[0] === "--version",
  npm: (args) => args.length === 1 && (args[0] === "--version" || args[0] === "ls"),
  pnpm: (args) => args.length === 1 && (args[0] === "--version" || args[0] === "ls"),
  python3: (args) => args.length === 1 && args[0] === "--version",
  cargo: (args) => args.length === 1 && args[0] === "--version",
  go: (args) => args.length === 1 && args[0] === "version",
  docker: (args) =>
    args.length === 1 && (args[0] === "ps" || args[0] === "images" || args[0] === "version"),
};

export const formatAllowlist = (): ReadonlyArray<string> => [
  ...Object.entries(curatedAllowlist).map(
    ([name, args]) =>
      `${name}: ${args.length === 0 ? "no options; path arguments inside declared roots" : args.join(", ")}`,
  ),
  "gh: read-only queries only",
];
