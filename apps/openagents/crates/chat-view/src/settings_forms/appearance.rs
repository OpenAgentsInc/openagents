//! Settings → Appearance (#12289): the color scheme, the light and dark
//! themes, the accent, glass, the new-thread background image, motion, the
//! conversation width, and the theme library, with the desktop's words
//! (`openagents-ui`'s `settings::appearance`). The palette comes from the
//! same registry and the same `Theme::for_selection` the desktop installs.

use openagents_theme::vscode::{CompileOptions, compile_text};
use openagents_theme::{
    AccentPreset, AccentSelection, Appearance as Scheme, SurfacePreference, ThemeFamily,
    ThemeRegistry, ThemeSelection,
};
use rust_native::style::Style;
use rust_native::{Axis, Element, Node};
use serde::{Deserialize, Serialize};

use crate::motion::ReduceMotion;

use super::{
    Intent, Paints, Remote, button, danger_button, header, node, row, section, segmented, text,
    toggle, wide_row,
};

/// Light, dark, or as the system says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    System,
    Light,
    Dark,
}

impl Mode {
    pub const ALL: [Mode; 3] = [Mode::System, Mode::Light, Mode::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Mode::System => "System",
            Mode::Light => "Light",
            Mode::Dark => "Dark",
        }
    }

    /// The scheme drawn, given whether the system prefers dark.
    pub fn resolve(self, system_dark: bool) -> Scheme {
        match self {
            Mode::Light => Scheme::Light,
            Mode::Dark => Scheme::Dark,
            Mode::System if system_dark => Scheme::Dark,
            Mode::System => Scheme::Light,
        }
    }
}

/// How the new-thread background image is framed: the point it centers on
/// (0 to 1 each way) and its zoom, as the desktop keeps it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Framing {
    pub focal_x: f32,
    pub focal_y: f32,
    pub zoom: f32,
}

impl Framing {
    pub const MIN_ZOOM: f32 = 1.0;
    pub const MAX_ZOOM: f32 = 4.0;
    /// One step of the adjust controls.
    pub const STEP: f32 = 0.1;

    pub fn normalized(self) -> Self {
        let clamp = |value: f32, low: f32, high: f32, default: f32| {
            if value.is_finite() {
                value.clamp(low, high)
            } else {
                default
            }
        };
        Self {
            focal_x: clamp(self.focal_x, 0., 1., 0.5),
            focal_y: clamp(self.focal_y, 0., 1., 0.5),
            zoom: clamp(self.zoom, Self::MIN_ZOOM, Self::MAX_ZOOM, 1.),
        }
    }
}

impl Default for Framing {
    fn default() -> Self {
        Self {
            focal_x: 0.5,
            focal_y: 0.5,
            zoom: 1.,
        }
    }
}

/// The new-thread background image. The picture itself stays in the
/// browser's IndexedDB; this names it and its framing.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Background {
    pub name: String,
    #[serde(default)]
    pub framing: Framing,
}

/// A VS Code theme imported in this browser: its file's text, compiled
/// again each time the page loads ([`LibraryTheme::compile`]), so only the
/// source is stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTheme {
    /// The family's id, which is also its one variant's id.
    pub id: String,
    pub name: String,
    pub file: String,
    pub source: String,
}

impl LibraryTheme {
    /// The largest theme file the browser keeps.
    pub const MAX_SOURCE: usize = 512 * 1024;

    pub fn compile(&self) -> Option<ThemeFamily> {
        let mut family = compile_text(
            &self.source,
            &self.file,
            CompileOptions {
                family_id: self.id.clone(),
                family_name: self.name.clone(),
                source_url: String::new(),
                revision: String::new(),
                license: String::new(),
            },
        )
        .ok()?
        .family;
        family.name = self.name.clone();
        Some(family)
    }
}

/// The conversation's widest column, in points, as the desktop bounds it.
pub const WIDTH_MIN: u16 = 560;
pub const WIDTH_MAX: u16 = 1200;
pub const WIDTH_DEFAULT: u16 = 736;
pub const WIDTH_STEP: u16 = 16;

/// The appearance choices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub mode: Mode,
    pub themes: ThemeSelection,
    pub accent: AccentSelection,
    pub surface: SurfacePreference,
    pub background: Option<Background>,
    pub reduce_motion: ReduceMotion,
    pub pause_in_background: bool,
    pub conversation_width: u16,
    /// Themes imported in this browser.
    pub library: Vec<LibraryTheme>,
}

