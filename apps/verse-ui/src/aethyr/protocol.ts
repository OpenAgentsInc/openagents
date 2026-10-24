// Generated from crates/aethyr-ui (the aethyr.ui.v1 schema); do not edit.
// Regenerate: cargo run -p aethyr-ui -- --write

export const SCHEMA = "aethyr.ui.v1";
export const SCHEMA_SHA256 = "7cb7158967ef3a98dd1dcd0052830489501b3dd5705907c50374a6cebde1bce8";
export const MAX_NODES = 1024;
export const MAX_DEPTH = 16;
export const MAX_VIEW_BYTES = 524288;
export const MAX_ID_BYTES = 128;
export const MAX_TEXT_BYTES = 16384;
export const MAX_FIELD_BYTES = 65536;
export const ELEMENT_KINDS = ["panel", "stack", "text", "label", "button", "chip", "list", "field", "sheet", "dialog", "toast", "media_slot", "engine_surface"] as const;

/**
 * One published revision of a controller's interface.
 *
 * A new controller lifetime takes a fresh `instance`. A revision names one
 * immutable tree in that lifetime; the controller never reuses a revision
 * for different content.
 */
export interface View {
  /**
   * Always `aethyr.ui.v1`.
   */
  readonly schema: string;
  readonly instance: string;
  /**
   * Positive, and increasing within an instance.
   */
  readonly revision: number;
  readonly root: Node;
}

/**
 * A semantic node. Its `id` is stable across revisions for the same thing,
 * so a renderer keeps focus, scroll, and mounted media on it.
 */
export interface Node {
  /**
   * Unique in the view: letters, digits, `-`, `_`, `.`, `:`.
   */
  readonly id: string;
  readonly style?: Style;
  readonly a11y?: A11y;
  readonly element: Element;
}

/**
 * Presentation, by token reference only.
 */
export interface Style {
  readonly gap?: Space;
  readonly padding?: Space;
  readonly align?: Align;
  readonly tone?: Tone;
  /**
   * A secondary node, hidden at phone width.
   */
  readonly hide_on_phone?: boolean;
}

/**
 * A step on the spacing scale (`--hud-space-*`).
 */
export type Space = "none" | "xs" | "sm" | "md" | "lg" | "xl";

export type Align = "start" | "center" | "end" | "between";

/**
 * A color role, resolved by the renderer through the design tokens.
 */
export type Tone = "default" | "muted" | "primary" | "danger";

/**
 * Accessibility overrides. A control's visible label is its accessible
 * name unless `label` says otherwise.
 */
export interface A11y {
  readonly role?: Role;
  readonly label?: string;
  readonly description?: string;
  readonly live?: Live;
  /**
   * Hidden from assistive technology (decorative).
   */
  readonly hidden?: boolean;
}

/**
 * An explicit accessibility role. Each kind has its own default; set this
 * only to change it.
 */
export type Role = "group" | "region" | "navigation" | "status" | "alert" | "presentation";

export type Live = "polite" | "assertive";

/**
 * The closed set of node kinds. Anything else needs its own contract first.
 */
