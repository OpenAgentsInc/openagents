//! Apps SDK UI token values, ported from `@openai/apps-sdk-ui` 0.2.2
//! (commit `0f00143`, MIT, see `NOTICE`): `src/styles/variables-primitive.css`,
//! `variables-semantic.css`, and `variables-components.css`.
//!
//! Values are kept as written upstream, except seven intent text roles
//! marked `openagents: WCAG AA`, moved one or two ramp steps so they pass
//! the contrast test in both themes. Upstream syntax is kept, including the `alpha()` and
//! `spacing()` build functions, which `openagents_ui::tokens::css_value` lowers to plain
//! CSS. Generated once from the upstream files; edit here, then regenerate
//! the stylesheets (see `crates/openagents-ui/src/tokens/mod.rs`).

use crate::Section;

pub const PRIMITIVE: &[Section] = &[
    Section {
        title: "Gray",
        tokens: &[
            ("--gray-0", "light-dark(#ffffff, #0d0d0d)"),
            ("--gray-25", "light-dark(#fcfcfc, #101010)"),
            ("--gray-50", "light-dark(#f9f9f9, #131313)"),
            ("--gray-75", "light-dark(#f3f3f3, #161616)"),
            ("--gray-100", "light-dark(#ededed, #181818)"),
            ("--gray-150", "light-dark(#dfdfdf, #1c1c1c)"),
            ("--gray-200", "light-dark(#cdcdcd, #212121)"),
            ("--gray-250", "light-dark(#b9b9b9, #282828)"),
            ("--gray-300", "light-dark(#afafaf, #303030)"),
            ("--gray-350", "light-dark(#9f9f9f, #393939)"),
            ("--gray-400", "light-dark(#8f8f8f, #414141)"),
            ("--gray-450", "light-dark(#767676, #4f4f4f)"),
            ("--gray-500", "#5d5d5d"),
            ("--gray-550", "light-dark(#4f4f4f, #767676)"),
            ("--gray-600", "light-dark(#414141, #8f8f8f)"),
            ("--gray-650", "light-dark(#393939, #9f9f9f)"),
            ("--gray-700", "light-dark(#303030, #afafaf)"),
            ("--gray-750", "light-dark(#282828, #b9b9b9)"),
            ("--gray-800", "light-dark(#212121, #cdcdcd)"),
            ("--gray-850", "light-dark(#1c1c1c, #dcdcdc)"),
            ("--gray-900", "light-dark(#181818, #ededed)"),
            ("--gray-925", "light-dark(#161616, #f3f3f3)"),
            ("--gray-950", "light-dark(#131313, #f3f3f3)"),
            ("--gray-975", "light-dark(#101010, #f9f9f9)"),
            ("--gray-1000", "light-dark(#0d0d0d, #ffffff)"),
        ],
    },
    Section {
        title: "Alpha transparency",
        tokens: &[
            ("--alpha-base", "light-dark(#0d0d0d, #ffffff)"),
            ("--alpha-0", "alpha(var(--alpha-base), 0%)"),
            ("--alpha-02", "alpha(var(--alpha-base), 2%)"),
            ("--alpha-04", "alpha(var(--alpha-base), 4%)"),
            ("--alpha-05", "alpha(var(--alpha-base), 5%)"),
            ("--alpha-06", "alpha(var(--alpha-base), 6%)"),
            ("--alpha-08", "alpha(var(--alpha-base), 8%)"),
            ("--alpha-10", "alpha(var(--alpha-base), 10%)"),
            ("--alpha-12", "alpha(var(--alpha-base), 12%)"),
            ("--alpha-15", "alpha(var(--alpha-base), 15%)"),
            ("--alpha-16", "alpha(var(--alpha-base), 16%)"),
            ("--alpha-20", "alpha(var(--alpha-base), 20%)"),
            ("--alpha-25", "alpha(var(--alpha-base), 25%)"),
            ("--alpha-30", "alpha(var(--alpha-base), 30%)"),
            ("--alpha-35", "alpha(var(--alpha-base), 35%)"),
            ("--alpha-40", "alpha(var(--alpha-base), 40%)"),
            ("--alpha-50", "alpha(var(--alpha-base), 50%)"),
            ("--alpha-60", "alpha(var(--alpha-base), 60%)"),
            ("--alpha-70", "alpha(var(--alpha-base), 70%)"),
        ],
    },
    Section {
        title: "Consistent contrast",
        tokens: &[("--white", "#ffffff"), ("--black", "#000000")],
    },
    Section {
        title: "Green",
        tokens: &[
            ("--green-25", "#edfaf2"),
            ("--green-50", "#d9f4e4"),
            ("--green-75", "#b8ebcc"),
            ("--green-100", "#8cdfad"),
            ("--green-200", "#66d492"),
            ("--green-300", "#40c977"),
            ("--green-400", "#04b84c"),
            ("--green-500", "#00a240"),
            ("--green-600", "#008635"),
            ("--green-700", "#00692a"),
            ("--green-800", "#004f1f"),
            ("--green-900", "#003716"),
            ("--green-950", "#011c0b"),
            ("--green-1000", "#001207"),
            ("--green-a25", "alpha(var(--green-400), 8%)"),
            ("--green-a50", "alpha(var(--green-400), 15%)"),
            ("--green-a75", "alpha(var(--green-400), 29%)"),
            ("--green-a100", "alpha(var(--green-400), 45%)"),
            ("--green-a200", "alpha(var(--green-400), 60%)"),
            ("--green-a300", "alpha(var(--green-400), 75%)"),
        ],
    },
    Section {
        title: "Red",
        tokens: &[
            ("--red-25", "#fff0f0"),
            ("--red-50", "#ffd9d9"),
            ("--red-75", "#ffc6c5"),
            ("--red-100", "#ffa4a2"),
            ("--red-200", "#ff8583"),
            ("--red-300", "#ff6764"),
            ("--red-400", "#fa423e"),
            ("--red-500", "#e02e2a"),
            ("--red-600", "#ba2623"),
            ("--red-700", "#911e1b"),
            ("--red-800", "#6e1615"),
            ("--red-900", "#4d100e"),
            ("--red-950", "#280b0a"),
            ("--red-1000", "#1f0909"),
            ("--red-a25", "alpha(var(--red-400), 8%)"),
            ("--red-a50", "alpha(var(--red-400), 16%)"),
            ("--red-a75", "alpha(var(--red-400), 30%)"),
            ("--red-a100", "alpha(var(--red-400), 48%)"),
            ("--red-a200", "alpha(var(--red-400), 64%)"),
            ("--red-a300", "alpha(var(--red-400), 79%)"),
        ],
    },
    Section {
        title: "Pink",
        tokens: &[
            ("--pink-25", "#fff4f9"),
            ("--pink-50", "#ffe8f3"),
            ("--pink-75", "#ffd4e8"),
            ("--pink-100", "#ffbada"),
            ("--pink-200", "#ffa3ce"),
            ("--pink-300", "#ff8cc1"),
            ("--pink-400", "#ff66ad"),
            ("--pink-500", "#e04c91"),
            ("--pink-600", "#ba437a"),
            ("--pink-700", "#963c67"),
            ("--pink-800", "#6e2c4a"),
            ("--pink-900", "#4d1f34"),
            ("--pink-950", "#29101c"),
            ("--pink-1000", "#1a0a11"),
            ("--pink-a25", "alpha(var(--pink-400), 8%)"),
            ("--pink-a50", "alpha(var(--pink-400), 16%)"),
            ("--pink-a75", "alpha(var(--pink-400), 28%)"),
            ("--pink-a100", "alpha(var(--pink-400), 45%)"),
            ("--pink-a200", "alpha(var(--pink-400), 60%)"),
            ("--pink-a300", "alpha(var(--pink-400), 76%)"),
        ],
    },
    Section {
        title: "Orange",
        tokens: &[
            ("--orange-25", "#fff5f0"),
            ("--orange-50", "#ffe7d9"),
            ("--orange-75", "#ffcfb4"),
            ("--orange-100", "#ffb790"),
            ("--orange-200", "#ff9e6c"),
            ("--orange-300", "#ff8549"),
            ("--orange-400", "#fb6a22"),
            ("--orange-500", "#e25507"),
            ("--orange-600", "#b9480d"),
            ("--orange-700", "#923b0f"),
            ("--orange-800", "#6d2e0f"),
            ("--orange-900", "#4a2206"),
            ("--orange-950", "#281105"),
            ("--orange-1000", "#211107"),
            ("--orange-a25", "alpha(var(--orange-400), 7%)"),
            ("--orange-a50", "alpha(var(--orange-400), 16%)"),
            ("--orange-a75", "alpha(var(--orange-400), 33%)"),
            ("--orange-a100", "alpha(var(--orange-400), 48%)"),
            ("--orange-a200", "alpha(var(--orange-400), 65%)"),
            ("--orange-a300", "alpha(var(--orange-400), 81%)"),
        ],
    },
    Section {
        title: "Yellow",
        tokens: &[
            ("--yellow-25", "#fffbed"),
            ("--yellow-50", "#fff6d9"),
            ("--yellow-75", "#ffeeb8"),
            ("--yellow-100", "#ffe48c"),
            ("--yellow-200", "#ffdb66"),
            ("--yellow-300", "#ffd240"),
            ("--yellow-400", "#ffc300"),
            ("--yellow-500", "#e0ac00"),
            ("--yellow-600", "#ba8e00"),
            ("--yellow-700", "#916f00"),
            ("--yellow-800", "#6e5400"),
            ("--yellow-900", "#4d3b00"),
            ("--yellow-950", "#261d00"),
            ("--yellow-1000", "#1a1400"),
            ("--yellow-a25", "alpha(var(--yellow-400), 8%)"),
            ("--yellow-a50", "alpha(var(--yellow-400), 15%)"),
            ("--yellow-a75", "alpha(var(--yellow-400), 27%)"),
            ("--yellow-a100", "alpha(var(--yellow-400), 45%)"),
            ("--yellow-a200", "alpha(var(--yellow-400), 59%)"),
            ("--yellow-a300", "alpha(var(--yellow-400), 74%)"),
        ],
    },
    Section {
        title: "Purple",
        tokens: &[
            ("--purple-25", "#f9f5fe"),
            ("--purple-50", "#efe5fe"),
            ("--purple-75", "#e0cefd"),
            ("--purple-100", "#ceb0fb"),
            ("--purple-200", "#be95fa"),
            ("--purple-300", "#ad7bf9"),
            ("--purple-400", "#924ff7"),
            ("--purple-500", "#8046d9"),
            ("--purple-600", "#6b3ab4"),
            ("--purple-700", "#532d8d"),
            ("--purple-800", "#3f226a"),
            ("--purple-900", "#2c184a"),
            ("--purple-950", "#160c25"),
            ("--purple-1000", "#100a19"),
            ("--purple-a25", "alpha(var(--purple-400), 6%)"),
            ("--purple-a50", "alpha(var(--purple-400), 15%)"),
            ("--purple-a75", "alpha(var(--purple-400), 28%)"),
            ("--purple-a100", "alpha(var(--purple-400), 45%)"),
            ("--purple-a200", "alpha(var(--purple-400), 60%)"),
            ("--purple-a300", "alpha(var(--purple-400), 75%)"),
        ],
    },
    Section {
        title: "Blue",
        tokens: &[
            ("--blue-25", "#f5faff"),
            ("--blue-50", "#e5f3ff"),
            ("--blue-75", "#cce6ff"),
            ("--blue-100", "#99ceff"),
            ("--blue-200", "#66b5ff"),
            ("--blue-300", "#339cff"),
            ("--blue-400", "#0285ff"),
            ("--blue-500", "#0169cc"),
            ("--blue-600", "#004f99"),
            ("--blue-700", "#003f7a"),
            ("--blue-800", "#013566"),
            ("--blue-900", "#00284d"),
            ("--blue-950", "#000e1a"),
            ("--blue-1000", "#000d19"),
            ("--blue-a25", "alpha(var(--blue-400), 4%)"),
            ("--blue-a50", "alpha(var(--blue-400), 13%)"),
            ("--blue-a75", "alpha(var(--blue-400), 25%)"),
            ("--blue-a100", "alpha(var(--blue-400), 40%)"),
            ("--blue-a200", "alpha(var(--blue-400), 60%)"),
            ("--blue-a300", "alpha(var(--blue-400), 80%)"),
        ],
    },
    Section {
        title: "Sizes",
        tokens: &[("--hairline", "1px")],
    },
    Section {
        title: "Shadows",
        tokens: &[
            ("--shadow-color", "0 0 0"),
            ("--elevation-100-geo", "0 1px 2px -1px"),
            ("--elevation-200-geo", "0 2px 4px -1px"),
            ("--elevation-300-geo", "0 4px 8px -2px"),
            ("--elevation-400-geo", "0 8px 16px -4px"),
        ],
    },
];

