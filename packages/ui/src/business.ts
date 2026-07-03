import { clsx } from "clsx";
import type { Attribute, Html } from "foldkit/html";
import { html } from "foldkit/html";

import { inputGroup, textareaGroup } from "./forms";
import { kitFamily, statusDotClass } from "./primitives";
import type { PublicLandingThemeMode } from "./public-theme";
import { linkButton } from "./shared";

export type BusinessLandingMode = PublicLandingThemeMode;

export type BusinessAvailability =
  | "available_now"
  | "operator_assisted"
  | "roadmap"
  | "blocked";

export type BusinessOffering = Readonly<{
  title: string;
  availability: BusinessAvailability;
  what: string;
  liveNow: string;
  caveat: string;
  quickWin: string;
  promiseIds?: ReadonlyArray<string>;
}>;

export type BusinessLadderStep = Readonly<{
  when: string;
  title: string;
  body: string;
}>;

export type BusinessRateCardPackage = Readonly<{
  title: string;
  price: string;
  scope: string;
  receiptPlan: ReadonlyArray<string>;
  caveat: string;
  promiseIds?: ReadonlyArray<string>;
}>;

export type BusinessIntakeFieldNames = Readonly<{
  businessName: string;
  contactEmail: string;
  website: string;
  phone: string;
  helpWith: string;
  requestSlackChannel: string;
  referralCode: string;
}>;

export const defaultBusinessIntakeFieldNames: BusinessIntakeFieldNames = {
  businessName: "businessName",
  contactEmail: "contactEmail",
  website: "website",
  phone: "phone",
  helpWith: "helpWith",
  requestSlackChannel: "requestSlackChannel",
  referralCode: "referralCode",
};

const theme = (mode: BusinessLandingMode = "dark") =>
  mode === "light"
    ? {
        canvas: "bg-public-landing-page text-public-landing-text",
        surface:
          "border-public-landing-border bg-public-landing-surface text-public-landing-text",
        raised:
          "border-public-landing-border bg-public-landing-surface text-public-landing-text shadow-sm",
        inset:
          "border-public-landing-border bg-public-landing-surface-muted text-public-landing-text",
        heading: "text-public-landing-text",
        body: "text-public-landing-muted",
        muted: "text-public-landing-muted",
        faint: "text-public-landing-faint",
        border: "border-public-landing-border",
        divider: "border-public-landing-border",
        accent: "text-public-landing-accent",
        warningText: "text-public-landing-warning",
        fieldInput:
          "!border-public-landing-border !bg-public-landing-surface !text-public-landing-text placeholder:!text-public-landing-faint focus:!border-public-landing-accent focus:!ring-public-landing-accent",
        fieldLabel: "!text-public-landing-muted",
        fieldHelp: "!text-public-landing-faint",
        strongButton:
          "border-public-landing-text bg-public-landing-text text-public-landing-page hover:opacity-90",
      }
    : {
        canvas: "bg-[#000] text-[#f1efe8]",
        surface: "border-[#222] bg-[#010102] text-[#f1efe8]",
        raised: "border-[#222] bg-[#010102] text-[#f1efe8]",
        inset: "border-[#222] bg-[#030303] text-[#f1efe8]",
        heading: "text-[#f1efe8]",
        body: "text-white/65",
        muted: "text-white/55",
        faint: "text-white/35",
        border: "border-[#222]",
        divider: "border-[#222]",
        accent: "text-[#ffb400]",
        warningText: "text-[#ffd54a]/85",
        fieldInput: "",
        fieldLabel: "",
        fieldHelp: "",
        strongButton:
          "border-[#f1efe8] bg-[#f1efe8] text-[#000] hover:bg-white",
      };

const availabilityLabel: Record<BusinessAvailability, string> = {
  available_now: "Available now",
  operator_assisted: "Operator-assisted",
  roadmap: "Roadmap",
  blocked: "Blocked",
};

const availabilityTone: Record<BusinessAvailability, string> = {
  available_now: "positive",
  operator_assisted: "warning",
  roadmap: "neutral",
  blocked: "negative",
};

