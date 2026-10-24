// View and intent checks, mirroring crates/aethyr-ui (validate.rs). Both
// sides run the same fixtures (crates/aethyr-ui/fixtures) and must return
// the same error codes.
import {
  MAX_ACCEPT,
  MAX_CANVAS_PIXELS,
  MAX_DEPTH,
  MAX_FIELD_BYTES,
  MAX_FILES,
  MAX_OPTIONS,
  MAX_UPLOAD_BYTES,
  MAX_ID_BYTES,
  MAX_NODES,
  MAX_TEXT_BYTES,
  MAX_VIEW_BYTES,
  SCHEMA,
  SCHEMA_DOCUMENT,
  type Element,
  type Intent,
  type Node,
  type View,
} from "./protocol";

export type ViewErrorCode =
  | "schema"
  | "identity"
  | "bad_id"
  | "duplicate_id"
  | "node_limit"
  | "depth_limit"
  | "text_limit"
  | "view_limit"
  | "missing_label"
  | "bad_reference"
  | "encoding";

export type IntentErrorCode =
  "stale" | "unknown_node" | "not_accepted" | "disabled" | "bad_value";

export type Checked<T, E> =
  | { readonly ok: true; readonly value: T }
  | { readonly ok: false; readonly code: E; readonly detail?: string };

const ok = <T>(value: T) => ({ ok: true, value }) as const;
const fail = <E extends string>(code: E, detail?: string) =>
  ({ ok: false, code, detail }) as const;

const encoder = new TextEncoder();
/** UTF-8 length, as Rust's `str::len`. */
export const byteLength = (text: string) => encoder.encode(text).length;

export const validId = (id: string) =>
  id.length > 0 &&
  byteLength(id) <= MAX_ID_BYTES &&
  /^[A-Za-z0-9_.:-]+$/.test(id);

export const validHref = (href: string) =>
  href.startsWith("/") &&
  !href.startsWith("//") &&
  href.length <= 2048 &&
  !/[\\\p{Cc}\s]/u.test(href);

// ---------------------------------------------------------------------------
// Structural decoding against the generated JSON Schema. Like serde, it
// checks types, closed sets, required and unknown properties; patterns and
// minimums are the semantic checks below, so both sides return one code.

type Schema = { readonly [key: string]: unknown };
const defs = (SCHEMA_DOCUMENT as Schema)["$defs"] as Record<string, Schema>;

function conforms(value: unknown, schema: Schema): boolean {
  const ref = schema["$ref"];
  if (typeof ref === "string")
    return conforms(value, defs[ref.split("/").pop()!]!);
  if ("const" in schema) return value === schema["const"];
  if (Array.isArray(schema["enum"]))
    return (schema["enum"] as unknown[]).includes(value);
  if (Array.isArray(schema["oneOf"]))
    return (
      (schema["oneOf"] as Schema[]).filter((s) => conforms(value, s)).length ===
      1
    );
  if (Array.isArray(schema["anyOf"]))
    return (schema["anyOf"] as Schema[]).some((s) => conforms(value, s));
  const type = schema["type"];
  if (Array.isArray(type))
    return type.some((t) => conforms(value, { ...schema, type: t }));
  switch (type) {
    case "null":
      return value === null;
    case "boolean":
      return typeof value === "boolean";
    case "string":
      return typeof value === "string";
    case "number":
      return typeof value === "number" && Number.isFinite(value);
    case "integer": {
      if (typeof value !== "number" || !Number.isInteger(value)) return false;
      const format = schema["format"];
      if (format === "uint32") return value >= 0 && value <= 0xffffffff;
      if (format === "uint64") return value >= 0;
      return true;
    }
    case "array":
      return (
        Array.isArray(value) &&
        value.every((item) => conforms(item, schema["items"] as Schema))
      );
    case "object": {
      if (typeof value !== "object" || value === null || Array.isArray(value))
        return false;
      const properties = (schema["properties"] ?? {}) as Record<string, Schema>;
      const record = value as Record<string, unknown>;
      for (const key of (schema["required"] ?? []) as string[])
        if (!(key in record)) return false;
      for (const [key, item] of Object.entries(record)) {
        const property = properties[key];
        if (!property) {
          if (schema["additionalProperties"] === false) return false;
          continue;
        }
        if (!conforms(item, property)) return false;
      }
      return true;
    }
    default:
      return true;
  }
}

// ---------------------------------------------------------------------------
// Value checks for the form kinds, as in validate.rs.

/** `#rrggbb`, lowercase. */
export const validColor = (value: string) => /^#[0-9a-f]{6}$/.test(value);