export type Element =
  /**
   * A HUD glass panel with corner ticks.
   */
  | {
    readonly title?: string;
    readonly children: ReadonlyArray<Node>;
    readonly kind: "panel";
  }
  /**
   * Children in a row, a column, or a wrapping row.
   */
  | {
    readonly axis: Axis;
    readonly children: ReadonlyArray<Node>;
    readonly kind: "stack";
  }
  /**
   * Read-only text.
   */
  | {
    readonly value: string;
    readonly role: TextRole;
    readonly kind: "text";
  }
  /**
   * A mono, uppercase HUD readout, optionally led by a pulsing signal dot.
   */
  | {
    readonly value: string;
    readonly signal?: boolean;
    readonly kind: "label";
  }
  /**
   * A control. Sends `activate`. With `href` (a same-site path) it is a
   * link: a plain click still sends `activate`, so the controller decides,
   * and the link keeps working without the controller.
   */
  | {
    readonly label: string;
    readonly enabled?: boolean;
    readonly emphasis?: Emphasis;
    readonly href?: string;
    readonly leading?: Glyph;
    readonly trailing?: Glyph;
    readonly kind: "button";
  }
  /**
   * A small toggle or filter. Sends `activate`; `selected` is controller
   * state, never inferred from color.
   */
  | {
    readonly label: string;
    readonly selected: boolean;
    readonly enabled?: boolean;
    readonly kind: "chip";
  }
  /**
   * Rows. When `selectable`, choosing a row sends `select` with that
   * row's (direct child's) ID as the value.
   */
  | {
    readonly label: string;
    readonly selectable?: boolean;
    readonly selected?: string;
    readonly children: ReadonlyArray<Node>;
    readonly kind: "list";
  }
  /**
   * An editable value. Typing sends `input` with the whole value; Enter
   * (single line) sends `submit`.
   */
  | {
    readonly label: string;
    readonly value: string;
    readonly placeholder?: string;
    readonly secret?: boolean;
    readonly multiline?: boolean;
    readonly enabled?: boolean;
    readonly max_bytes: number;
    readonly kind: "field";
  }
  /**
   * A non-modal panel: beside the content on wide screens, a bottom sheet
   * on phones. Sends `dismiss` when `dismissible`.
   */
  | {
    readonly label: string;
    readonly open: boolean;
    readonly dismissible?: boolean;
    readonly children: ReadonlyArray<Node>;
    readonly kind: "sheet";
  }
  /**
   * A modal focus scope. Sends `dismiss` (Escape) when `dismissible`; the
   * controller decides whether it closes. The renderer restores focus.
   */
  | {
    readonly label: string;
    readonly open: boolean;
    readonly dismissible?: boolean;
    readonly children: ReadonlyArray<Node>;
    readonly kind: "dialog";
  }
  /**
   * A short status message, announced politely. Sends `dismiss` when
   * `dismissible`.
   */
  | {
    readonly message: string;
    readonly tone?: Tone;
    readonly dismissible?: boolean;
    readonly kind: "toast";
  }
  /**
   * A place the host fills with an image, video, or illustration it
   * registered under `slot`. Never a URL.
   */
  | {
    readonly slot: string;
    readonly label: string;
    readonly aspect?: Aspect;
    readonly kind: "media_slot";
  }
  /**
   * The engine's drawing surface, registered by the host under `surface`.
   * The renderer keeps it mounted across revisions and never sends
   * intents for it; the engine takes its own input.
   */
  | {
    readonly surface: string;
    readonly label: string;
    readonly kind: "engine_surface";
  };

export type Axis = "vertical" | "horizontal" | "wrap";

export type TextRole = "body" | "heading" | "code" | "title" | "caption" | "status";

export type Emphasis = "secondary" | "primary" | "ghost" | "link";

/**
 * A decorative mark beside a button's label. Hidden from assistive
 * technology; the label carries the meaning.
 */
export type Glyph = "forward" | "back" | "close";

export type Aspect = "square" | "wide" | "tall";

/**
 * What the person did, sent by the renderer. It carries identity and, for
 * `select` and `input`, a value; the current tree says what it means.
 * Input values are personal data: never log them.
 */
export interface Intent {
  readonly instance: string;
  /**
   * The revision the renderer drew. An intent for an older revision is
   * refused as stale.
   */
  readonly revision: number;
  readonly node: string;
  readonly kind: IntentKind;
  readonly value?: string;
}

export type IntentKind = "activate" | "select" | "submit" | "dismiss" | "input";