const availabilityBadgeClass = (
  availability: BusinessAvailability,
  mode: BusinessLandingMode,
): string => {
  if (mode === "light") {
    return clsx({
      "border-public-landing-positive bg-public-landing-surface-muted text-public-landing-positive":
        availability === "available_now",
      "border-public-landing-warning bg-public-landing-surface-muted text-public-landing-warning":
        availability === "operator_assisted",
      "border-public-landing-border bg-public-landing-surface-muted text-public-landing-muted":
        availability === "roadmap",
      "border-public-landing-negative bg-public-landing-surface-muted text-public-landing-negative":
        availability === "blocked",
    });
  }

  return clsx({
    "border-[#1f4d2b] bg-[#06140a] text-[#7fdc9b]":
      availability === "available_now",
    "border-[#4d3f00] bg-[#141004] text-[#ffd54a]":
      availability === "operator_assisted",
    "border-[#222] bg-[#070707] text-white/55": availability === "roadmap",
    "border-[#4d1111] bg-[#160404] text-[#ff9a9a]": availability === "blocked",
  });
};

const mergeAttrs = <Message>(
  attrs: ReadonlyArray<Attribute<Message>> | undefined,
  className: string,
): ReadonlyArray<Attribute<Message>> => [
  ...(attrs ?? []),
  html<Message>().Class(className),
];

export const businessAvailabilityBadge = <Message>(input: {
  availability: BusinessAvailability;
  label?: string;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";

  return h.span(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>("business/availability-badges"),
      h.DataAttribute("business-availability", input.availability),
      h.Class(
        clsx(
          "inline-flex shrink-0 items-center gap-1.5 border py-1 pr-2 pl-1 font-mono text-[0.6875rem] uppercase tracking-wide",
          availabilityBadgeClass(input.availability, mode),
          input.className,
        ),
      ),
    ],
    [
      h.span(
        [
          h.AriaHidden(true),
          h.Class(
            statusDotClass(
              availabilityTone[input.availability] as Parameters<
                typeof statusDotClass
              >[0],
            ),
          ),
        ],
        [],
      ),
      input.label ?? availabilityLabel[input.availability],
    ],
  );
};

export const businessLandingHero = <Message>(input: {
  eyebrow?: string;
  title: string;
  body: string;
  secondaryBody?: string;
  primaryHref: string;
  primaryLabel: string;
  secondaryHref?: string;
  secondaryLabel?: string;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);

  return h.section(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-4 border-b pb-8", t.divider, input.className),
      ),
      kitFamily<Message>("business/landing-heroes"),
    ],
    [
      h.p(
        [h.Class(clsx("m-0 font-mono text-base sm:text-sm", t.faint))],
        [input.eyebrow ?? "For your business"],
      ),
      h.h1(
        [
          h.Class(
            clsx(
              "m-0 max-w-[14ch] text-balance text-4xl font-medium tracking-normal sm:text-5xl",
              t.heading,
            ),
          ),
        ],
        [input.title],
      ),
      h.p(
        [h.Class(clsx("m-0 max-w-[68ch] text-base/7", t.body))],
        [input.body],
      ),
      input.secondaryBody === undefined
        ? null
        : h.p(
            [h.Class(clsx("m-0 max-w-[68ch] text-base/7", t.muted))],
            [input.secondaryBody],
          ),
      h.div(
        [h.Class("mt-2 flex flex-wrap items-center gap-3")],
        [
          linkButton<Message>({
            href: input.primaryHref,
            label: input.primaryLabel,
            attrs: [h.Class(t.strongButton)],
          }),
          input.secondaryHref === undefined ||
          input.secondaryLabel === undefined
            ? null
            : linkButton<Message>({
                href: input.secondaryHref,
                label: input.secondaryLabel,
                variant: "secondary",
              }),
        ],
      ),
    ],
  );
};

