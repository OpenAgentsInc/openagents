// What the house editor draws for each of the release's controls, by the IDs
// in `crates/openagents-web/src/pages/new_lander_house.rs`. Each window keeps
// the release's order; copy that the release never changes is fixed here.
import type { Spec, Surface } from "./editor";

const TOOLS: ReadonlyArray<readonly [string, string]> = [
  ["objects", "Object"],
  ["materials", "Materials"],
  ["request", "Request"],
  ["compile", "Compile"],
  ["history", "History"],
];

const objects: Spec = {
  t: "window",
  id: "panel-objects",
  tool: "objects",
  title: "01 / Object",
  close: "close-objects",
  children: [
    { t: "text", id: "edit-target" },
    { t: "button", id: "edit-pick" },
    {
      t: "group",
      id: "prop-tools",
      children: [
        {
          t: "field",
          id: "edit-place-asset",
          label: "Prop to add at X, Y, Z below",
          max: 128,
        },
        { t: "button", id: "edit-place" },
      ],
    },
    {
      t: "group",
      id: "edit-form",
      children: [
        {
          t: "group",
          id: "edit-position",
          bridged: false,
          axis: "horizontal",
          children: [
            { t: "field", id: "edit-x", label: "X", max: 32 },
            { t: "field", id: "edit-y", label: "Y", max: 32 },
            { t: "field", id: "edit-z", label: "Z", max: 32 },
          ],
        },
        {
          t: "field",
          id: "edit-note",
          label: "Notes",
          max: 48000,
          multiline: true,
        },
        {
          t: "button",
          id: "edit-submit",
          label: "Move object",
          emphasis: "primary",
        },
      ],
    },
  ],
};

const face: Spec = {
  t: "group",
  id: "face-controls",
  children: [
    { t: "select", id: "face-area", label: "Scar area" },
    {
      t: "field",
      id: "face-request",
      label: "Describe scars",
      max: 1200,
      multiline: true,
    },
    {
      t: "static",
      id: "face-help",
      role: "caption",
      text: "Choose one cheek or the forehead, one to eight scars, thin or thick, healed or fresh, and a solid color. Painting also works on a blank face layer.",
    },
    {
      t: "more",
      id: "face-colors",
      label: "Generator colors",
      children: [
        {
          t: "static",
          id: "face-colors-text",
          role: "caption",
          text: "Pink, brown, red, blue, green, purple, tan, black, white, orange, yellow, or gray. Use the brush for other colors.",
        },
      ],
    },
    { t: "button", id: "face-generate" },
    {
      t: "canvas",
      id: "face-canvas",
      label: "Face color map; drag to paint",
      width: 256,
      height: 256,
    },
    { t: "select", id: "face-brush-mode", label: "Brush" },
    { t: "color", id: "face-brush-color", label: "Paint color" },
    { t: "range", id: "face-brush-size", label: "Brush size" },
    { t: "range", id: "face-brush-opacity", label: "Brush opacity" },
    { t: "check", id: "face-generated-visible", label: "Show generated scars" },
    { t: "range", id: "face-generated-strength", label: "Scar strength" },
    { t: "check", id: "face-handmade-visible", label: "Show handmade paint" },
    { t: "range", id: "face-handmade-strength", label: "Paint strength" },
    {
      t: "group",
      id: "face-actions",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "button", id: "face-preview" },
        { t: "button", id: "face-cancel" },
        { t: "button", id: "face-apply", emphasis: "primary" },
      ],
    },
  ],
};

const materials: Spec = {
  t: "window",
  id: "panel-materials",
  tool: "materials",
  title: "02 / Materials",
  close: "close-materials",
  children: [
    {
      t: "static",
      id: "materials-heading",
      role: "heading",
      text: "Werewolf face",
    },
    {
      t: "static",
      id: "materials-intro",
      text: "Edit one selected werewolf. The local scar generator uses no model and has no charge.",
    },
    {
      t: "static",
      id: "materials-limits",
      role: "caption",
      text: "Face layers are 256 × 256 PNGs, up to 16 KiB each for preview and Apply. You can export your sources if a layer is too large.",
    },
    { t: "button", id: "face-open" },
    face,
    {
      t: "group",
      id: "face-files",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "button", id: "face-export" },
        { t: "button", id: "face-discard" },
      ],
    },
    {
      t: "file",
      id: "face-import",
      label: "Open an exported face-layer file",
      max: 4 * 1024 * 1024,
    },
    { t: "text", id: "face-status", role: "status" },
    {
      t: "more",
      id: "material-fog",
      label: "Shared material and fog",
      children: [
        {
          t: "field",
          id: "edit-material",
          label: "Shared material ID (all surfaces using this ID)",
          max: 128,
        },
        { t: "text", id: "edit-material-ids", role: "caption" },
        {
          t: "field",
          id: "edit-parameter",
          label: "Parameter: red, green, blue, roughness, metallic",
          max: 32,
        },
        { t: "field", id: "edit-value", label: "Value", max: 32 },
        { t: "button", id: "edit-material-apply" },
        { t: "field", id: "edit-fog", label: "Fog density (0–0.06)", max: 32 },
        { t: "button", id: "edit-fog-apply" },
      ],
    },
  ],
};

