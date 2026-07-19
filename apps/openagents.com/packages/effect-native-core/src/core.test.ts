import { describe, expect, test } from "vite-plus/test";
import { Effect, Exit, Option, Schema } from "effect";

import {
  blurFormField,
  chordEquals,
  ComponentValueBinding,
  decodeIntent,
  defineFormSpec,
  encodeIntent,
  FieldBinding,
  formatChord,
  formFieldError,
  formFieldFocused,
  formFieldValue,
  FormFieldValueBinding,
  IntentRef,
  makeFormState,
  makeIntent,
  makeKeymap,
  normalizeChordKey,
  resolveAlertAppearance,
  resolveBadgeAppearance,
  resolveButtonAppearance,
  resolveIntentRef,
  resolveSelectAppearance,
  resolveTextFieldAppearance,
  rovingTabIndex,
  serializeIntentEvent,
  setFormFieldValue,
  StaticPayload,
  UnknownIntentError,
} from "./index";

// Structural casts so we can call resolvers with only the fields they read,
// without constructing whole ViewNode trees.
type ButtonInput = Parameters<typeof resolveButtonAppearance>[0];
type FormStateInput = Parameters<typeof formFieldFocused>[0];

describe("keybinding formatting", () => {
  test("normalizeChordKey lowercases only single-char keys", () => {
    expect(normalizeChordKey("A")).toBe("a");
    expect(normalizeChordKey("Z")).toBe("z");
    expect(normalizeChordKey("Enter")).toBe("Enter");
    expect(normalizeChordKey("ArrowUp")).toBe("ArrowUp");
  });

  test("chordEquals is case-insensitive on single-char keys and strict on modifiers", () => {
    expect(chordEquals({ key: "A" }, { key: "a" })).toBe(true);
    expect(chordEquals({ key: "a", meta: true }, { key: "A", meta: true })).toBe(true);
    expect(chordEquals({ key: "a", meta: true }, { key: "a" })).toBe(false);
    expect(chordEquals({ key: "a", meta: true }, { key: "a", meta: true, shift: true })).toBe(
      false,
    );
    expect(chordEquals({ key: "Enter" }, { key: "enter" })).toBe(false);
  });

  test("formatChord ios uses glyphs in ctrl/alt/shift/meta order with no separators", () => {
    expect(formatChord({ key: "a", ctrl: true, alt: true, shift: true, meta: true }, "ios")).toBe(
      "⌃⌥⇧⌘A",
    );
    expect(formatChord({ key: "s", meta: true }, "ios")).toBe("⌘S");
    expect(formatChord({ key: "Enter", meta: true }, "ios")).toBe("⌘Enter");
  });

  test("formatChord non-ios spells modifiers and joins with '+'", () => {
    expect(formatChord({ key: "a", ctrl: true, alt: true, shift: true, meta: true }, "web")).toBe(
      "Ctrl+Alt+Shift+Meta+A",
    );
    expect(formatChord({ key: "Enter", meta: true }, "web")).toBe("Meta+Enter");
    // default platform is "web"
    expect(formatChord({ key: "k" })).toBe("K");
  });
});

describe("rovingTabIndex", () => {
  test("active index gets 0, all others -1", () => {
    expect(rovingTabIndex(3, 1)).toEqual([-1, 0, -1]);
    expect(rovingTabIndex(4, 0)).toEqual([0, -1, -1, -1]);
    expect(rovingTabIndex(0, 0)).toEqual([]);
    // out-of-range active index yields all -1
    expect(rovingTabIndex(2, 5)).toEqual([-1, -1]);
  });
});