export const businessOfferingCard = <Message>(input: {
  offering: BusinessOffering;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);

  return h.article(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-3 border p-4", t.raised, input.className),
      ),
      kitFamily<Message>("business/offering-cards"),
      h.DataAttribute("business-offering-title", input.offering.title),
    ],
    [
      h.div(
        [h.Class("flex items-start justify-between gap-3")],
        [
          h.h3(
            [h.Class(clsx("m-0 text-base font-medium", t.heading))],
            [input.offering.title],
          ),
          businessAvailabilityBadge<Message>({
            availability: input.offering.availability,
            mode,
          }),
        ],
      ),
      h.p([h.Class(clsx("m-0 text-sm/6", t.body))], [input.offering.what]),
      h.p(
        [h.Class(clsx("m-0 text-sm/6", t.muted))],
        [`Live now: ${input.offering.liveNow}`],
      ),
      h.p(
        [h.Class(clsx("m-0 text-sm/6", t.warningText))],
        [`Current caveat: ${input.offering.caveat}`],
      ),
      h.p(
        [h.Class(clsx("m-0 font-mono text-xs", t.faint))],
        [input.offering.quickWin],
      ),
      input.offering.promiseIds === undefined ||
      input.offering.promiseIds.length === 0
        ? null
        : h.p(
            [h.Class(clsx("m-0 font-mono text-xs", t.faint))],
            [`Promise refs: ${input.offering.promiseIds.join(", ")}`],
          ),
    ],
  );
};

export const businessOfferingMenu = <Message>(input: {
  title?: string;
  body?: string;
  offerings: ReadonlyArray<BusinessOffering>;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);

  return h.section(
    [
      ...mergeAttrs<Message>(input.attrs, clsx("grid gap-4", input.className)),
      kitFamily<Message>("business/offering-menus"),
    ],
    [
      h.div(
        [h.Class("grid gap-2")],
        [
          h.h2(
            [h.Class(clsx("m-0 text-xl font-medium", t.heading))],
            [input.title ?? "What we can do"],
          ),
          input.body === undefined
            ? null
            : h.p(
                [h.Class(clsx("m-0 max-w-[68ch] text-sm/6", t.muted))],
                [input.body],
              ),
        ],
      ),
      h.div(
        [h.Class("grid gap-3 md:grid-cols-2")],
        input.offerings.map((offering) =>
          businessOfferingCard<Message>({ offering, mode }),
        ),
      ),
    ],
  );
};

export const quickWinLadder = <Message>(input: {
  title?: string;
  body?: string;
  steps: ReadonlyArray<BusinessLadderStep>;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);

  return h.section(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-4 border-t pt-8", t.divider, input.className),
      ),
      kitFamily<Message>("business/quick-win-ladders"),
    ],
    [
      h.div(
        [h.Class("grid gap-2")],
        [
          h.h2(
            [h.Class(clsx("m-0 text-xl font-medium", t.heading))],
            [input.title ?? "Quick win -> put your business on Autopilot"],
          ),
          input.body === undefined
            ? null
            : h.p(
                [h.Class(clsx("m-0 max-w-[68ch] text-sm/6", t.muted))],
                [input.body],
              ),
        ],
      ),
      h.ol(
        [h.Class("m-0 grid gap-3 p-0")],
        [
          ...input.steps.map((step) =>
            h.li(
              [
                h.Class(clsx("grid gap-1 border p-4 list-none", t.raised)),
                h.DataAttribute("business-ladder-step", step.when),
              ],
              [
                h.p(
                  [h.Class(clsx("m-0 font-mono text-xs", t.accent))],
                  [`${step.when} - ${step.title}`],
                ),
                h.p([h.Class(clsx("m-0 text-sm/6", t.body))], [step.body]),
              ],
            ),
          ),
        ],
      ),
    ],
  );
};