const token = /^[a-z0-9+.-]+$/;
/** A media type (`image/png`, `image/*`) or an extension (`.json`). */
export function validAccept(entry: string): boolean {
  if (entry.startsWith(".")) return token.test(entry.slice(1));
  const at = entry.indexOf("/");
  if (at < 0) return false;
  const kind = entry.slice(0, at);
  const sub = entry.slice(at + 1);
  return token.test(kind) && (sub === "*" || token.test(sub));
}

/**
 * A range value as text: an optional `-`, digits with an optional fraction,
 * and an optional exponent. No `+`, spaces, `Infinity`, or `NaN`.
 */
export function decimalText(text: string): number | undefined {
  if (text.length > 64) return undefined;
  if (!/^-?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?$/.test(text)) return undefined;
  const value = Number(text);
  return Number.isFinite(value) ? value : undefined;
}

/** Finite, within bounds, and on the step from `min`. */
export function onRange(value: number, min: number, max: number, step: number) {
  if (!Number.isFinite(value) || value < min || value > max) return false;
  const steps = Math.round((value - min) / step);
  return (
    Math.abs(min + steps * step - value) <= 1e-9 * Math.max(Math.abs(step), 1)
  );
}

/** File names, one per line: at least one, none empty or with control characters. */
export function validFileNames(value: string, multiple: boolean): boolean {
  const names = value.split("\n");
  return (
    byteLength(value) <= MAX_TEXT_BYTES &&
    names.length <= (multiple ? MAX_FILES : 1) &&
    names.every((name) => name.trim() !== "" && !/\p{Cc}/u.test(name))
  );
}

// ---------------------------------------------------------------------------

export const children = (element: Element): ReadonlyArray<Node> =>
  "children" in element ? element.children : [];

function checkNode(node: Node): ViewErrorCode | undefined {
  const texts: string[] = [];
  if (node.a11y?.label !== undefined) texts.push(node.a11y.label);
  if (node.a11y?.description !== undefined) texts.push(node.a11y.description);
  const element = node.element;
  let label: string | undefined;
  switch (element.kind) {
    case "panel":
      if (element.title !== undefined) texts.push(element.title);
      break;
    case "stack":
      break;
    case "text":
    case "label":
      texts.push(element.value);
      break;
    case "button":
      if (element.href !== undefined && !validHref(element.href))
        return "bad_reference";
      label = element.label;
      break;
    case "chip":
      if (element.detail !== undefined) texts.push(element.detail);
      label = element.label;
      break;
    case "list":
      if (
        element.selected !== undefined &&
        (!element.selectable ||
          !element.children.some((child) => child.id === element.selected))
      )
        return "bad_reference";
      label = element.label;
      break;
    case "field":
      if (
        element.max_bytes === 0 ||
        element.max_bytes > MAX_FIELD_BYTES ||
        byteLength(element.value) > element.max_bytes
      )
        return "text_limit";
      texts.push(element.placeholder ?? "");
      label = element.label;
      break;
    case "sheet":
    case "dialog":
      label = element.label;
      break;
    case "toast":
      label = element.message;
      break;
    case "media_slot":
      if (!validId(element.slot)) return "bad_reference";
      label = element.label;
      break;
    case "engine_surface":
      if (!validId(element.surface)) return "bad_reference";
      label = element.label;
      break;
    case "select": {
      if (element.options.length === 0 || element.options.length > MAX_OPTIONS)
        return "bad_reference";
      const values = new Set<string>();
      for (const option of element.options) {
        if (option.label.trim() === "") return "missing_label";
        if (values.has(option.value)) return "bad_reference";
        values.add(option.value);
        texts.push(option.label, option.value);
      }
      if (!values.has(element.value)) return "bad_reference";
      label = element.label;
      break;
    }
    case "checkbox":
      label = element.label;
      break;
    case "range":
      if (
        element.min >= element.max ||
        element.step <= 0 ||
        element.value < element.min ||
        element.value > element.max
      )
        return "bad_reference";
      label = element.label;
      break;
    case "color":
      if (!validColor(element.value)) return "bad_reference";
      label = element.label;
      break;
    case "file":
      if (
        !validId(element.slot) ||
        (element.accept ?? []).length > MAX_ACCEPT ||
        !(element.accept ?? []).every(validAccept) ||
        element.max_bytes === 0 ||
        element.max_bytes > MAX_UPLOAD_BYTES
      )
        return "bad_reference";
      label = element.label;
      break;
    case "canvas":
      if (
        !validId(element.surface) ||
        element.width < 1 ||
        element.width > MAX_CANVAS_PIXELS ||
        element.height < 1 ||
        element.height > MAX_CANVAS_PIXELS
      )
        return "bad_reference";
      label = element.label;
      break;
  }
  if (label !== undefined) {
    if (label.trim() === "") return "missing_label";
    texts.push(label);
  }
  if (texts.some((text) => byteLength(text) > MAX_TEXT_BYTES))
    return "text_limit";
  return undefined;
}

