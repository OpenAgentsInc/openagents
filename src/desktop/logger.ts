const RESET = "\x1b[0m";
const COLORS: Record<string, string> = {
  Desktop: "\x1b[36m", // cyan
  DesktopServer: "\x1b[35m", // magenta
  Worker: "\x1b[33m", // yellow
  Webview: "\x1b[32m", // green
};

const FALLBACK_COLORS = ["\x1b[34m", "\x1b[31m", "\x1b[90m"];
const fallbackAssignments = new Map<string, string>();

const supportsColor = typeof process !== "undefined" && process.stdout?.isTTY !== false;

const colorFor = (source: string): string => {
  if (!supportsColor) {
    return "";
  }

  const known = COLORS[source];
  if (known) return known;

  if (fallbackAssignments.has(source)) {
    return fallbackAssignments.get(source)!;
  }

  const nextColor = FALLBACK_COLORS[fallbackAssignments.size % FALLBACK_COLORS.length];
  fallbackAssignments.set(source, nextColor);
  return nextColor;
};

export const formatSource = (source: string): string => {
  const color = colorFor(source);
  if (!color) return `[${source}]`;
  return `${color}[${source}]${RESET}`;
};

export const log = (source: string, ...args: unknown[]): void => {
  console.log(formatSource(source), ...args);
};

export const warn = (source: string, ...args: unknown[]): void => {
  console.warn(formatSource(source), ...args);
};

export const error = (source: string, ...args: unknown[]): void => {
  console.error(formatSource(source), ...args);
};
