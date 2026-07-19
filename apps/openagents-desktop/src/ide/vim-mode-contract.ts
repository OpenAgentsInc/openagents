import { Schema } from "effect";

export const IdeVimDecisionSchemaVersion = Schema.Literal("openagents.desktop.ide-vim-decision.v1");

export const IdeVimCapabilitySchema = Schema.Literals([
  "normal",
  "insert",
  "visual",
  "visual_line",
  "visual_block",
  "replace",
  "operator_pending",
  "motions",
  "counts",
  "operators",
  "text_objects",
  "marks",
  "registers",
  "repeat",
  "search",
  "character_find",
  "join",
  "indent",
  "case",
  "paste",
  "undo_redo",
  "system_clipboard_explicit",
  "ex_commands_bounded",
  "save_close_guarded",
  "ime_passthrough",
  "screen_reader_status",
  "keyboard_layout",
  "multi_cursor_policy",
  "split_scoping",
  "focus_escape",
  "restart_persistence",
  "handler_teardown",
]);
export type IdeVimCapability = typeof IdeVimCapabilitySchema.Type;

export const IdeVimCapabilityDecisionSchema = Schema.Struct({
  capability: IdeVimCapabilitySchema,
  disposition: Schema.Literals(["implement", "guard", "explicitly_unsupported"]),
  acceptance: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(600)),
}).annotate({ identifier: "IdeVimCapabilityDecision" });
export type IdeVimCapabilityDecision = typeof IdeVimCapabilityDecisionSchema.Type;

export const IdeVimEngineDecisionSchema = Schema.Struct({
  schemaVersion: IdeVimDecisionSchemaVersion,
  selected: Schema.Literal("first_party_public_monaco_controller"),
  dependencyPackages: Schema.Tuple([]),
  defaultEnabled: Schema.Literal(false),
  persistenceScope: Schema.Literal("user_preference"),
  commandAuthority: Schema.Literal("desktop_typed_commands"),
  replacementBoundary: Schema.Literal("VimModeController"),
  capabilities: Schema.Array(IdeVimCapabilityDecisionSchema).check(
    Schema.isMinLength(32),
    Schema.isMaxLength(32),
  ),
}).annotate({ identifier: "IdeVimEngineDecision" });
export type IdeVimEngineDecision = typeof IdeVimEngineDecisionSchema.Type;

const acceptanceByCapability: Readonly<Record<IdeVimCapability, string>> = {
  normal: "Normal mode owns only editor keystroke interpretation while focused.",
  insert: "Insert mode delegates text, composition, undo stops, and multi-cursor edits to Monaco.",
  visual: "Characterwise selection is projected through public selection APIs.",
  visual_line: "Linewise selection has deterministic inclusive-line fixtures.",
  visual_block:
    "Block selection is implemented as bounded Monaco selections with an explicit primary cursor.",
  replace: "Replace and single-character replace preserve undo grouping and exit on Escape.",
  operator_pending:
    "Pending operator/count/register state is visible and clears on blur, Escape, disable, or disposal.",
  motions:
    "Core character, word, line, paragraph, bracket, document, viewport, and go-to motions are fixture-driven.",
  counts: "Counts compose with motions/operators and have a bounded numeric accumulator.",
  operators:
    "Delete, change, yank, indent, case, and format requests use public edit/command APIs.",
  text_objects:
    "Word, quote, bracket, paragraph, and tag objects are explicit supported entries, never silent fallthrough.",
  marks:
    "Local marks are document-ref scoped; uppercase/global marks are initially unsupported and visible.",
  registers:
    "Named, numbered, small-delete, black-hole, and unnamed registers stay controller-owned per project session.",
  repeat: "Dot replay records a bounded semantic command, not raw DOM KeyboardEvents.",
  search:
    "Slash/question search delegates to the owned Monaco find controller without leaking document text.",
  character_find: "f/F/t/T and ;/, repeat are line-bounded and Unicode-code-point aware.",
  join: "Join uses a versioned edit and one undo group.",
  indent: "Indent operators call public Monaco commands and retain selection/mode invariants.",
  case: "Case transforms are versioned edits over the selected range.",
  paste: "Paste consumes controller registers; system clipboard access is never implicit.",
  undo_redo:
    "Undo/redo delegate to Monaco while Desktop remains canonical for dirty/conflict truth.",
  system_clipboard_explicit:
    "Only explicit +/* register commands request the typed clipboard capability and disclose failure.",
  ex_commands_bounded:
    ":w, :wa, :q, :qa, :wq, :x, and bounded substitutions map to typed commands; shell/file Ex commands are absent.",
  save_close_guarded:
    "Save/quit never bypasses dirty, conflict, proposal, permission, or close guards.",
  ime_passthrough:
    "Compositionstart suspends Normal mappings; composition text is accepted only through Monaco Insert mode.",
  screen_reader_status:
    "Mode, pending command, recording, and error are aria-live status projections with non-color cues.",
  keyboard_layout:
    "Mappings use normalized key/code policy with US and non-US fixture matrices and user conflicts visible.",
  multi_cursor_policy:
    "Normal mode collapses to one primary cursor unless a command explicitly supports multiple selections.",
  split_scoping:
    "Mode/register/mark state is document and editor-group scoped with no unrelated split leakage.",
  focus_escape:
    "Escape first closes owned transient UI, then returns to Normal; blur never captures global keys.",
  restart_persistence:
    "Only the on/off preference persists; volatile mode/operator/register state does not revive after restart.",
  handler_teardown:
    "Every listener, decoration, status node, command, composition hook, and timer is scope-finalized exactly once.",
};

export const ide01VimDecision = IdeVimEngineDecisionSchema.make({
  schemaVersion: "openagents.desktop.ide-vim-decision.v1",
  selected: "first_party_public_monaco_controller",
  dependencyPackages: [],
  defaultEnabled: false,
  persistenceScope: "user_preference",
  commandAuthority: "desktop_typed_commands",
  replacementBoundary: "VimModeController",
  capabilities: IdeVimCapabilitySchema.literals.map((capability) => ({
    capability,
    disposition: capability === "marks" ? "explicitly_unsupported" : "implement",
    acceptance: acceptanceByCapability[capability],
  })),
});

export const decodeIdeVimDecision = Schema.decodeUnknownSync(IdeVimEngineDecisionSchema);