impl Default for Appearance {
    fn default() -> Self {
        Self {
            mode: Mode::System,
            themes: ThemeSelection::default(),
            accent: AccentSelection::ThemeDefault,
            surface: SurfacePreference::ThemeDefault,
            background: None,
            reduce_motion: ReduceMotion::System,
            pause_in_background: true,
            conversation_width: WIDTH_DEFAULT,
            library: Vec::new(),
        }
    }
}

/// An appearance change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Mode(Mode),
    Theme(Scheme, String),
    Accent(AccentSelection),
    Surface(SurfacePreference),
    /// The host asks the browser for an image, then answers with
    /// [`Change::SetBackground`].
    ChooseBackground,
    SetBackground(String),
    RemoveBackground,
    /// Move the framing by steps: right, down, and zoom in.
    Frame(i8, i8, i8),
    ResetFraming,
    ReduceMotion(ReduceMotion),
    PauseInBackground(bool),
    /// Narrower or wider by one step.
    Width(i8),
    /// The host asks the browser for a theme file, then answers with
    /// [`Change::Imported`].
    ImportTheme,
    Imported(LibraryTheme),
    DuplicateTheme(String),
    RemoveTheme(String),
}

impl Appearance {
    pub(super) fn apply(&mut self, change: &Change) {
        match change {
            Change::Mode(mode) => self.mode = *mode,
            Change::Theme(scheme, id) => self.themes.set_variant(*scheme, id.clone()),
            Change::Accent(accent) => self.accent = *accent,
            Change::Surface(surface) => self.surface = *surface,
            Change::SetBackground(name) => {
                self.background = Some(Background {
                    name: name.clone(),
                    framing: Framing::default(),
                })
            }
            Change::RemoveBackground => self.background = None,
            Change::Frame(right, down, zoom) => {
                if let Some(background) = &mut self.background {
                    let framing = &mut background.framing;
                    framing.focal_x += f32::from(*right) * Framing::STEP;
                    framing.focal_y += f32::from(*down) * Framing::STEP;
                    framing.zoom += f32::from(*zoom) * Framing::STEP * 2.;
                    *framing = framing.normalized();
                }
            }
            Change::ResetFraming => {
                if let Some(background) = &mut self.background {
                    background.framing = Framing::default();
                }
            }
            Change::ReduceMotion(value) => self.reduce_motion = *value,
            Change::PauseInBackground(on) => self.pause_in_background = *on,
            Change::Width(steps) => {
                let width = i32::from(self.width()) + i32::from(*steps) * i32::from(WIDTH_STEP);
                self.conversation_width =
                    width.clamp(i32::from(WIDTH_MIN), i32::from(WIDTH_MAX)) as u16;
            }
            Change::Imported(theme) => {
                self.library.retain(|t| t.id != theme.id);
                self.library.push(theme.clone());
            }
            Change::DuplicateTheme(id) => {
                if let Some(copy) = self.duplicate(id) {
                    self.library.push(copy);
                }
            }
            Change::RemoveTheme(id) => {
                self.library.retain(|t| &t.id != id);
                // A chosen theme that's gone falls back to the built-in one.
                let defaults = ThemeSelection::default();
                for scheme in [Scheme::Light, Scheme::Dark] {
                    if self.themes.variant_id(scheme) == id {
                        self.themes
                            .set_variant(scheme, defaults.variant_id(scheme).to_owned());
                    }
                }
            }
            Change::ChooseBackground | Change::ImportTheme => {}
        }
    }

    /// The column width, kept on the desktop's steps.
    pub fn width(&self) -> u16 {
        let width = self.conversation_width.clamp(WIDTH_MIN, WIDTH_MAX);
        WIDTH_MIN + (width - WIDTH_MIN + WIDTH_STEP / 2) / WIDTH_STEP * WIDTH_STEP
    }

    /// The palette these choices draw with, given whether the system
    /// prefers dark: the library's themes are registered first, so a chosen
    /// imported theme resolves as the desktop's do.
    pub fn theme(&self, system_dark: bool) -> crate::theme::Theme {
        self.theme_with(self.families(), system_dark)
    }