/** The JSON Schema these types come from; the web validator decodes with it. */
export const SCHEMA_DOCUMENT: unknown = {
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://openagents.com/schemas/aethyr.ui.v1.schema.json",
  "title": "aethyr.ui.v1",
  "description": "Aethyr UI protocol: a view tree published by a controller (View) and the intents a renderer sends back (Intent). Generated from crates/aethyr-ui; do not edit.",
  "$ref": "#/$defs/View",
  "$defs": {
    "View": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "schema": {
          "type": "string",
          "description": "Always `aethyr.ui.v1`."
        },
        "instance": {
          "type": "string",
          "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
        },
        "revision": {
          "type": "integer",
          "format": "uint64",
          "minimum": 1,
          "description": "Positive, and increasing within an instance."
        },
        "root": {
          "$ref": "#/$defs/Node"
        }
      },
      "required": [
        "schema",
        "instance",
        "revision",
        "root"
      ],
      "description": "One published revision of a controller's interface.\n\nA new controller lifetime takes a fresh `instance`. A revision names one\nimmutable tree in that lifetime; the controller never reuses a revision\nfor different content."
    },
    "Node": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "id": {
          "type": "string",
          "description": "Unique in the view: letters, digits, `-`, `_`, `.`, `:`.",
          "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
        },
        "style": {
          "$ref": "#/$defs/Style"
        },
        "a11y": {
          "$ref": "#/$defs/A11y"
        },
        "element": {
          "$ref": "#/$defs/Element"
        }
      },
      "required": [
        "id",
        "element"
      ],
      "description": "A semantic node. Its `id` is stable across revisions for the same thing,\nso a renderer keeps focus, scroll, and mounted media on it."
    },
    "Style": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "gap": {
          "anyOf": [
            {
              "$ref": "#/$defs/Space"
            },
            {
              "type": "null"
            }
          ]
        },
        "padding": {
          "anyOf": [
            {
              "$ref": "#/$defs/Space"
            },
            {
              "type": "null"
            }
          ]
        },
        "align": {
          "anyOf": [
            {
              "$ref": "#/$defs/Align"
            },
            {
              "type": "null"
            }
          ]
        },
        "tone": {
          "anyOf": [
            {
              "$ref": "#/$defs/Tone"
            },
            {
              "type": "null"
            }
          ]
        },
        "hide_on_phone": {
          "type": "boolean",
          "description": "A secondary node, hidden at phone width."
        }
      },
      "description": "Presentation, by token reference only."
    },
    "Space": {
      "type": "string",
      "enum": [
        "none",
        "xs",
        "sm",
        "md",
        "lg",
        "xl"
      ],
      "description": "A step on the spacing scale (`--hud-space-*`)."
    },
    "Align": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "start",
            "center",
            "end"
          ]
        },
        {
          "type": "string",
          "const": "between",
          "description": "Spread children to both ends (stacks only)."
        }
      ]
    },
    "Tone": {
      "type": "string",
      "enum": [
        "default",
        "muted",
        "primary",
        "danger"
      ],
      "description": "A color role, resolved by the renderer through the design tokens."
    },
    "A11y": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "role": {
          "anyOf": [
            {
              "$ref": "#/$defs/Role"
            },
            {
              "type": "null"
            }
          ]
        },
        "label": {
          "type": [
            "string",
            "null"
          ]
        },
        "description": {
          "type": [
            "string",
            "null"
          ]
        },
        "live": {
          "anyOf": [
            {
              "$ref": "#/$defs/Live"
            },
            {
              "type": "null"
            }
          ]
        },
        "hidden": {
          "type": "boolean",
          "description": "Hidden from assistive technology (decorative)."
        }
      },
      "description": "Accessibility overrides. A control's visible label is its accessible\nname unless `label` says otherwise."
    },
    "Role": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "group",
            "region",
            "navigation",
            "status",
            "alert"
          ]
        },
        {
          "type": "string",
          "const": "presentation",
          "description": "Decorative: removed from the accessibility tree."
        }
      ],
      "description": "An explicit accessibility role. Each kind has its own default; set this\nonly to change it."
    },
    "Live": {
      "type": "string",
      "enum": [
        "polite",
        "assertive"
      ]
    },
    "Element": {
      "oneOf": [
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "title": {
              "type": [
                "string",
                "null"
              ]
            },
            "children": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/Node"
              }
            },
            "kind": {
              "type": "string",
              "const": "panel"
            }
          },
          "required": [
            "kind",
            "children"
          ],
          "description": "A HUD glass panel with corner ticks."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "axis": {
              "$ref": "#/$defs/Axis"
            },
            "children": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/Node"
              }
            },
            "kind": {
              "type": "string",
              "const": "stack"
            }
          },
          "required": [
            "kind",
            "axis",
            "children"
          ],
          "description": "Children in a row, a column, or a wrapping row."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "value": {
              "type": "string"
            },
            "role": {
              "$ref": "#/$defs/TextRole"
            },
            "kind": {
              "type": "string",
              "const": "text"
            }
          },
          "required": [
            "kind",
            "value",
            "role"
          ],
          "description": "Read-only text."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "value": {
              "type": "string"
            },
            "signal": {
              "type": "boolean"
            },
            "kind": {
              "type": "string",
              "const": "label"
            }
          },
          "required": [
            "kind",
            "value"
          ],
          "description": "A mono, uppercase HUD readout, optionally led by a pulsing signal dot."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "enabled": {
              "type": "boolean"
            },
            "emphasis": {
              "$ref": "#/$defs/Emphasis"
            },
            "href": {
              "type": [
                "string",
                "null"
              ]
            },
            "leading": {
              "anyOf": [
                {
                  "$ref": "#/$defs/Glyph"
                },
                {
                  "type": "null"
                }
              ]
            },
            "trailing": {
              "anyOf": [
                {
                  "$ref": "#/$defs/Glyph"
                },
                {
                  "type": "null"
                }
              ]
            },
            "kind": {
              "type": "string",
              "const": "button"
            }
          },
          "required": [
            "kind",
            "label"
          ],
          "description": "A control. Sends `activate`. With `href` (a same-site path) it is a\nlink: a plain click still sends `activate`, so the controller decides,\nand the link keeps working without the controller."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "selected": {
              "type": "boolean"
            },
            "enabled": {
              "type": "boolean"
            },
            "kind": {
              "type": "string",
              "const": "chip"
            }
          },
          "required": [
            "kind",
            "label",
            "selected"
          ],
          "description": "A small toggle or filter. Sends `activate`; `selected` is controller\nstate, never inferred from color."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "selectable": {
              "type": "boolean"
            },
            "selected": {
              "type": [
                "string",
                "null"
              ]
            },
            "children": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/Node"
              }
            },
            "kind": {
              "type": "string",
              "const": "list"
            }
          },
          "required": [
            "kind",
            "label",
            "children"
          ],
          "description": "Rows. When `selectable`, choosing a row sends `select` with that\nrow's (direct child's) ID as the value."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "value": {
              "type": "string"
            },
            "placeholder": {
              "type": "string"
            },
            "secret": {
              "type": "boolean"
            },
            "multiline": {
              "type": "boolean"
            },
            "enabled": {
              "type": "boolean"
            },
            "max_bytes": {
              "type": "integer",
              "format": "uint32",
              "minimum": 0
            },
            "kind": {
              "type": "string",
              "const": "field"
            }
          },
          "required": [
            "kind",
            "label",
            "value",
            "max_bytes"
          ],
          "description": "An editable value. Typing sends `input` with the whole value; Enter\n(single line) sends `submit`."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "open": {
              "type": "boolean"
            },
            "dismissible": {
              "type": "boolean"
            },
            "children": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/Node"
              }
            },
            "kind": {
              "type": "string",
              "const": "sheet"
            }
          },
          "required": [
            "kind",
            "label",
            "open",
            "children"
          ],
          "description": "A non-modal panel: beside the content on wide screens, a bottom sheet\non phones. Sends `dismiss` when `dismissible`."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "label": {
              "type": "string"
            },
            "open": {
              "type": "boolean"
            },
            "dismissible": {
              "type": "boolean"
            },
            "children": {
              "type": "array",
              "items": {
                "$ref": "#/$defs/Node"
              }
            },
            "kind": {
              "type": "string",
              "const": "dialog"
            }
          },
          "required": [
            "kind",
            "label",
            "open",
            "children"
          ],
          "description": "A modal focus scope. Sends `dismiss` (Escape) when `dismissible`; the\ncontroller decides whether it closes. The renderer restores focus."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "message": {
              "type": "string"
            },
            "tone": {
              "$ref": "#/$defs/Tone"
            },
            "dismissible": {
              "type": "boolean"
            },
            "kind": {
              "type": "string",
              "const": "toast"
            }
          },
          "required": [
            "kind",
            "message"
          ],
          "description": "A short status message, announced politely. Sends `dismiss` when\n`dismissible`."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "slot": {
              "type": "string",
              "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
            },
            "label": {
              "type": "string"
            },
            "aspect": {
              "anyOf": [
                {
                  "$ref": "#/$defs/Aspect"
                },
                {
                  "type": "null"
                }
              ]
            },
            "kind": {
              "type": "string",
              "const": "media_slot"
            }
          },
          "required": [
            "kind",
            "slot",
            "label"
          ],
          "description": "A place the host fills with an image, video, or illustration it\nregistered under `slot`. Never a URL."
        },
        {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "surface": {
              "type": "string",
              "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
            },
            "label": {
              "type": "string"
            },
            "kind": {
              "type": "string",
              "const": "engine_surface"
            }
          },
          "required": [
            "kind",
            "surface",
            "label"
          ],
          "description": "The engine's drawing surface, registered by the host under `surface`.\nThe renderer keeps it mounted across revisions and never sends\nintents for it; the engine takes its own input."
        }
      ],
      "description": "The closed set of node kinds. Anything else needs its own contract first."
    },
    "Axis": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "vertical",
            "horizontal"
          ]
        },
        {
          "type": "string",
          "const": "wrap",
          "description": "Left to right, continuing on the next line."
        }
      ]
    },
    "TextRole": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "body",
            "heading",
            "code"
          ]
        },
        {
          "type": "string",
          "const": "title",
          "description": "The main heading of a panel or dialog."
        },
        {
          "type": "string",
          "const": "caption",
          "description": "Small supporting text."
        },
        {
          "type": "string",
          "const": "status",
          "description": "Text that changes and is announced politely."
        }
      ]
    },
    "Emphasis": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "secondary"
          ]
        },
        {
          "type": "string",
          "const": "primary",
          "description": "The one main action of a group."
        },
        {
          "type": "string",
          "const": "ghost",
          "description": "No fill until hovered."
        },
        {
          "type": "string",
          "const": "link",
          "description": "Inline, like a text link."
        }
      ]
    },
    "Glyph": {
      "oneOf": [
        {
          "type": "string",
          "const": "forward",
          "description": "Go somewhere (↗)."
        },
        {
          "type": "string",
          "const": "back",
          "description": "Go back (←)."
        },
        {
          "type": "string",
          "const": "close",
          "description": "Close (×)."
        }
      ],
      "description": "A decorative mark beside a button's label. Hidden from assistive\ntechnology; the label carries the meaning."
    },
    "Aspect": {
      "oneOf": [
        {
          "type": "string",
          "enum": [
            "square"
          ]
        },
        {
          "type": "string",
          "const": "wide",
          "description": "16:9."
        },
        {
          "type": "string",
          "const": "tall",
          "description": "3:4."
        }
      ]
    },
    "Intent": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "instance": {
          "type": "string",
          "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
        },
        "revision": {
          "type": "integer",
          "format": "uint64",
          "minimum": 1,
          "description": "The revision the renderer drew. An intent for an older revision is\nrefused as stale."
        },
        "node": {
          "type": "string",
          "pattern": "^[A-Za-z0-9_.:-]{1,128}$"
        },
        "kind": {
          "$ref": "#/$defs/IntentKind"
        },
        "value": {
          "type": [
            "string",
            "null"
          ]
        }
      },
      "required": [
        "instance",
        "revision",
        "node",
        "kind"
      ],
      "description": "What the person did, sent by the renderer. It carries identity and, for\n`select` and `input`, a value; the current tree says what it means.\nInput values are personal data: never log them."
    },
    "IntentKind": {
      "oneOf": [
        {
          "type": "string",
          "const": "activate",
          "description": "Press a button or chip."
        },
        {
          "type": "string",
          "const": "select",
          "description": "Choose a row of a selectable list; `value` is the row's ID."
        },
        {
          "type": "string",
          "const": "submit",
          "description": "Submit a field."
        },
        {
          "type": "string",
          "const": "dismiss",
          "description": "Close a dismissible dialog, sheet, or toast."
        },
        {
          "type": "string",
          "const": "input",
          "description": "A field's new value, in full."
        }
      ]
    }
  }
};