const request: Spec = {
  t: "window",
  id: "panel-request",
  tool: "request",
  title: "03 / Kiln",
  close: "close-request",
  children: [
    {
      t: "field",
      id: "edit-kiln-request",
      label: "Request",
      max: 4800,
      multiline: true,
    },
    {
      t: "check",
      id: "edit-kiln-world",
      label: "Allow a world effect (all materials)",
    },
    {
      t: "group",
      id: "request-actions",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "button", id: "edit-request-review", emphasis: "primary" },
        { t: "button", id: "edit-kiln-ask" },
        { t: "button", id: "request-options" },
        { t: "button", id: "edit-kiln-stop" },
        { t: "button", id: "edit-kiln-review" },
      ],
    },
    { t: "text", id: "edit-kiln-status", role: "status" },
  ],
};

const compile: Spec = {
  t: "window",
  id: "panel-compile",
  tool: "compile",
  title: "04 / World effect",
  close: "close-compile",
  children: [
    {
      t: "static",
      id: "compile-scope",
      role: "caption",
      text: "Applies to every material in the house.",
    },
    {
      t: "field",
      id: "edit-source",
      label: "WGSL source",
      max: 4096,
      multiline: true,
    },
    {
      t: "group",
      id: "compile-actions",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "button", id: "edit-source-draft" },
        { t: "button", id: "edit-source-compile", emphasis: "primary" },
      ],
    },
    { t: "text", id: "edit-source-status", role: "status" },
    {
      t: "group",
      id: "publish-tools",
      children: [
        {
          t: "static",
          id: "publish-heading",
          role: "heading",
          text: "Playable versions",
        },
        {
          t: "static",
          id: "publish-intro",
          role: "caption",
          text: "Compile and inspect your change, then save before publishing. Revert returns to Starting house as a new change.",
        },
        { t: "select", id: "publish-visibility", label: "Who can play" },
        { t: "button", id: "publish-refresh" },
        { t: "select", id: "publish-version", label: "Published version" },
        { t: "button", id: "publish-rollback" },
        { t: "button", id: "publish-revoke" },
        { t: "link", id: "published-link" },
        { t: "link", id: "visit-manage" },
        { t: "text", id: "publish-status", role: "status" },
      ],
    },
    { t: "button", id: "edit-capture" },
    {
      t: "media",
      id: "edit-capture-image",
      label: "Captured view of the house",
    },
  ],
};

const history: Spec = {
  t: "window",
  id: "panel-history",
  tool: "history",
  title: "05 / This visit",
  close: "close-history",
  children: [
    {
      t: "group",
      id: "history-actions",
      bridged: false,
      axis: "horizontal",
      children: [
        { t: "button", id: "edit-undo" },
        { t: "button", id: "edit-redo" },
      ],
    },
    { t: "items", id: "edit-history-entries", label: "Changes this visit" },
    {
      t: "group",
      id: "project-tools",
      children: [
        {
          t: "group",
          id: "contribution-tools",
          children: [
            {
              t: "static",
              id: "contribution-heading",
              role: "heading",
              text: "Saved changes",
            },
            { t: "button", id: "project-history-refresh" },
            { t: "text", id: "project-history-status", role: "status" },
            { t: "rows", id: "project-history-rows", label: "Saved changes" },
            { t: "button", id: "project-history-more" },
            {
              t: "group",
              id: "project-history-detail",
              children: [
                { t: "text", id: "project-history-version", role: "heading" },
                {
                  t: "items",
                  id: "project-history-changes",
                  label: "What changed",
                },
                {
                  t: "text",
                  id: "project-history-change-limit",
                  role: "caption",
                },
                { t: "button", id: "project-history-download" },
              ],
            },
          ],
        },
        {
          t: "static",
          id: "projects-heading",
          role: "heading",
          text: "Saved worlds",
        },
        { t: "select", id: "project-picker", label: "Project" },
        { t: "button", id: "project-list" },
        { t: "field", id: "project-name", label: "New project name", max: 120 },
        {
          t: "group",
          id: "project-actions",
          bridged: false,
          axis: "wrap",
          children: [
            { t: "button", id: "project-create" },
            { t: "button", id: "project-open" },
            { t: "button", id: "project-save-copy", emphasis: "primary" },
            { t: "button", id: "project-undo" },
            { t: "button", id: "project-device" },
            { t: "button", id: "project-recover" },
            { t: "button", id: "project-export" },
          ],
        },
        { t: "text", id: "project-status", role: "status" },
        {
          t: "more",
          id: "project-import-more",
          label: "Import an editable or older Kiln save",
          children: [
            {
              t: "static",
              id: "project-import-help",
              role: "caption",
              text: "Export files from the older editor first. Choose its scene, material, and changes files together. Originals stay in your editable export.",
            },
            {
              t: "file",
              id: "project-import-files",
              label: "Older save files",
              max: 8 * 1024 * 1024,
            },
            {
              t: "field",
              id: "project-import-mapping",
              label: "Old asset name to current asset ID (JSON)",
              max: 4096,
              multiline: true,
            },
            { t: "button", id: "project-import" },
            { t: "text", id: "project-import-report", role: "caption" },
          ],
        },
      ],
    },
    { t: "text", id: "visit-earnings", role: "caption" },
  ],
};