    /// [`Appearance::theme`] with the library already compiled, as the chat
    /// reads it back ([`Appearance::COMPILED_KEY`]) without linking the
    /// theme importer.
    pub fn theme_with(&self, families: Vec<ThemeFamily>, system_dark: bool) -> crate::theme::Theme {
        openagents_theme::replace_custom_families(families);
        let scheme = self.mode.resolve(system_dark);
        let appearance = match scheme {
            Scheme::Dark => crate::theme::Appearance::Dark,
            Scheme::Light => crate::theme::Appearance::Light,
        };
        crate::theme::Theme::for_selection(
            appearance,
            self.themes.variant_id(scheme),
            self.accent,
            self.surface,
        )
    }

    /// Where Settings keeps the library compiled, for the chat.
    pub const COMPILED_KEY: &'static str = "openagents.themes.v1";

    /// The library's themes, compiled.
    pub fn families(&self) -> Vec<ThemeFamily> {
        self.library
            .iter()
            .filter_map(LibraryTheme::compile)
            .collect()
    }

    /// A copy of the imported theme `id` under a new id and name.
    fn duplicate(&self, id: &str) -> Option<LibraryTheme> {
        let theme = self.library.iter().find(|t| t.id == id)?;
        let mut n = 2;
        let new_id = loop {
            let candidate = format!("{id}-copy-{n}");
            if !self.library.iter().any(|t| t.id == candidate) {
                break candidate;
            }
            n += 1;
        };
        Some(LibraryTheme {
            id: new_id,
            name: format!("{} copy", theme.name),
            ..theme.clone()
        })
    }
}

fn change(c: Change) -> Intent {
    Intent::Appearance(c)
}