export const businessRateCard = <Message>(input: {
  title?: string;
  body?: string;
  packages: ReadonlyArray<BusinessRateCardPackage>;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);

  return h.section(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-4 border-t pt-8", t.divider, input.className),
      ),
      kitFamily<Message>("business/rate-cards"),
    ],
    [
      h.div(
        [h.Class("grid gap-2")],
        [
          h.h2(
            [h.Class(clsx("m-0 text-xl font-medium", t.heading))],
            [input.title ?? "Rate card"],
          ),
          input.body === undefined
            ? null
            : h.p(
                [h.Class(clsx("m-0 max-w-[68ch] text-sm/6", t.muted))],
                [input.body],
              ),
        ],
      ),
      h.div(
        [h.Class("grid gap-3 lg:grid-cols-2")],
        input.packages.map((pkg) =>
          h.article(
            [
              h.Class(clsx("grid gap-3 border p-4", t.raised)),
              kitFamily<Message>("business/rate-card-packages"),
              h.DataAttribute("business-rate-card-package", pkg.title),
            ],
            [
              h.div(
                [h.Class("flex flex-wrap items-start justify-between gap-3")],
                [
                  h.h3(
                    [
                      h.Class(
                        clsx(
                          "m-0 max-w-[24ch] text-base font-medium",
                          t.heading,
                        ),
                      ),
                    ],
                    [pkg.title],
                  ),
                  h.p(
                    [
                      h.Class(
                        clsx(
                          "m-0 border px-2 py-1 font-mono text-sm",
                          t.inset,
                          t.accent,
                        ),
                      ),
                    ],
                    [pkg.price],
                  ),
                ],
              ),
              h.p([h.Class(clsx("m-0 text-sm/6", t.body))], [pkg.scope]),
              h.div(
                [h.Class("grid gap-2")],
                [
                  h.p(
                    [h.Class(clsx("m-0 font-mono text-xs", t.faint))],
                    ["Receipt plan"],
                  ),
                  h.ul(
                    [h.Class("m-0 grid gap-1.5 p-0")],
                    [
                      ...pkg.receiptPlan.map((item) =>
                        h.li(
                          [
                            h.Class(
                              clsx(
                                "grid grid-cols-[auto_minmax(0,1fr)] gap-2 text-sm/6",
                                t.muted,
                              ),
                            ),
                          ],
                          [
                            h.span(
                              [h.AriaHidden(true), h.Class(t.accent)],
                              ["-"],
                            ),
                            h.span([], [item]),
                          ],
                        ),
                      ),
                    ],
                  ),
                ],
              ),
              h.p(
                [h.Class(clsx("m-0 text-sm/6", t.warningText))],
                [pkg.caveat],
              ),
              pkg.promiseIds === undefined || pkg.promiseIds.length === 0
                ? null
                : h.p(
                    [h.Class(clsx("m-0 font-mono text-xs", t.faint))],
                    [`Promise refs: ${pkg.promiseIds.join(", ")}`],
                  ),
            ],
          ),
        ),
      ),
    ],
  );
};

export const publicProofCaveat = <Message>(input: {
  eyebrow?: string;
  title: string;
  body: string;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const t = theme(input.mode);

  return h.aside(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-2 border px-4 py-3", t.inset, input.className),
      ),
      kitFamily<Message>("business/proof-caveats"),
    ],
    [
      h.p(
        [
          h.Class(
            clsx("m-0 font-mono text-xs uppercase tracking-wide", t.faint),
          ),
        ],
        [input.eyebrow ?? "Promise boundary"],
      ),
      h.h2(
        [h.Class(clsx("m-0 text-base font-medium", t.heading))],
        [input.title],
      ),
      h.p([h.Class(clsx("m-0 text-sm/6", t.body))], [input.body]),
    ],
  );
};