/** Jarvis: its header, tool tabs, tool windows, and status. */
const jarvis: Surface = {
  id: "edit-panel",
  kind: "sheet",
  label: "Jarvis",
  dismiss: "edit-close",
  children: [
    {
      t: "group",
      id: "edit-header",
      bridged: false,
      axis: "horizontal",
      children: [
        { t: "static", id: "edit-title", role: "title", text: "Jarvis" },
        { t: "button", id: "edit-close", label: "Inspect world" },
      ],
    },
    {
      t: "group",
      id: "jarvis-toolbar",
      axis: "wrap",
      gap: "xs",
      children: TOOLS.map(
        ([id, label]) => ({ t: "tab", id: `tab-${id}`, label }) as Spec,
      ),
    },
    {
      t: "group",
      id: "edit-windows",
      bridged: false,
      gap: "md",
      children: [
        objects,
        {
          t: "group",
          id: "author-tools",
          gap: "md",
          children: [materials, request, compile, history],
        },
      ],
    },
    { t: "text", id: "edit-status", role: "status" },
  ],
};

/** Sign in, or confirm a change. Escape cancels, as the release's Cancel does. */
const signIn: Surface = {
  id: "edit-auth",
  kind: "dialog",
  label: "Confirm your change",
  dismiss: "edit-cancel",
  children: [
    { t: "text", id: "edit-auth-title", role: "title" },
    { t: "text", id: "edit-auth-description" },
    { t: "items", id: "edit-auth-changes", label: "Changes" },
    {
      t: "group",
      id: "edit-auth-actions",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "link", id: "edit-signin", emphasis: "primary" },
        { t: "button", id: "edit-confirm", emphasis: "primary" },
        { t: "button", id: "edit-auth-voice-stop" },
        { t: "button", id: "edit-cancel" },
      ],
    },
  ],
};

const publishReview: Surface = {
  id: "publish-review",
  kind: "dialog",
  label: "Publish this saved version?",
  dismiss: "publish-cancel",
  children: [
    { t: "text", id: "publish-review-title", role: "title" },
    { t: "text", id: "publish-review-target" },
    { t: "link", id: "publish-review-link" },
    { t: "text", id: "publish-review-description" },
    {
      t: "check",
      id: "publish-confirm-rights",
      label: "Confirm this release change.",
    },
    {
      t: "static",
      id: "publish-review-voice",
      role: "caption",
      text: "Say Confirm publish or Cancel publish, or use the buttons.",
    },
    {
      t: "group",
      id: "publish-review-actions",
      bridged: false,
      axis: "wrap",
      children: [
        { t: "button", id: "publish-confirm", emphasis: "primary" },
        { t: "button", id: "publish-cancel" },
      ],
    },
  ],
};

const shareCard: Surface = {
  id: "publish-share-card",
  kind: "panel",
  label: "Share this version",
  children: [
    {
      t: "static",
      id: "publish-share-heading",
      role: "caption",
      text: "Exact playable version",
    },
    { t: "link", id: "publish-share-link" },
    { t: "button", id: "publish-share-close" },
  ],
};

const courtyard: Surface = {
  id: "planet-settings",
  kind: "panel",
  label: "Courtyard settings",
  children: [
    {
      t: "more",
      id: "planet-card",
      label: "Your courtyard",
      open: true,
      children: [
        {
          t: "static",
          id: "planet-size",
          role: "caption",
          text: "64 × 64 metres · Up to 128 objects",
        },
        { t: "select", id: "planet-style", label: "Style" },
        { t: "field", id: "planet-hour", label: "Starting hour", max: 8 },
        { t: "check", id: "planet-cycle", label: "Advancing day and night" },
        { t: "field", id: "planet-day", label: "Minutes per day", max: 4 },
        { t: "field", id: "planet-seed", label: "Layout seed", max: 10 },
        {
          t: "group",
          id: "planet-actions",
          bridged: false,
          axis: "wrap",
          children: [
            { t: "button", id: "planet-generate", emphasis: "primary" },
            { t: "button", id: "planet-apply" },
          ],
        },
        { t: "text", id: "planet-status", role: "status" },
        {
          t: "static",
          id: "planet-voice",
          role: "caption",
          text: "Say Generate courtyard, or use the controls.",
        },
        {
          t: "more",
          id: "planet-voice-choices",
          label: "Voice choices",
          children: [
            {
              t: "static",
              id: "planet-voice-list",
              role: "caption",
              text: "Use soft pastels / Use bold fantasy · Set time to 18 · Pause day cycle / Start day cycle · Confirm change / Cancel change",
            },
          ],
        },
      ],
    },
  ],
};

/** Drawn in this order; dialogs last, so they sit on top. */
export const EDITOR_SURFACES: ReadonlyArray<Surface> = [
  courtyard,
  jarvis,
  shareCard,
  publishReview,
  signIn,
];