describe("appearance resolvers", () => {
  test("resolveButtonAppearance normalizes legacy variants", () => {
    expect(resolveButtonAppearance({ variant: "primary" } as ButtonInput)).toEqual({
      tone: "accent",
      variant: "solid",
      size: "md",
    });
    expect(resolveButtonAppearance({ variant: "secondary" } as ButtonInput)).toEqual({
      tone: "secondary",
      variant: "solid",
      size: "md",
    });
    expect(resolveButtonAppearance({ variant: "ghost" } as ButtonInput)).toEqual({
      tone: "accent",
      variant: "ghost",
      size: "md",
    });
    expect(resolveButtonAppearance({} as ButtonInput)).toEqual({
      tone: "accent",
      variant: "solid",
      size: "md",
    });
    // explicit tone/size override legacy tone table
    expect(
      resolveButtonAppearance({ variant: "primary", tone: "danger", size: "lg" } as ButtonInput),
    ).toEqual({ tone: "danger", variant: "solid", size: "lg" });
  });

  test("resolveBadgeAppearance maps legacy tone and flags isLegacy", () => {
    expect(resolveBadgeAppearance({})).toEqual({
      tone: "secondary",
      variant: "ghost",
      size: "md",
      isLegacy: true,
    });
    expect(resolveBadgeAppearance({ tone: "warn", variant: "solid" })).toEqual({
      tone: "warning",
      variant: "solid",
      size: "md",
      isLegacy: false,
    });
    expect(resolveBadgeAppearance({ tone: "success", size: "sm" })).toEqual({
      tone: "success",
      variant: "ghost",
      size: "sm",
      isLegacy: false,
    });
  });

  test("resolveTextFieldAppearance switches tone to danger when invalid", () => {
    expect(resolveTextFieldAppearance({})).toEqual({
      tone: "secondary",
      variant: "outline",
      size: "md",
      isLegacy: true,
    });
    expect(resolveTextFieldAppearance({ invalid: true })).toEqual({
      tone: "danger",
      variant: "outline",
      size: "md",
      isLegacy: true,
    });
    expect(resolveTextFieldAppearance({ variant: "soft", size: "lg" })).toEqual({
      tone: "secondary",
      variant: "soft",
      size: "lg",
      isLegacy: false,
    });
  });

  test("resolveSelectAppearance defaults tone/icon and coerces pill", () => {
    expect(resolveSelectAppearance({})).toEqual({
      tone: "secondary",
      variant: "outline",
      size: "md",
      pill: false,
      dropdownIcon: "ChevronDown",
      isLegacy: true,
    });
    expect(
      resolveSelectAppearance({ variant: "ghost", pill: true, dropdownIcon: "Filter" }),
    ).toEqual({
      tone: "secondary",
      variant: "ghost",
      size: "md",
      pill: true,
      dropdownIcon: "Filter",
      isLegacy: false,
    });
  });

  test("resolveAlertAppearance picks default icon per tone", () => {
    expect(resolveAlertAppearance({})).toEqual({
      tone: "info",
      variant: "soft",
      icon: "InfoCircle",
    });
    expect(resolveAlertAppearance({ tone: "danger" })).toEqual({
      tone: "danger",
      variant: "soft",
      icon: "AlertCircle",
    });
    expect(resolveAlertAppearance({ tone: "success" })).toEqual({
      tone: "success",
      variant: "soft",
      icon: "CheckCircle",
    });
    expect(resolveAlertAppearance({ tone: "success", variant: "outline", icon: "Star" })).toEqual({
      tone: "success",
      variant: "outline",
      icon: "Star",
    });
  });
});

describe("form helpers", () => {
  const spec = defineFormSpec({
    id: "f",
    fields: [
      { name: "name", schema: Schema.String, initialValue: "" },
      {
        name: "age",
        schema: Schema.Number,
        initialValue: 0,
        validateOn: "change",
        invalidMessage: "NaN",
      },
    ],
  });

  test("makeFormState seeds untouched initial values", () => {
    expect(makeFormState(spec)).toEqual({
      id: "f",
      fields: {
        name: { value: "", touched: false },
        age: { value: 0, touched: false },
      },
    });
  });

  test("setFormFieldValue on a submit-validated field just marks touched", () => {
    const next = setFormFieldValue(spec, makeFormState(spec), "name", "Alice");
    expect(next.fields.name).toEqual({ value: "Alice", touched: true });
    expect(formFieldValue(next, "name")).toBe("Alice");
    // untouched sibling is unchanged
    expect(next.fields.age).toEqual({ value: 0, touched: false });
  });

  test("setFormFieldValue with validateOn:change records invalid message, clears on valid", () => {
    const invalid = setFormFieldValue(spec, makeFormState(spec), "age", "notnum");
    expect(invalid.fields.age).toEqual({ value: "notnum", touched: true, error: "NaN" });
    expect(formFieldError(invalid, "age")).toBe("NaN");
    expect(formFieldValue(invalid, "age")).toBe("notnum");

    const valid = setFormFieldValue(spec, makeFormState(spec), "age", 42);
    expect(valid.fields.age).toEqual({ value: 42, touched: true });
    expect(formFieldError(valid, "age")).toBe("");
  });

  test("blurFormField only validates when validateOn is blur", () => {
    const blurSpec = defineFormSpec({
      id: "b",
      fields: [
        {
          name: "n",
          schema: Schema.Number,
          initialValue: 0,
          validateOn: "blur",
          invalidMessage: "bad",
        },
      ],
    });
    // setting the value does not validate (trigger is blur, not change)
    const typed = setFormFieldValue(blurSpec, makeFormState(blurSpec), "n", "xyz");
    expect(formFieldError(typed, "n")).toBe("");
    // blur triggers validation -> error appears
    const blurred = blurFormField(blurSpec, typed, "n");
    expect(formFieldError(blurred, "n")).toBe("bad");
    expect(blurred.fields.n?.touched).toBe(true);
  });

  test("formFieldFocused compares focusedField exactly", () => {
    expect(
      formFieldFocused({ id: "f", fields: {}, focusedField: "name" } as FormStateInput, "name"),
    ).toBe(true);
    expect(
      formFieldFocused({ id: "f", fields: {}, focusedField: "name" } as FormStateInput, "age"),
    ).toBe(false);
    expect(formFieldFocused({ id: "f", fields: {} } as FormStateInput, "name")).toBe(false);
  });
});

