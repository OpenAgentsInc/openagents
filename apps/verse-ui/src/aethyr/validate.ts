// View and intent checks, mirroring crates/aethyr-ui (validate.rs). Both
// sides run the same fixtures (crates/aethyr-ui/fixtures) and must return
// the same error codes.
import {
  MAX_DEPTH,
  MAX_FIELD_BYTES,
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
      break;
    case "input":
      if (element.kind === "field") {
        if (element.enabled === false) return fail("disabled");
        if (value === undefined || byteLength(value) > element.max_bytes)
          return fail("bad_value");
        return ok(node);
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