pub(super) fn page(prefs: &Appearance, remote: &Remote, paints: &Paints) -> Vec<Node<Intent>> {
    let registry = ThemeRegistry::active();
    let mut scheme_rows = vec![row(
        "appearance-mode",
        "Appearance",
        None,
        segmented(
            "appearance-mode",
            &Mode::ALL.map(|m| (m.label(), change(Change::Mode(m)))),
            Mode::ALL.iter().position(|m| *m == prefs.mode).unwrap_or(0),
        ),
        paints,
    )];
    for (scheme, key, title) in [
        (Scheme::Light, "appearance-light", "Light theme"),
        (Scheme::Dark, "appearance-dark", "Dark theme"),
    ] {
        let variants: Vec<(String, String)> = registry
            .variants_for(scheme)
            .map(|v| (v.id.clone(), v.name.clone()))
            .collect();
        let chosen = prefs.themes.variant_id(scheme);
        let options: Vec<(&str, Intent)> = variants
            .iter()
            .map(|(id, name)| (name.as_str(), change(Change::Theme(scheme, id.clone()))))
            .collect();
        let at = variants
            .iter()
            .position(|(id, _)| id == chosen)
            .unwrap_or(usize::MAX);
        scheme_rows.push(wide_row(
            key,
            title,
            None,
            segmented(key, &options, at),
            paints,
        ));
    }
    let mut accents: Vec<(&str, Intent)> = vec![(
        "Theme default",
        change(Change::Accent(AccentSelection::ThemeDefault)),
    )];
    accents.extend(AccentPreset::ALL.map(|p| {
        (
            p.label(),
            change(Change::Accent(AccentSelection::Preset(p))),
        )
    }));
    let accent_at = match prefs.accent {
        AccentSelection::ThemeDefault => 0,
        AccentSelection::Preset(p) => AccentPreset::ALL
            .iter()
            .position(|x| *x == p)
            .map_or(0, |ix| ix + 1),
    };
    scheme_rows.push(wide_row(
        "appearance-accent",
        "Accent color",
        None,
        segmented("appearance-accent", &accents, accent_at),
        paints,
    ));

    let surfaces = [
        (SurfacePreference::ThemeDefault, "Theme default"),
        (SurfacePreference::Frosted, "Translucent surfaces"),
        (SurfacePreference::Opaque, "Solid surfaces"),
    ];
    let mut material = vec![row(
        "appearance-surface",
        "Glass",
        None,
        segmented(
            "appearance-surface",
            &surfaces.map(|(s, label)| (label, change(Change::Surface(s)))),
            surfaces
                .iter()
                .position(|(s, _)| *s == prefs.surface)
                .unwrap_or(0),
        ),
        paints,
    )];
    material.push(match &prefs.background {
        None => row(
            "appearance-background",
            "New thread background",
            Some("No image selected"),
            vec![button(
                "appearance-background-choose",
                "Choose image",
                change(Change::ChooseBackground),
            )],
            paints,
        ),
        Some(background) => {
            let framing = background.framing;
            let detail = format!(
                "{} · {}% zoom",
                background.name,
                (framing.zoom * 100.).round() as u32
            );
            wide_row(
                "appearance-background",
                "New thread background",
                Some(&detail),
                vec![
                    button(
                        "appearance-background-left",
                        "←",
                        change(Change::Frame(-1, 0, 0)),
                    ),
                    button(
                        "appearance-background-right",
                        "→",
                        change(Change::Frame(1, 0, 0)),
                    ),
                    button(
                        "appearance-background-up",
                        "↑",
                        change(Change::Frame(0, -1, 0)),
                    ),
                    button(
                        "appearance-background-down",
                        "↓",
                        change(Change::Frame(0, 1, 0)),
                    ),
                    button(
                        "appearance-background-out",
                        "−",
                        change(Change::Frame(0, 0, -1)),
                    ),
                    button(
                        "appearance-background-in",
                        "+",
                        change(Change::Frame(0, 0, 1)),
                    ),
                    button(
                        "appearance-background-reset",
                        "Reset",
                        change(Change::ResetFraming),
                    ),
                    button(
                        "appearance-background-replace",
                        "Replace image",
                        change(Change::ChooseBackground),
                    ),
                    danger_button(
                        "appearance-background-remove",
                        "Remove",
                        change(Change::RemoveBackground),
                        paints,
                    ),
                ],
                paints,
            )
        }
    });

    let motion = vec![
        row(
            "appearance-motion",
            "Reduce motion",
            None,
            segmented(
                "appearance-motion",
                &ReduceMotion::ALL.map(|m| (m.label(), change(Change::ReduceMotion(m)))),
                ReduceMotion::ALL
                    .iter()
                    .position(|m| *m == prefs.reduce_motion)
                    .unwrap_or(0),
            ),
            paints,
        ),
        row(
            "appearance-pause",
            "Pause animations in background",
            None,
            vec![toggle(
                "appearance-pause-toggle",
                prefs.pause_in_background,
                change(Change::PauseInBackground(!prefs.pause_in_background)),
            )],
            paints,
        ),
    ];

    let width = prefs.width();
    let layout = vec![row(
        "appearance-width",
        "Conversation width",
        Some(&format!("{width} pt")),
        vec![
            super::enabled_button(
                "appearance-width-narrower",
                "Narrower",
                width > WIDTH_MIN,
                change(Change::Width(-1)),
            ),
            super::enabled_button(
                "appearance-width-wider",
                "Wider",
                width < WIDTH_MAX,
                change(Change::Width(1)),
            ),
        ],
        paints,
    )];

    let mut library: Vec<Node<Intent>> = prefs
        .library
        .iter()
        .enumerate()
        .map(|(ix, family)| {
            // The registry holds the compiled theme once the host installs it.
            let kind = match registry.variant(&family.id) {
                Some(v) if v.appearance.is_dark() => "Dark",
                Some(_) => "Light",
                None => "Couldn't be read",
            };
            row(
                &format!("appearance-library-{ix}"),
                &family.name,
                Some(kind),
                vec![
                    button(
                        &format!("appearance-library-{ix}-duplicate"),
                        "Duplicate",
                        change(Change::DuplicateTheme(family.id.clone())),
                    ),
                    danger_button(
                        &format!("appearance-library-{ix}-remove"),
                        "Remove",
                        change(Change::RemoveTheme(family.id.clone())),
                        paints,
                    ),
                ],
                paints,
            )
        })
        .collect();
    library.push(row(
        "appearance-import",
        "Import a VS Code theme",
        Some(
            remote
                .theme_notice
                .as_deref()
                .unwrap_or("A theme's .json file. It's kept in this browser."),
        ),
        vec![button(
            "appearance-import-file",
            "Choose file",
            change(Change::ImportTheme),
        )],
        paints,
    ));

    vec![
        header("appearance-title", "Appearance", None, paints),
        section("appearance-scheme", "Color scheme", scheme_rows, paints),
        section(
            "appearance-material",
            "Material and background",
            material,
            paints,
        ),
        section("appearance-motion-section", "Motion", motion, paints),
        section("appearance-layout", "Layout", layout, paints),
        section("appearance-themes", "Theme library", library, paints),
        node(
            "appearance-foot",
            Style::default(),
            Element::Stack {
                axis: Axis::Vertical,
                children: vec![text(
                    "appearance-foot-text",
                    "These choices are kept in this browser.",
                    paints.faint,
                )],
            },
        ),
    ]
}