describe("intents", () => {
  test("encode/decode round-trip preserves name and payload", () => {
    const intent = makeIntent("Save", { form: "login", value: 3 });
    expect(decodeIntent(encodeIntent(intent))).toEqual(intent);
    expect(intent).toEqual({ name: "Save", payload: { form: "login", value: 3 } });
  });

  test("resolveIntentRef handles every payload template", () => {
    expect(resolveIntentRef(IntentRef("Save"))).toEqual({ name: "Save", payload: null });
    expect(resolveIntentRef(IntentRef("Save", StaticPayload({ x: 1 })))).toEqual({
      name: "Save",
      payload: { x: 1 },
    });
    expect(resolveIntentRef(IntentRef("Save", ComponentValueBinding()), "hello")).toEqual({
      name: "Save",
      payload: "hello",
    });
    expect(
      resolveIntentRef(
        IntentRef("Save", FormFieldValueBinding(FieldBinding("login", "email"))),
        42,
      ),
    ).toEqual({
      name: "Save",
      payload: { form: "login", field: "email", value: 42 },
    });
  });

  test("serializeIntentEvent maps success and IntentError failures", () => {
    const success = serializeIntentEvent({
      timestamp: 5,
      intent: makeIntent("Save", null),
      result: Exit.succeed(undefined),
    });
    expect(success).toEqual({
      timestamp: 5,
      intent: { name: "Save", payload: null },
      result: { _tag: "Success" },
    });

    const failure = serializeIntentEvent({
      timestamp: 7,
      intent: makeIntent("Nope", null),
      result: Exit.fail(new UnknownIntentError({ name: "Nope" })),
    });
    expect(failure).toEqual({
      timestamp: 7,
      intent: { name: "Nope", payload: null },
      result: { _tag: "Failure", error: { _tag: "UnknownIntentError", name: "Nope" } },
    });
  });
});

describe("makeKeymap", () => {
  test("detects same-scope chord conflicts (case-insensitive key)", () => {
    const keymap = Effect.runSync(
      makeKeymap([
        { id: "a", title: "A", intent: IntentRef("a"), binding: { key: "k", meta: true } },
        { id: "b", title: "B", intent: IntentRef("b"), binding: { key: "K", meta: true } },
      ]),
    );
    expect(keymap.conflicts).toEqual([
      { chord: { key: "k", meta: true }, scope: "global", commandIds: ["a", "b"] },
    ]);
  });

  test("resolve + keybindingLabel honor platform and case-folding", () => {
    const keymap = Effect.runSync(
      makeKeymap(
        [
          {
            id: "save",
            title: "Save",
            intent: IntentRef("Save"),
            binding: { key: "s", meta: true },
          },
        ],
        { platform: "ios" },
      ),
    );
    const resolved = Effect.runSync(keymap.resolve({ key: "S", meta: true }));
    expect(Option.getOrNull(resolved)?.id).toBe("save");
    expect(Effect.runSync(keymap.resolve({ key: "s" }))).toEqual(Option.none());
    expect(keymap.keybindingLabel("save")).toEqual(Option.some("⌘S"));
    expect(keymap.keybindingLabel("missing")).toEqual(Option.none());
  });

  test("a pushed scope shadows a global binding with the same chord", () => {
    const keymap = Effect.runSync(
      makeKeymap([
        { id: "g", title: "G", scope: "global", intent: IntentRef("g"), binding: { key: "Enter" } },
        { id: "m", title: "M", scope: "modal", intent: IntentRef("m"), binding: { key: "Enter" } },
      ]),
    );
    // before push: only the global command is reachable
    expect(Option.getOrNull(Effect.runSync(keymap.resolve({ key: "Enter" })))?.id).toBe("g");
    Effect.runSync(keymap.pushScope("modal"));
    expect(Option.getOrNull(Effect.runSync(keymap.resolve({ key: "Enter" })))?.id).toBe("m");
    // popping restores the global binding
    Effect.runSync(keymap.popScope);
    expect(Option.getOrNull(Effect.runSync(keymap.resolve({ key: "Enter" })))?.id).toBe("g");
  });
});