/** Structural and semantic checks of a decoded value. */
export function validateView(value: unknown): Checked<View, ViewErrorCode> {
  if (!conforms(value, defs["View"]!)) return fail("encoding");
  const view = value as View;
  if (view.schema !== SCHEMA) return fail("schema");
  if (!validId(view.instance) || view.revision === 0) return fail("identity");
  const ids = new Set<string>();
  let count = 0;
  // Same traversal order as the Rust validator, so the first error matches.
  const pending: Array<[Node, number]> = [[view.root, 1]];
  while (pending.length > 0) {
    const [node, depth] = pending.pop()!;
    count += 1;
    if (count > MAX_NODES) return fail("node_limit");
    if (depth > MAX_DEPTH) return fail("depth_limit");
    if (!validId(node.id)) return fail("bad_id", node.id);
    if (ids.has(node.id)) return fail("duplicate_id", node.id);
    ids.add(node.id);
    const problem = checkNode(node);
    if (problem) return fail(problem, node.id);
    for (const child of children(node.element))
      pending.push([child, depth + 1]);
  }
  return ok(view);
}

/** Decode and check a view received as JSON. */
export function parseView(text: string): Checked<View, ViewErrorCode> {
  if (byteLength(text) > MAX_VIEW_BYTES) return fail("view_limit");
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (error) {
    return fail("encoding", String(error));
  }
  return validateView(value);
}

export function findNode(view: View, id: string): Node | undefined {
  const pending: Node[] = [view.root];
  while (pending.length > 0) {
    const node = pending.pop()!;
    if (node.id === id) return node;
    pending.push(...children(node.element));
  }
  return undefined;
}

export const isIntent = (value: unknown): value is Intent =>
  conforms(value, defs["Intent"]!);

/** Check an intent against the view it names; returns the node it acts on. */
export function acceptIntent(
  view: View,
  intent: Intent,
): Checked<Node, IntentErrorCode> {
  if (intent.instance !== view.instance || intent.revision !== view.revision)
    return fail("stale");
  const node = findNode(view, intent.node);
  if (!node) return fail("unknown_node");
  const { element } = node;
  const value = intent.value;
  const noValue = () => (value === undefined ? ok(node) : fail("bad_value"));
  switch (intent.kind) {
    case "activate":
      if (element.kind === "button" || element.kind === "chip")
        return element.enabled === false ? fail("disabled") : noValue();
      break;
    case "select":
      if (element.kind === "list" && element.selectable) {
        if (value === undefined) return fail("bad_value");
        return element.children.some((child) => child.id === value)
          ? ok(node)
          : fail("bad_value");
      }
      if (element.kind === "select") {
        if (element.enabled === false) return fail("disabled");
        return value !== undefined &&
          element.options.some((option) => option.value === value)
          ? ok(node)
          : fail("bad_value");
      }
      break;
    case "input":
      if (element.kind === "field") {
        if (element.enabled === false) return fail("disabled");
        if (value === undefined || byteLength(value) > element.max_bytes)
          return fail("bad_value");
        return ok(node);
      }
      if (element.kind === "checkbox") {
        if (element.enabled === false) return fail("disabled");
        return value === "true" || value === "false"
          ? ok(node)
          : fail("bad_value");
      }
      if (element.kind === "range") {
        if (element.enabled === false) return fail("disabled");
        const number = value === undefined ? undefined : decimalText(value);
        return number !== undefined &&
          onRange(number, element.min, element.max, element.step)
          ? ok(node)
          : fail("bad_value");
      }
      if (element.kind === "color") {
        if (element.enabled === false) return fail("disabled");
        return value !== undefined && validColor(value)
          ? ok(node)
          : fail("bad_value");
      }
      if (element.kind === "file") {
        if (element.enabled === false) return fail("disabled");
        return value !== undefined &&
          validFileNames(value, element.multiple === true)
          ? ok(node)
          : fail("bad_value");
      }
      break;
    case "submit":
      if (element.kind === "field")
        return element.enabled === false ? fail("disabled") : noValue();
      break;
    case "dismiss":
      if (
        ((element.kind === "sheet" || element.kind === "dialog") &&
          element.open &&
          element.dismissible) ||
        (element.kind === "toast" && element.dismissible)
      )
        return noValue();
      break;
  }
  return fail("not_accepted");
}