pub const SEMANTIC: &[Section] = &[
    Section {
        // Injected upstream by `postcss/injectBreakpoints.mjs` (DEFAULT_CONFIG).
        title: "Breakpoints",
        tokens: &[
            ("--breakpoint-xs", "380px"),
            ("--breakpoint-sm", "576px"),
            ("--breakpoint-md", "768px"),
            ("--breakpoint-lg", "1024px"),
            ("--breakpoint-xl", "1280px"),
            ("--breakpoint-2xl", "1536px"),
        ],
    },
    Section {
        // Upstream `tailwind-utilities.css` pointers that the type scale reads.
        // `--tracking-none` is Tailwind's default, read by component modules.
        title: "Font tracking",
        tokens: &[
            ("--tracking-wide", "var(--font-tracking-wide)"),
            ("--tracking-normal", "var(--font-tracking-normal)"),
            ("--tracking-tight", "var(--font-tracking-tight)"),
            ("--tracking-none", "0em"),
        ],
    },
    Section {
        title: "Font families",
        tokens: &[
            (
                "--font-sans",
                "ui-sans-serif, -apple-system, system-ui, \"Segoe UI\", \"Noto Sans\", \"Helvetica\", \"Arial\", \"Apple Color Emoji\", \"Segoe UI Emoji\", \"Segoe UI Symbol\", sans-serif",
            ),
            (
                "--font-mono",
                "ui-monospace, \"SFMono-Regular\", \"SF Mono\", \"Menlo\", \"Monaco\", \"Consolas\", \"Liberation Mono\", \"DejaVu Sans Mono\", \"Courier New\", monospace",
            ),
        ],
    },
    Section {
        title: "Font weights",
        tokens: &[
            ("--font-weight-normal", "400"),
            ("--font-weight-medium", "500"),
            ("--font-weight-semibold", "600"),
            ("--font-weight-bold", "700"),
        ],
    },
    Section {
        title: "Radius",
        tokens: &[
            ("--radius-2xs", "0.125rem"),
            ("--radius-xs", "0.25rem"),
            ("--radius-sm", "0.375rem"),
            ("--radius-md", "0.5rem"),
            ("--radius-lg", "0.625rem"),
            ("--radius-xl", "0.75rem"),
            ("--radius-2xl", "1rem"),
            ("--radius-3xl", "1.25rem"),
            ("--radius-4xl", "1.5rem"),
            ("--radius-full", "9999px"),
        ],
    },
    Section {
        title: "Spacing",
        tokens: &[("--spacing", "0.25rem")],
    },
    Section {
        title: "Text colors",
        tokens: &[
            ("--color-text", "var(--gray-1000)"),
            (
                "--color-text-secondary",
                "light-dark(var(--gray-500), var(--gray-700))",
            ),
            (
                "--color-text-tertiary",
                "light-dark(var(--gray-400), var(--gray-600))",
            ),
            ("--color-text-inverse", "var(--gray-0)"),
        ],
    },
    Section {
        title: "Ring colors",
        tokens: &[(
            "--color-ring",
            "light-dark(var(--blue-500), var(--blue-400))",
        )],
    },
    Section {
        title: "Primary colors",
        tokens: &[
            ("--color-text-primary", "var(--color-text)"),
            (
                "--color-background-primary-soft",
                "light-dark(var(--gray-100), var(--gray-300))",
            ),
            (
                "--color-background-primary-soft-hover",
                "light-dark(var(--gray-150), var(--gray-350))",
            ),
            (
                "--color-background-primary-soft-active",
                "light-dark(var(--gray-200), var(--gray-400))",
            ),
            (
                "--color-background-primary-soft-alpha",
                "light-dark(var(--alpha-08), var(--alpha-12))",
            ),
            (
                "--color-background-primary-soft-alpha-hover",
                "light-dark(var(--alpha-12), var(--alpha-16))",
            ),
            (
                "--color-background-primary-soft-alpha-active",
                "light-dark(var(--alpha-16), var(--alpha-20))",
            ),
            ("--color-text-primary-soft", "var(--color-text)"),
            ("--color-background-primary-soft-alt", "var(--alpha-02)"),
            ("--color-border-primary-soft-alt", "var(--alpha-06)"),
            ("--color-text-primary-soft-alt", "var(--color-text)"),
            (
                "--color-background-primary-surface",
                "light-dark(var(--alpha-05), var(--alpha-08))",
            ),
            (
                "--color-border-primary-surface",
                "light-dark(var(--alpha-05), var(--alpha-08))",
            ),
            ("--color-text-primary-surface", "var(--color-text)"),
            (
                "--color-background-primary-solid",
                "light-dark(var(--gray-900), var(--gray-950))",
            ),
            (
                "--color-background-primary-solid-hover",
                "light-dark(var(--gray-700), var(--gray-900))",
            ),
            (
                "--color-background-primary-solid-active",
                "light-dark(var(--gray-600), var(--gray-850))",
            ),
            ("--color-text-primary-solid", "var(--color-text-inverse)"),
            (
                "--color-background-primary-outline-hover",
                "light-dark(var(--alpha-02), var(--alpha-04))",
            ),
            (
                "--color-background-primary-outline-active",
                "light-dark(var(--alpha-04), var(--alpha-06))",
            ),
            (
                "--color-border-primary-outline",
                "light-dark(var(--alpha-16), var(--alpha-25))",
            ),
            (
                "--color-border-primary-outline-hover",
                "light-dark(var(--alpha-20), var(--alpha-30))",
            ),
            ("--color-text-primary-outline", "var(--color-text)"),
            ("--color-text-primary-outline-hover", "var(--color-text)"),
            (
                "--color-background-primary-ghost-hover",
                "light-dark(var(--alpha-08), var(--alpha-12))",
            ),
            (
                "--color-background-primary-ghost-active",
                "light-dark(var(--alpha-12), var(--alpha-16))",
            ),
            ("--color-text-primary-ghost", "var(--color-text)"),
            ("--color-text-primary-ghost-hover", "var(--color-text)"),
            ("--color-ring-primary", "var(--color-ring)"),
            ("--color-ring-primary-soft", "var(--color-ring-primary)"),
            ("--color-ring-primary-solid", "var(--color-ring-primary)"),
            ("--color-ring-primary-outline", "var(--color-ring-primary)"),
            ("--color-ring-primary-ghost", "var(--color-ring-primary)"),
        ],
    },
    Section {
        title: "Secondary colors",
        tokens: &[
            (
                "--color-background-secondary-soft",
                "light-dark(var(--gray-100), var(--gray-300))",
            ),
            (
                "--color-background-secondary-soft-hover",
                "light-dark(var(--gray-150), var(--gray-350))",
            ),
            (
                "--color-background-secondary-soft-active",
                "light-dark(var(--gray-200), var(--gray-400))",
            ),
            (
                "--color-background-secondary-soft-alpha",
                "light-dark(var(--alpha-08), var(--alpha-12))",
            ),
            (
                "--color-background-secondary-soft-alpha-hover",
                "light-dark(var(--alpha-12), var(--alpha-16))",
            ),
            (
                "--color-background-secondary-soft-alpha-active",
                "light-dark(var(--alpha-16), var(--alpha-20))",
            ),
            ("--color-text-secondary-soft", "var(--color-text)"),
            ("--color-background-secondary-soft-alt", "var(--alpha-02)"),
            ("--color-border-secondary-soft-alt", "var(--alpha-06)"),
            ("--color-text-secondary-soft-alt", "var(--color-text)"),
            (
                "--color-background-secondary-solid",
                "light-dark(var(--gray-500), var(--gray-400))",
            ),
            (
                "--color-background-secondary-solid-hover",
                "light-dark(var(--gray-600), var(--gray-450))",
            ),
            (
                "--color-background-secondary-solid-active",
                "light-dark(var(--gray-700), var(--gray-500))",
            ),
            ("--color-text-secondary-solid", "var(--white)"),
            (
                "--color-background-secondary-outline-hover",
                "light-dark(var(--alpha-02), var(--alpha-04))",
            ),
            (
                "--color-background-secondary-outline-active",
                "light-dark(var(--alpha-04), var(--alpha-06))",
            ),
            (
                "--color-border-secondary-outline",
                "light-dark(var(--alpha-16), var(--alpha-25))",
            ),
            (
                "--color-border-secondary-outline-hover",
                "light-dark(var(--alpha-20), var(--alpha-30))",
            ),
            (
                "--color-text-secondary-outline",
                "var(--color-text-secondary)",
            ),
            ("--color-text-secondary-outline-hover", "var(--color-text)"),
            (
                "--color-background-secondary-ghost-hover",
                "light-dark(var(--alpha-08), var(--alpha-12))",
            ),
            (
                "--color-background-secondary-ghost-active",
                "light-dark(var(--alpha-12), var(--alpha-16))",
            ),
            (
                "--color-text-secondary-ghost",
                "var(--color-text-secondary)",
            ),
            ("--color-text-secondary-ghost-hover", "var(--color-text)"),
            ("--color-ring-secondary", "var(--color-ring)"),
            ("--color-ring-secondary-soft", "var(--color-ring-secondary)"),
            (
                "--color-ring-secondary-solid",
                "var(--color-ring-secondary)",
            ),
            (
                "--color-ring-secondary-outline",
                "var(--color-ring-secondary)",
            ),
            (
                "--color-ring-secondary-ghost",
                "var(--color-ring-secondary)",
            ),
        ],
    },
    Section {
        title: "Info colors",
        tokens: &[
            (
                "--color-text-info",
                "light-dark(var(--blue-500), var(--blue-200))",
            ),
            ("--color-background-info-soft", "var(--blue-50)"),
            ("--color-background-info-soft-hover", "var(--blue-75)"),
            ("--color-background-info-soft-active", "var(--blue-75)"),
            ("--color-background-info-soft-alpha", "var(--blue-a50)"),
            (
                "--color-background-info-soft-alpha-hover",
                "var(--blue-a75)",
            ),
            (
                "--color-background-info-soft-alpha-active",
                "var(--blue-a75)",
            ),
            (
                "--color-text-info-soft",
                "light-dark(var(--blue-600), var(--blue-300))",
            ),
            (
                "--color-background-info-surface",
                "light-dark(var(--blue-a25), var(--blue-a50))",
            ),
            (
                "--color-border-info-surface",
                "light-dark(var(--blue-a25), var(--blue-a50))",
            ),
            (
                "--color-text-info-surface",
                "light-dark(var(--blue-600), var(--blue-300))",
            ),
            ("--color-background-info-solid", "var(--blue-400)"),
            ("--color-background-info-solid-hover", "var(--blue-500)"),
            ("--color-background-info-solid-active", "var(--blue-500)"),
            ("--color-text-info-solid", "var(--white)"),
            ("--color-background-info-outline-hover", "var(--blue-a25)"),
            ("--color-background-info-outline-active", "var(--blue-a25)"),
            ("--color-border-info-outline", "var(--blue-500)"),
            ("--color-border-info-outline-hover", "var(--blue-500)"),
            ("--color-text-info-outline", "var(--blue-500)"),
            ("--color-text-info-outline-hover", "var(--blue-500)"),
            ("--color-background-info-ghost-hover", "var(--blue-a50)"),
            ("--color-background-info-ghost-active", "var(--blue-a50)"),
            (
                "--color-text-info-ghost",
                "light-dark(var(--blue-500), var(--blue-200))",
            ),
            (
                "--color-text-info-ghost-hover",
                "light-dark(var(--blue-500), var(--blue-200))",
            ),
            ("--color-ring-info", "var(--color-ring)"),
            ("--color-ring-info-soft", "var(--color-ring-info)"),
            ("--color-ring-info-solid", "var(--color-ring-info)"),
            ("--color-ring-info-outline", "var(--color-ring-info)"),
            ("--color-ring-info-ghost", "var(--color-ring-info)"),
        ],
    },
    Section {
        title: "Warning colors",
        tokens: &[
            (
                "--color-text-warning",
                "light-dark(var(--orange-700), var(--orange-500))",
            ),
            ("--color-background-warning-soft", "var(--orange-50)"),
            ("--color-background-warning-soft-hover", "var(--orange-75)"),
            ("--color-background-warning-soft-active", "var(--orange-75)"),
            ("--color-background-warning-soft-alpha", "var(--orange-a50)"),
            (
                "--color-background-warning-soft-alpha-hover",
                "var(--orange-a75)",
            ),
            (
                "--color-background-warning-soft-alpha-active",
                "var(--orange-a75)",
            ),
            (
                "--color-text-warning-soft",
                "light-dark(var(--orange-700), var(--orange-400))",
            ),
            (
                "--color-background-warning-surface",
                "light-dark(var(--orange-a25), var(--orange-a50))",
            ),
            (
                "--color-border-warning-surface",
                "light-dark(var(--orange-a25), var(--orange-a50))",
            ),
            (
                "--color-text-warning-surface",
                "light-dark(var(--orange-700), var(--orange-400))",
            ),
            ("--color-background-warning-solid", "var(--orange-500)"),
            (
                "--color-background-warning-solid-hover",
                "var(--orange-600)",
            ),
            (
                "--color-background-warning-solid-active",
                "var(--orange-600)",
            ),
            ("--color-text-warning-solid", "var(--white)"),
            (
                "--color-background-warning-outline-hover",
                "var(--orange-a25)",
            ),
            (
                "--color-background-warning-outline-active",
                "var(--orange-a25)",
            ),
            ("--color-border-warning-outline", "var(--orange-500)"),
            ("--color-border-warning-outline-hover", "var(--orange-500)"),
            ("--color-text-warning-outline", "var(--orange-500)"),
            ("--color-text-warning-outline-hover", "var(--orange-500)"),
            (
                "--color-background-warning-ghost-hover",
                "var(--orange-a50)",
            ),
            (
                "--color-background-warning-ghost-active",
                "var(--orange-a50)",
            ),
            ("--color-text-warning-ghost", "var(--orange-500)"),
            ("--color-text-warning-ghost-hover", "var(--orange-500)"),
            ("--color-ring-warning", "var(--color-ring)"),
            ("--color-ring-warning-soft", "var(--color-ring-warning)"),
            ("--color-ring-warning-solid", "var(--color-ring-warning)"),
            ("--color-ring-warning-outline", "var(--color-ring-warning)"),
            ("--color-ring-warning-ghost", "var(--color-ring-warning)"),
        ],
    },
    Section {
        title: "Caution colors",
        tokens: &[
            // openagents: WCAG AA (upstream light-dark(var(--yellow-700), var(--yellow-500)))
            (
                "--color-text-caution",
                "light-dark(var(--yellow-800), var(--yellow-500))",
            ),
            ("--color-text-caution-hover", "var(--yellow-800)"),
            ("--color-background-caution-soft", "var(--yellow-50)"),
            ("--color-background-caution-soft-hover", "var(--yellow-75)"),
            ("--color-background-caution-soft-active", "var(--yellow-75)"),
            ("--color-background-caution-soft-alpha", "var(--yellow-a50)"),
            (
                "--color-background-caution-soft-alpha-hover",
                "var(--yellow-a75)",
            ),
            (
                "--color-background-caution-soft-alpha-active",
                "var(--yellow-a75)",
            ),
            (
                "--color-text-caution-soft",
                "light-dark(var(--yellow-800), var(--yellow-400))",
            ),
            (
                "--color-background-caution-surface",
                "light-dark(var(--yellow-a25), var(--yellow-a50))",
            ),
            (
                "--color-border-caution-surface",
                "light-dark(var(--yellow-a25), var(--yellow-a50))",
            ),
            (
                "--color-text-caution-surface",
                "light-dark(var(--yellow-800), var(--yellow-400))",
            ),
            ("--color-background-caution-solid", "var(--yellow-600)"),
            (
                "--color-background-caution-solid-hover",
                "var(--yellow-700)",
            ),
            (
                "--color-background-caution-solid-active",
                "var(--yellow-700)",
            ),
            ("--color-text-caution-solid", "var(--white)"),
            (
                "--color-background-caution-outline-hover",
                "var(--yellow-a25)",
            ),
            (
                "--color-background-caution-outline-active",
                "var(--yellow-a25)",
            ),
            ("--color-border-caution-outline", "var(--yellow-700)"),
            ("--color-border-caution-outline-hover", "var(--yellow-700)"),
            ("--color-text-caution-outline", "var(--yellow-700)"),
            ("--color-text-caution-outline-hover", "var(--yellow-700)"),
            (
                "--color-background-caution-ghost-hover",
                "var(--yellow-a50)",
            ),
            (
                "--color-background-caution-ghost-active",
                "var(--yellow-a50)",
            ),
            ("--color-text-caution-ghost", "var(--yellow-700)"),
            ("--color-text-caution-ghost-hover", "var(--yellow-700)"),
            ("--color-ring-caution", "var(--color-ring)"),
            ("--color-ring-caution-soft", "var(--color-ring-caution)"),
            ("--color-ring-caution-solid", "var(--color-ring-caution)"),
            ("--color-ring-caution-outline", "var(--color-ring-caution)"),
            ("--color-ring-caution-ghost", "var(--color-ring-caution)"),
        ],
    },
    Section {
        title: "Danger colors",
        tokens: &[
            // openagents: WCAG AA (upstream light-dark(var(--red-700), var(--red-500)))
            (
                "--color-text-danger",
                "light-dark(var(--red-700), var(--red-300))",
            ),
            ("--color-background-danger-soft", "var(--red-50)"),
            ("--color-background-danger-soft-hover", "var(--red-75)"),
            ("--color-background-danger-soft-active", "var(--red-75)"),
            ("--color-background-danger-soft-alpha", "var(--red-a50)"),
            (
                "--color-background-danger-soft-alpha-hover",
                "var(--red-a75)",
            ),
            (
                "--color-background-danger-soft-alpha-active",
                "var(--red-a75)",
            ),
            // openagents: WCAG AA (upstream light-dark(var(--red-600), var(--red-400)))
            (
                "--color-text-danger-soft",
                "light-dark(var(--red-600), var(--red-300))",
            ),
            (
                "--color-background-danger-surface",
                "light-dark(var(--red-a25), var(--red-a50))",
            ),
            (
                "--color-border-danger-surface",
                "light-dark(var(--red-a25), var(--red-a50))",
            ),
            // openagents: WCAG AA (upstream light-dark(var(--red-600), var(--red-400)))
            (
                "--color-text-danger-surface",
                "light-dark(var(--red-600), var(--red-300))",
            ),
            ("--color-background-danger-solid", "var(--red-500)"),
            ("--color-background-danger-solid-hover", "var(--red-600)"),
            ("--color-background-danger-solid-active", "var(--red-600)"),
            ("--color-text-danger-solid", "var(--white)"),
            ("--color-background-danger-outline-hover", "var(--red-a25)"),
            ("--color-background-danger-outline-active", "var(--red-a25)"),
            ("--color-border-danger-outline", "var(--red-500)"),
            ("--color-border-danger-outline-hover", "var(--red-500)"),
            ("--color-text-danger-outline", "var(--red-500)"),
            ("--color-text-danger-outline-hover", "var(--red-500)"),
            ("--color-background-danger-ghost-hover", "var(--red-a50)"),
            ("--color-background-danger-ghost-active", "var(--red-a50)"),
            ("--color-text-danger-ghost", "var(--red-500)"),
            ("--color-text-danger-ghost-hover", "var(--red-500)"),
            ("--color-ring-danger", "var(--red-200)"),
            ("--color-ring-danger-soft", "var(--color-ring-danger)"),
            ("--color-ring-danger-solid", "var(--color-ring-danger)"),
            ("--color-ring-danger-outline", "var(--color-ring-danger)"),
            ("--color-ring-danger-ghost", "var(--color-ring-danger)"),
        ],
    },
    Section {
        title: "Success colors",
        tokens: &[
            (
                "--color-text-success",
                "light-dark(var(--green-700), var(--green-400))",
            ),
            ("--color-background-success-soft", "var(--green-50)"),
            ("--color-background-success-soft-hover", "var(--green-75)"),
            ("--color-background-success-soft-active", "var(--green-75)"),
            ("--color-background-success-soft-alpha", "var(--green-a50)"),
            (
                "--color-background-success-soft-alpha-hover",
                "var(--green-a75)",
            ),
            (
                "--color-background-success-soft-alpha-active",
                "var(--green-a75)",
            ),
            // openagents: WCAG AA (upstream light-dark(var(--green-600), var(--green-400)))
            (
                "--color-text-success-soft",
                "light-dark(var(--green-700), var(--green-400))",
            ),
            (
                "--color-background-success-surface",
                "light-dark(var(--green-a25), var(--green-a50))",
            ),
            (
                "--color-border-success-surface",
                "light-dark(var(--green-a25), var(--green-a50))",
            ),
            // openagents: WCAG AA (upstream light-dark(var(--green-600), var(--green-400)))
            (
                "--color-text-success-surface",
                "light-dark(var(--green-700), var(--green-400))",
            ),
            (
                "--color-background-success-solid",
                "light-dark(var(--green-500), var(--green-600))",
            ),
            (
                "--color-background-success-solid-hover",
                "light-dark(var(--green-500), var(--green-600))",
            ),
            (
                "--color-background-success-solid-active",
                "light-dark(var(--green-500), var(--green-600))",
            ),
            ("--color-text-success-solid", "var(--white)"),
            (
                "--color-background-success-outline-hover",
                "var(--green-a25)",
            ),
            (
                "--color-background-success-outline-active",
                "var(--green-a25)",
            ),
            (
                "--color-border-success-outline",
                "light-dark(var(--green-500), var(--green-600))",
            ),
            (
                "--color-border-success-outline-hover",
                "light-dark(var(--green-500), var(--green-600))",
            ),
            ("--color-text-success-outline", "var(--green-500)"),
            ("--color-text-success-outline-hover", "var(--green-500)"),
            ("--color-background-success-ghost-hover", "var(--green-a50)"),
            (
                "--color-background-success-ghost-active",
                "var(--green-a50)",
            ),
            ("--color-text-success-ghost", "var(--green-500)"),
            ("--color-text-success-ghost-hover", "var(--green-500)"),
            ("--color-ring-success", "var(--color-ring)"),
            ("--color-ring-success-soft", "var(--color-ring-info)"),
            ("--color-ring-success-solid", "var(--color-ring-info)"),
            ("--color-ring-success-outline", "var(--color-ring-info)"),
            ("--color-ring-success-ghost", "var(--color-ring-info)"),
        ],
    },
    Section {
        title: "Discovery colors",
        tokens: &[
            // openagents: WCAG AA (upstream light-dark(var(--purple-700), var(--purple-500)))
            (
                "--color-text-discovery",
                "light-dark(var(--purple-700), var(--purple-300))",
            ),
            ("--color-background-discovery-soft", "var(--purple-50)"),
            (
                "--color-background-discovery-soft-hover",
                "var(--purple-75)",
            ),
            (
                "--color-background-discovery-soft-active",
                "var(--purple-75)",
            ),
            (
                "--color-background-discovery-soft-alpha",
                "var(--purple-a50)",
            ),
            (
                "--color-background-discovery-soft-alpha-hover",
                "var(--purple-a75)",
            ),
            (
                "--color-background-discovery-soft-alpha-active",
                "var(--purple-a75)",
            ),
            (
                "--color-text-discovery-soft",
                "light-dark(var(--purple-600), var(--purple-200))",
            ),
            (
                "--color-background-discovery-surface",
                "light-dark(var(--purple-a25), var(--purple-a50))",
            ),
            (
                "--color-border-discovery-surface",
                "light-dark(var(--purple-a25), var(--purple-a50))",
            ),
            (
                "--color-text-discovery-surface",
                "light-dark(var(--purple-600), var(--purple-200))",
            ),
            ("--color-background-discovery-solid", "var(--purple-400)"),
            (
                "--color-background-discovery-solid-hover",
                "var(--purple-500)",
            ),
            (
                "--color-background-discovery-solid-active",
                "var(--purple-500)",
            ),
            ("--color-text-discovery-solid", "var(--white)"),
            (
                "--color-background-discovery-outline-hover",
                "var(--purple-a25)",
            ),
            (
                "--color-background-discovery-outline-active",
                "var(--purple-a25)",
            ),
            ("--color-border-discovery-outline", "var(--purple-500)"),
            (
                "--color-border-discovery-outline-hover",
                "var(--purple-500)",
            ),
            (
                "--color-text-discovery-outline",
                "light-dark(var(--purple-500), var(--purple-400))",
            ),
            (
                "--color-text-discovery-outline-hover",
                "light-dark(var(--purple-500), var(--purple-400))",
            ),
            (
                "--color-background-discovery-ghost-hover",
                "var(--purple-a50)",
            ),
            (
                "--color-background-discovery-ghost-active",
                "var(--purple-a50)",
            ),
            ("--color-text-discovery-ghost", "var(--purple-500)"),
            ("--color-text-discovery-ghost-hover", "var(--purple-500)"),
            ("--color-ring-discovery", "var(--color-ring)"),
            ("--color-ring-discovery-soft", "var(--color-ring)"),
            ("--color-ring-discovery-solid", "var(--color-ring)"),
            ("--color-ring-discovery-outline", "var(--color-ring)"),
            ("--color-ring-discovery-ghost", "var(--color-ring)"),
        ],
    },
    Section {
        title: "Disabled colors",
        tokens: &[
            ("--color-background-disabled", "var(--alpha-05)"),
            ("--color-border-disabled", "var(--alpha-06)"),
            (
                "--color-text-disabled",
                "light-dark(var(--gray-400), var(--gray-500))",
            ),
        ],
    },
    Section {
        title: "Border colors",
        tokens: &[
            (
                "--color-border-subtle",
                "light-dark(var(--alpha-05), var(--alpha-06))",
            ),
            (
                "--color-border",
                "light-dark(var(--alpha-10), var(--alpha-12))",
            ),
            (
                "--color-border-strong",
                "light-dark(var(--alpha-15), var(--alpha-20))",
            ),
        ],
    },
    Section {
        title: "Typography",
        tokens: &[
            ("--font-tracking-wide", "0em"),
            ("--font-tracking-normal", "0em"),
            ("--font-tracking-tight", "0em"),
            ("--font-heading-5xl-size", "4.5rem"),
            ("--font-heading-5xl-line-height", "4.5rem"),
            ("--font-heading-5xl-weight", "var(--font-weight-semibold)"),
            ("--font-heading-5xl-tracking", "var(--tracking-tight)"),
            ("--font-heading-4xl-size", "3.75rem"),
            ("--font-heading-4xl-line-height", "3.75rem"),
            ("--font-heading-4xl-weight", "var(--font-weight-semibold)"),
            ("--font-heading-4xl-tracking", "var(--tracking-tight)"),
            ("--font-heading-3xl-size", "3rem"),
            ("--font-heading-3xl-line-height", "3rem"),
            ("--font-heading-3xl-weight", "var(--font-weight-semibold)"),
            ("--font-heading-3xl-tracking", "var(--tracking-tight)"),
            ("--font-heading-2xl-size", "2.25rem"),
            ("--font-heading-2xl-line-height", "2.625rem"),
            ("--font-heading-2xl-weight", "var(--font-weight-semibold)"),
            ("--font-heading-2xl-tracking", "var(--tracking-tight)"),
            ("--font-heading-xl-size", "2rem"),
            ("--font-heading-xl-line-height", "2.375rem"),
            ("--font-heading-xl-weight", "var(--font-weight-semibold)"),
            ("--font-heading-xl-tracking", "var(--tracking-tight)"),
            ("--font-heading-lg-size", "1.5rem"),
            ("--font-heading-lg-line-height", "1.75rem"),
            ("--font-heading-lg-weight", "var(--font-weight-semibold)"),
            ("--font-heading-lg-tracking", "var(--tracking-normal)"),
            ("--font-heading-md-size", "1.25rem"),
            ("--font-heading-md-line-height", "1.625rem"),
            ("--font-heading-md-weight", "var(--font-weight-semibold)"),
            ("--font-heading-md-tracking", "var(--tracking-normal)"),
            ("--font-heading-sm-size", "1.125rem"),
            ("--font-heading-sm-line-height", "1.625rem"),
            ("--font-heading-sm-weight", "var(--font-weight-semibold)"),
            ("--font-heading-sm-tracking", "var(--tracking-normal)"),
            ("--font-heading-xs-size", "1rem"),
            ("--font-heading-xs-line-height", "1.5rem"),
            ("--font-heading-xs-weight", "var(--font-weight-semibold)"),
            ("--font-heading-xs-tracking", "var(--tracking-normal)"),
            ("--font-text-lg-size", "1.125rem"),
            ("--font-text-lg-line-height", "1.8125rem"),
            ("--font-text-lg-weight", "var(--font-weight-normal)"),
            ("--font-text-lg-tracking", "var(--tracking-normal)"),
            ("--font-text-md-size", "1rem"),
            ("--font-text-md-line-height", "1.5rem"),
            ("--font-text-md-weight", "var(--font-weight-normal)"),
            ("--font-text-md-tracking", "var(--tracking-normal)"),
            ("--font-text-sm-size", "0.875rem"),
            ("--font-text-sm-line-height", "1.25rem"),
            ("--font-text-sm-weight", "var(--font-weight-normal)"),
            ("--font-text-sm-tracking", "var(--tracking-normal)"),
            ("--font-text-xs-size", "0.75rem"),
            ("--font-text-xs-line-height", "1.125rem"),
            ("--font-text-xs-weight", "var(--font-weight-normal)"),
            ("--font-text-xs-tracking", "var(--tracking-wide)"),
            ("--font-text-2xs-size", "0.625rem"),
            ("--font-text-2xs-line-height", "0.875rem"),
            ("--font-text-2xs-weight", "var(--font-weight-normal)"),
            ("--font-text-2xs-tracking", "var(--tracking-wide)"),
            ("--font-text-3xs-size", "0.5rem"),
            ("--font-text-3xs-line-height", "0.75rem"),
            ("--font-text-3xs-weight", "var(--font-weight-normal)"),
            ("--font-text-3xs-tracking", "var(--tracking-wide)"),
        ],
    },
    Section {
        title: "Control sizes",
        tokens: &[
            ("--control-size-3xs", "1.375rem"),
            ("--control-size-2xs", "1.5rem"),
            ("--control-size-xs", "1.625rem"),
            ("--control-size-sm", "1.75rem"),
            ("--control-size-md", "2rem"),
            ("--control-size-lg", "2.25rem"),
            ("--control-size-xl", "2.5rem"),
            ("--control-size-2xl", "2.75rem"),
            ("--control-size-3xl", "3rem"),
            ("--control-gutter-2xs", "0.375rem"),
            ("--control-gutter-xs", "0.5rem"),
            ("--control-gutter-sm", "0.625rem"),
            ("--control-gutter-md", "0.75rem"),
            ("--control-gutter-lg", "0.875rem"),
            ("--control-gutter-xl", "1rem"),
            ("--control-gutter-pill-scaling", "1.33"),
            ("--control-radius-sm", "var(--radius-sm)"),
            ("--control-radius-md", "var(--radius-md)"),
            ("--control-radius-lg", "var(--radius-lg)"),
            ("--control-radius-xl", "var(--radius-xl)"),
            ("--control-font-size-sm", "var(--font-text-xs-size)"),
            ("--control-font-size-md", "var(--font-text-sm-size)"),
            ("--control-font-size-lg", "var(--font-text-md-size)"),
            ("--control-icon-size-xs", "0.875rem"),
            ("--control-icon-size-sm", "1rem"),
            ("--control-icon-size-md", "1.125rem"),
            ("--control-icon-size-lg", "1.25rem"),
            ("--control-icon-size-xl", "1.375rem"),
            ("--control-icon-size-2xl", "1.5rem"),
        ],
    },
    Section {
        title: "Motion",
        tokens: &[
            ("--cubic-enter", "cubic-bezier(0.19, 1, 0.22, 1)"),
            ("--cubic-exit", "cubic-bezier(0.8, 0, 0.4, 1)"),
            ("--cubic-exit-snappy", "cubic-bezier(0.65, 0, 0.4, 1)"),
            ("--cubic-move", "cubic-bezier(0.65, 0, 0.35, 1)"),
            ("--transition-duration-basic", "150ms"),
            ("--transition-ease-basic", "ease"),
        ],
    },
    Section {
        title: "Scrollbar",
        tokens: &[("--scrollbar-color", "var(--alpha-30)")],
    },
    Section {
        title: "Shadows",
        tokens: &[
            (
                "--shadow",
                "0 10px 15px -3px light-dark(rgba(0 0 0 / 10%), rgba(0 0 0 / 20%)), 0 4px 6px -4px light-dark(rgba(0 0 0 / 10%), rgba(0 0 0 / 20%))",
            ),
            (
                "--shadow-hairline",
                "0 0 0 var(--shadow-hairline-width) var(--shadow-hairline-color)",
            ),
            (
                "--shadow-100",
                "var(--elevation-100-geo) rgb(var(--shadow-color) / var(--shadow-alpha-100))",
            ),
            (
                "--shadow-100-strong",
                "var(--elevation-100-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-100) * 1.25))",
            ),
            (
                "--shadow-100-stronger",
                "var(--elevation-100-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-100) * 1.6))",
            ),
            (
                "--shadow-200",
                "var(--elevation-200-geo) rgb(var(--shadow-color) / var(--shadow-alpha-200))",
            ),
            (
                "--shadow-200-strong",
                "var(--elevation-200-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-200) * 1.25))",
            ),
            (
                "--shadow-200-stronger",
                "var(--elevation-200-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-200) * 1.6))",
            ),
            (
                "--shadow-300",
                "var(--elevation-300-geo) rgb(var(--shadow-color) / var(--shadow-alpha-300))",
            ),
            (
                "--shadow-300-strong",
                "var(--elevation-300-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-300) * 1.25))",
            ),
            (
                "--shadow-300-stronger",
                "var(--elevation-300-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-300) * 1.6))",
            ),
            (
                "--shadow-400",
                "var(--elevation-400-geo) rgb(var(--shadow-color) / var(--shadow-alpha-400))",
            ),
            (
                "--shadow-400-strong",
                "var(--elevation-400-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-400) * 1.25))",
            ),
            (
                "--shadow-400-stronger",
                "var(--elevation-400-geo) rgb(var(--shadow-color) / calc(var(--shadow-alpha-400) * 1.6))",
            ),
        ],
    },
    Section {
        title: "Surfaces",
        tokens: &[
            (
                "--color-surface",
                "light-dark(var(--gray-0), var(--gray-200))",
            ),
            (
                "--color-surface-secondary",
                "light-dark(var(--gray-50), var(--gray-100))",
            ),
            (
                "--color-surface-tertiary",
                "light-dark(var(--gray-75), var(--gray-50))",
            ),
            (
                "--color-surface-elevated",
                "light-dark(var(--gray-0), var(--gray-300))",
            ),
            (
                "--color-surface-elevated-secondary",
                "light-dark(var(--gray-50), var(--gray-400))",
            ),
        ],
    },
];