export const businessProjectInvite = <Message>(input: {
  eyebrow?: string;
  title: string;
  body: string;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const t = theme(input.mode);

  return h.section(
    [
      ...mergeAttrs<Message>(
        input.attrs,
        clsx("grid gap-3 border p-4", t.raised, input.className),
      ),
      kitFamily<Message>("business/project-invites"),
    ],
    [
      h.p(
        [
          h.Class(
            clsx("m-0 font-mono text-xs uppercase tracking-wide", t.faint),
          ),
        ],
        [input.eyebrow ?? "Project invite"],
      ),
      h.h2(
        [h.Class(clsx("m-0 text-lg font-medium", t.heading))],
        [input.title],
      ),
      h.p([h.Class(clsx("m-0 text-base/7", t.body))], [input.body]),
    ],
  );
};

export const businessIntakeForm = <Message>(input: {
  action: string;
  fieldNames?: Partial<BusinessIntakeFieldNames>;
  method?: "post" | "get";
  title?: string;
  pricingNote: string | Html;
  privacyNote?: string;
  submitLabel?: string;
  slackLabel?: string;
  slackDetail?: string;
  mode?: BusinessLandingMode;
  className?: string;
  attrs?: ReadonlyArray<Attribute<Message>>;
}): Html => {
  const h = html<Message>();
  const mode = input.mode ?? "dark";
  const t = theme(mode);
  const names = { ...defaultBusinessIntakeFieldNames, ...input.fieldNames };

  return h.form(
    [
      ...(input.attrs ?? []),
      kitFamily<Message>("business/intake-forms"),
      h.Method(input.method ?? "post"),
      h.Action(input.action),
      h.AriaLabel(input.title ?? "Business signup"),
      h.Class(clsx("grid gap-4 border p-5 sm:p-6", t.raised, input.className)),
    ],
    [
      h.input([
        h.Id("business-referral-code"),
        h.Name(names.referralCode),
        h.Type("hidden"),
        h.Value(""),
      ]),
      input.title === undefined
        ? null
        : h.h2(
            [h.Class(clsx("m-0 text-lg font-medium", t.heading))],
            [input.title],
          ),
      inputGroup<Message>({
        id: "business-name",
        name: names.businessName,
        label: "Business name",
        placeholder: "Acme Co.",
        attrs: [h.Required(true), h.Attribute("autocomplete", "organization")],
        className: t.fieldInput,
        labelClassName: t.fieldLabel,
      }),
      inputGroup<Message>({
        id: "business-email",
        name: names.contactEmail,
        label: "Work email",
        type: "email",
        placeholder: "you@example.com",
        attrs: [
          h.Required(true),
          h.Attribute("autocomplete", "email"),
          h.Attribute("inputmode", "email"),
        ],
        className: t.fieldInput,
        labelClassName: t.fieldLabel,
      }),
      inputGroup<Message>({
        id: "business-website",
        name: names.website,
        label: "Website / URL",
        type: "url",
        placeholder: "https://example.com",
        help: "We use your public site to set up your workspace.",
        attrs: [
          h.Attribute("autocomplete", "url"),
          h.Attribute("inputmode", "url"),
        ],
        className: t.fieldInput,
        labelClassName: t.fieldLabel,
        helpClassName: t.fieldHelp,
      }),
      inputGroup<Message>({
        id: "business-phone",
        name: names.phone,
        label: "Phone number",
        type: "tel",
        placeholder: "+1 555 000 0000",
        help: "So we can reach you to get started.",
        attrs: [
          h.Required(true),
          h.Attribute("autocomplete", "tel"),
          h.Attribute("inputmode", "tel"),
        ],
        className: t.fieldInput,
        labelClassName: t.fieldLabel,
        helpClassName: t.fieldHelp,
      }),
      textareaGroup<Message>({
        id: "business-help",
        name: names.helpWith,
        label: "What do you want help with?",
        placeholder: "Describe the work you want done, in your own words.",
        rows: 4,
        className: t.fieldInput,
        labelClassName: t.fieldLabel,
      }),
      h.label(
        [
          h.For("business-slack-optin"),
          h.Class(
            clsx(
              "grid cursor-pointer grid-cols-[auto_minmax(0,1fr)] gap-3 border px-3 py-2.5",
              t.inset,
            ),
          ),
        ],
        [
          h.input([
            h.Id("business-slack-optin"),
            h.Name(names.requestSlackChannel),
            h.Type("checkbox"),
            h.Value("yes"),
            h.Class("mt-0.5 size-5 shrink-0 accent-[#ffb400] sm:size-4"),
          ]),
          h.span(
            [h.Class("grid gap-0.5")],
            [
              h.span(
                [h.Class(clsx("text-sm font-medium", t.heading))],
                [input.slackLabel ?? "Request a shared Slack channel"],
              ),
              h.span(
                [h.Class(clsx("font-mono text-xs", t.faint))],
                [
                  input.slackDetail ??
                    "We can set up a shared Slack channel so your team and your AI workforce can talk in one place.",
                ],
              ),
            ],
          ),
        ],
      ),
      typeof input.pricingNote === "string"
        ? publicProofCaveat<Message>({
            title: "Pricing and payment",
            body: input.pricingNote,
            mode,
          })
        : input.pricingNote,
      h.button(
        [
          h.Type("submit"),
          h.Class(
            clsx(
              "min-h-10 cursor-pointer border px-4 font-mono text-[0.8125rem]",
              t.strongButton,
            ),
          ),
        ],
        [input.submitLabel ?? "Get started"],
      ),
      h.p(
        [h.Class(clsx("m-0 font-mono text-xs", t.faint))],
        [
          input.privacyNote ??
            "We only use your details to set up your workspace and get in touch.",
        ],
      ),
    ],
  );
};