pub const COMPONENTS: &[Section] = &[
    Section {
        title: "Alert",
        tokens: &[
            ("--alert-border-radius", "var(--radius-xl)"),
            ("--alert-gap", "spacing(3)"),
            ("--alert-gutter", "spacing(4)"),
            ("--alert-font-size", "var(--font-text-sm-size)"),
            ("--alert-line-height", "var(--font-text-sm-line-height)"),
            ("--alert-title-font-weight", "var(--font-weight-semibold)"),
        ],
    },
    Section {
        title: "Avatar",
        tokens: &[
            ("--avatar-radius", "var(--radius-full)"),
            ("--avatar-size", "28px"),
            ("--avatar-font-size-scaling", "0.5"),
            ("--avatar-overflow-font-size-scaling-one", "0.45"),
            ("--avatar-overflow-font-size-scaling-two", "0.37"),
            ("--avatar-overflow-font-size-scaling-three", "0.3"),
            (
                "--avatar-image-border-color",
                "light-dark(var(--alpha-04), var(--alpha-15))",
            ),
        ],
    },
    Section {
        title: "AvatarGroup",
        tokens: &[
            ("--avatar-group-cutout-width", "3px"),
            ("--avatar-group-cutout-color", "var(--color-surface)"),
            ("--avatar-group-spacing", "-8px"),
        ],
    },
    Section {
        title: "Badge",
        tokens: &[
            ("--badge-gutter-sm", "calc(var(--control-gutter-2xs) - 1px)"),
            ("--badge-gutter-md", "var(--control-gutter-2xs)"),
            ("--badge-gutter-lg", "var(--control-gutter-xs)"),
            ("--badge-size-sm", "calc(var(--control-size-3xs) - 2px)"),
            ("--badge-size-md", "var(--control-size-3xs)"),
            ("--badge-size-lg", "var(--control-size-2xs)"),
            ("--badge-radius-sm", "var(--radius-xs)"),
            ("--badge-radius-md", "var(--radius-xs)"),
            ("--badge-radius-lg", "var(--radius-sm)"),
            ("--badge-font-size-sm", "var(--font-text-xs-size)"),
            ("--badge-font-size-md", "var(--font-text-sm-size)"),
            ("--badge-font-size-lg", "var(--font-text-sm-size)"),
            ("--badge-tracking-sm", "var(--tracking-wide)"),
            ("--badge-tracking-md", "var(--tracking-normal)"),
            ("--badge-tracking-lg", "var(--tracking-normal)"),
            ("--badge-font-weight-sm", "var(--font-weight-semibold)"),
            ("--badge-font-weight-md", "var(--font-weight-semibold)"),
            ("--badge-font-weight-lg", "var(--font-weight-semibold)"),
            ("--badge-icon-font-size-sm", "var(--font-text-xs-size)"),
            ("--badge-icon-font-size-md", "var(--font-text-md-size)"),
            ("--badge-icon-font-size-lg", "var(--font-text-md-size)"),
            ("--badge-indicator-size-sm", "var(--font-text-xs-size)"),
            ("--badge-indicator-size-md", "var(--font-text-xs-size)"),
            ("--badge-indicator-size-lg", "var(--font-text-sm-size)"),
        ],
    },
    Section {
        title: "Button",
        tokens: &[
            ("--button-gap-sm", "3px"),
            ("--button-gap-md", "4px"),
            ("--button-gap-lg", "6px"),
            ("--button-font-weight", "var(--font-weight-medium)"),
        ],
    },
    Section {
        title: "Input",
        tokens: &[
            ("--input-gap-xs", "4px"),
            ("--input-gap-sm", "6px"),
            ("--input-gap-md", "8px"),
            ("--input-gap-lg", "10px"),
            ("--input-text-color", "var(--color-text)"),
            (
                "--input-placeholder-text-color",
                "var(--color-text-tertiary)",
            ),
            (
                "--input-outline-border-color",
                "var(--color-border-primary-outline)",
            ),
            (
                "--input-outline-border-color-hover",
                "light-dark(var(--alpha-25), var(--alpha-30))",
            ),
            ("--input-outline-border-color-focus", "var(--alpha-50)"),
            (
                "--input-soft-background-color",
                "var(--color-background-primary-soft-alpha)",
            ),
            ("--input-soft-border-color-focus", "var(--alpha-20)"),
            (
                "--input-border-color-invalid",
                "light-dark(var(--red-500), var(--red-600))",
            ),
        ],
    },
    Section {
        title: "Link",
        tokens: &[
            ("--link-font-weight", "inherit"),
            ("--link-gap", "spacing(0.5)"),
            ("--link-radius", "var(--radius-sm)"),
            ("--link-underline-decoration-offset", "0.1em"),
            (
                "--link-primary-text-color",
                "light-dark(var(--blue-500), var(--blue-300))",
            ),
            (
                "--link-primary-text-color-hover",
                "light-dark(var(--blue-800), var(--blue-400))",
            ),
        ],
    },
    Section {
        title: "Chat",
        tokens: &[
            ("--chat-max-width", "800px"),
            ("--chat-gutter", "spacing(5)"),
            ("--chat-background-color", "var(--color-surface)"),
            ("--thread-gutter", "spacing(4)"),
            ("--composer-gutter", "spacing(3)"),
            ("--composer-compact-gutter", "spacing(2)"),
            ("--composer-radius", "var(--radius-4xl)"),
            (
                "--composer-background-color",
                "var(--color-surface-elevated)",
            ),
            ("--smoothing-background-color", "var(--color-surface)"),
            (
                "--user-message-background-color",
                "light-dark(var(--alpha-05), var(--alpha-08))",
            ),
            ("--user-message-text-color", "var(--color-text)"),
            ("--source-list-gutter", "var(--thread-gutter)"),
        ],
    },
    Section {
        title: "CodeBlock",
        tokens: &[
            ("--codeblock-background-color", "var(--gray-25)"),
            (
                "--codeblock-syntax-1",
                "light-dark(#c0660d, var(--yellow-100))",
            ),
            (
                "--codeblock-syntax-2",
                "light-dark(var(--blue-500), var(--blue-200))",
            ),
            (
                "--codeblock-syntax-3",
                "light-dark(var(--green-600), var(--green-300))",
            ),
            ("--codeblock-syntax-4", "var(--pink-500)"),
            (
                "--codeblock-syntax-5",
                "light-dark(var(--purple-500), var(--purple-300))",
            ),
        ],
    },
    Section {
        title: "Dialog",
        tokens: &[
            ("--dialog-min-width", "250px"),
            ("--dialog-max-width", "450px"),
            ("--dialog-container-inner-padding", "spacing(5)"),
            (
                "--dialog-backdrop-dim-background",
                "light-dark(rgb(0 0 0 / 30%), rgb(0 0 0 / 50%))",
            ),
            (
                "--dialog-backdrop-fade-background",
                "alpha(var(--color-surface-elevated), 60%)",
            ),
        ],
    },
    Section {
        title: "Menu",
        tokens: &[
            ("--menu-gutter", "spacing(1.5)"),
            ("--menu-radius", "var(--radius-xl)"),
            ("--menu-font-size", "var(--font-text-sm-size)"),
            ("--menu-line-height", "var(--font-text-sm-line-height)"),
            (
                "--menu-item-background-color",
                "light-dark(var(--alpha-08), var(--alpha-10))",
            ),
            ("--menu-item-padding", "spacing(1.5) spacing(2)"),
            ("--menu-item-gap", "spacing(1.5)"),
            (
                "--menu-separator-gutter",
                "var(--menu-gutter) calc(-1 * var(--menu-gutter))",
            ),
            ("--menu-separator-background-color", "var(--color-border)"),
            ("--menu-radio-indicator-size", "var(--font-text-lg-size)"),
            (
                "--menu-radio-indicator-hole-size",
                "var(--font-text-3xs-size)",
            ),
            ("--menu-checkbox-indicator-size", "var(--font-text-lg-size)"),
        ],
    },
    Section {
        title: "Modal",
        tokens: &[
            (
                "--modal-backdrop-background",
                "light-dark(rgb(0 0 0 / 30%), rgb(0 0 0 / 50%))",
            ),
            ("--modal-container-inner-padding", "spacing(5)"),
        ],
    },
    Section {
        title: "Popover",
        tokens: &[("--popover-radius", "var(--radius-xl)")],
    },
    Section {
        title: "RadioGroup",
        tokens: &[
            ("--radio-group-col-gap", "spacing(2.5)"),
            ("--radio-group-row-gap", "spacing(5)"),
            ("--radio-group-item-gap", "spacing(1.5)"),
            ("--radio-group-item-font-size", "var(--font-text-sm-size)"),
            (
                "--radio-group-item-line-height",
                "var(--font-text-sm-line-height)",
            ),
            ("--radio-group-indicator-size", "var(--font-text-md-size)"),
            (
                "--radio-group-indicator-border-color",
                "var(--color-border-primary-outline)",
            ),
            (
                "--radio-group-indicator-border-color-hover",
                "var(--alpha-25)",
            ),
            (
                "--radio-group-indicator-background-color",
                "var(--color-background-primary-solid)",
            ),
            ("--radio-group-indicator-hole-size", "0.375rem"),
            (
                "--radio-group-indicator-hole-background-color",
                "var(--color-text-primary-solid)",
            ),
        ],
    },
    Section {
        title: "Segmented Control",
        tokens: &[
            ("--segmented-control-gap", "2px"),
            ("--segmented-control-gutter", "2px"),
            (
                "--segmented-control-background",
                "light-dark(var(--gray-100), var(--gray-0))",
            ),
            (
                "--segmented-control-font-weight",
                "var(--font-weight-semibold)",
            ),
            (
                "--segmented-control-thumb-background",
                "light-dark(var(--gray-0), var(--gray-300))",
            ),
            (
                "--segmented-control-thumb-shadow",
                "0 1px 4px -1px rgb(0 0 0 / 20%)",
            ),
            ("--segmented-control-option-highlight-gutter", "1px"),
            (
                "--segmented-control-option-highlight-background-color",
                "light-dark(var(--gray-200), var(--gray-300))",
            ),
        ],
    },
    Section {
        title: "SelectControl",
        tokens: &[("--select-control-font-weight", "var(--font-weight-medium)")],
    },
    Section {
        title: "Slider",
        tokens: &[
            (
                "--slider-track-color",
                "light-dark(var(--gray-150), var(--gray-400))",
            ),
            (
                "--slider-range-color",
                "light-dark(var(--gray-450), var(--gray-600))",
            ),
        ],
    },
    Section {
        title: "Switch",
        tokens: &[
            ("--switch-track-width", "32px"),
            ("--switch-track-height", "19px"),
            (
                "--switch-track-color",
                "light-dark(var(--gray-150), var(--gray-400))",
            ),
            (
                "--switch-track-color-hover",
                "light-dark(var(--gray-200), var(--gray-450))",
            ),
            (
                "--switch-track-color-checked",
                "light-dark(var(--gray-900), var(--blue-400))",
            ),
            (
                "--switch-track-color-checked-disabled",
                "light-dark(var(--gray-300), var(--blue-700))",
            ),
            (
                "--switch-track-color-disabled",
                "light-dark(var(--gray-100), var(--gray-300))",
            ),
            ("--switch-thumb-offset", "3px"),
            (
                "--switch-thumb-size",
                "calc(var(--switch-track-height) - 2 * var(--switch-thumb-offset))",
            ),
            ("--switch-thumb-shadow", "0 1px 2px rgb(0 0 0 / 20%)"),
            (
                "--switch-thumb-color",
                "light-dark(var(--gray-0), var(--gray-1000))",
            ),
            (
                "--switch-thumb-color-disabled",
                "light-dark(var(--gray-0), var(--gray-800))",
            ),
            ("--switch-label-gap", "spacing(2)"),
        ],
    },
];
