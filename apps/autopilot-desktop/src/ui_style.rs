use wgpui::Hsla;

// ============================================================================
// AUTOPILOT UI STYLE TOKENS
// ============================================================================
//
// This file is the closest thing this app has to a CSS variables file.
//
// If you want to make broad visual changes without digging through pane code,
// start here.
//
// The main sections below map to common UI elements:
//
// 1. text
//    - pane headers
//    - section headings
//    - form labels
//    - form values
//    - supporting / helper copy
//
// 2. input
//    - input background
//    - input border
//    - input corner radius
//
// 3. button
//    - primary button
//    - secondary button
//    - tertiary button
//    - disabled button
//
// Think of the constants below like CSS custom properties / design tokens.
// Example:
// - change a font size once here
// - all shared uses of that role update together
//
// Notes:
// - Colors written as `0xFFFFFF` are hex colors.
// - `theme::...` values come from the shared app theme.
// - Roles are how the renderer asks for styles; constants are what you edit.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppTextRole {
    // Main pane/window title text.
    Header,
    // Small section titles such as Mission Control container headings.
    SectionHeading,
    // Primary row/title text used in list items and thread rows.
    PrimaryRow,
    // Secondary metadata / preview text beneath or beside primary rows.
    SecondaryMetadata,
    // Left-side labels in forms and rows, e.g. "Today", "Network", "Status".
    FormLabel,
    // Right-side values in forms and rows, e.g. "Connected", "0 sats".
    FormValue,
    // Secondary/supporting copy such as helper text or small descriptions.
    Supporting,
    // Small helper / hint text used for guidance and system notes.
    Helper,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AppTextStyle {
    pub font_size: f32,
    pub color: Hsla,
    pub mono: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AppInputStyle {
    pub background: Hsla,
    pub border: Hsla,
    pub border_width: f32,
    pub corner_radius: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppButtonRole {
    Primary,
    Secondary,
    Tertiary,
    Disabled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AppSpacingRole {
    PanelPadding,
    RowPadding,
    SectionGap,
    ButtonGap,
}

pub(crate) mod text {
    use super::{AppTextRole, AppTextStyle};
    use wgpui::theme;

    // ------------------------------------------------------------------------
    // TEXT TOKENS
    // ------------------------------------------------------------------------
    //
    // Edit these values to change text styling globally for shared pane text.
    //
    // HEADER
    // Used for larger pane/window titles.
    pub(crate) const HEADER_FONT_SIZE: f32 = 13.0;

    // SECTION HEADING
    // Used for container headings like:
    // "\\ SELL COMPUTE", "\\ WALLET & EARNINGS", "\\ LOG STREAM"
    pub(crate) const SECTION_HEADING_FONT_SIZE: f32 = 10.0;

    // FORM LABEL
    // Used for left-side labels like:
    // "Today", "This Month", "Address", "Network"
    pub(crate) const FORM_LABEL_FONT_SIZE: f32 = 12.0;

    // PRIMARY ROW
    // Used for dominant list-row titles like thread names.
    pub(crate) const PRIMARY_ROW_FONT_SIZE: f32 = 11.5;

    // SECONDARY METADATA
    // Used for preview text, thread metadata, and subordinate row content.
    pub(crate) const SECONDARY_METADATA_FONT_SIZE: f32 = 8.5;

    // FORM VALUE
    // Used for right-side values like:
    // "0 BTC", "CONNECTED", "MAINNET"
    pub(crate) const FORM_VALUE_FONT_SIZE: f32 = 12.0;

    // SUPPORTING
    // Used for helper or supporting text blocks.
    pub(crate) const SUPPORTING_FONT_SIZE: f32 = theme::font_size::SM;

    // Shared text colors.
    pub(crate) const HEADER_COLOR: wgpui::Hsla = theme::text::PRIMARY;
    pub(crate) const SECTION_HEADING_COLOR: wgpui::Hsla = theme::text::PRIMARY;
    pub(crate) const PRIMARY_ROW_COLOR: wgpui::Hsla = theme::text::PRIMARY;
    pub(crate) const SECONDARY_METADATA_COLOR: wgpui::Hsla = theme::text::MUTED;
    pub(crate) const FORM_LABEL_COLOR: wgpui::Hsla = theme::text::MUTED;
    pub(crate) const FORM_VALUE_COLOR: wgpui::Hsla = theme::text::PRIMARY;
    pub(crate) const SUPPORTING_COLOR: wgpui::Hsla = theme::text::MUTED;
    pub(crate) const HELPER_COLOR: wgpui::Hsla = theme::text::MUTED;

    // Font family control.
    // `true` = mono
    // `false` = ui/proportional
    pub(crate) const HEADER_MONO: bool = true;
    pub(crate) const SECTION_HEADING_MONO: bool = true;
    pub(crate) const PRIMARY_ROW_MONO: bool = true;
    pub(crate) const SECONDARY_METADATA_MONO: bool = true;
    pub(crate) const FORM_LABEL_MONO: bool = true;
    pub(crate) const FORM_VALUE_MONO: bool = true;
    pub(crate) const SUPPORTING_MONO: bool = false;
    pub(crate) const HELPER_MONO: bool = true;

    pub(crate) fn style(role: AppTextRole) -> AppTextStyle {
        match role {
            AppTextRole::Header => AppTextStyle {
                font_size: HEADER_FONT_SIZE,
                color: HEADER_COLOR,
                mono: HEADER_MONO,
            },
            AppTextRole::SectionHeading => AppTextStyle {
                font_size: SECTION_HEADING_FONT_SIZE,
                color: SECTION_HEADING_COLOR,
                mono: SECTION_HEADING_MONO,
            },
            AppTextRole::PrimaryRow => AppTextStyle {
                font_size: PRIMARY_ROW_FONT_SIZE,
                color: PRIMARY_ROW_COLOR,
                mono: PRIMARY_ROW_MONO,
            },
            AppTextRole::SecondaryMetadata => AppTextStyle {
                font_size: SECONDARY_METADATA_FONT_SIZE,
                color: SECONDARY_METADATA_COLOR,
                mono: SECONDARY_METADATA_MONO,
            },
            AppTextRole::FormLabel => AppTextStyle {
                font_size: FORM_LABEL_FONT_SIZE,
                color: FORM_LABEL_COLOR,
                mono: FORM_LABEL_MONO,
            },
            AppTextRole::FormValue => AppTextStyle {
                font_size: FORM_VALUE_FONT_SIZE,
                color: FORM_VALUE_COLOR,
                mono: FORM_VALUE_MONO,
            },
            AppTextRole::Supporting => AppTextStyle {
                font_size: SUPPORTING_FONT_SIZE,
                color: SUPPORTING_COLOR,
                mono: SUPPORTING_MONO,
            },
            AppTextRole::Helper => AppTextStyle {
                font_size: SECONDARY_METADATA_FONT_SIZE,
                color: HELPER_COLOR,
                mono: HELPER_MONO,
            },
        }
    }
}

pub(crate) mod spacing {
    use super::AppSpacingRole;

    // ------------------------------------------------------------------------
    // SPACING TOKENS
    // ------------------------------------------------------------------------
    //
    // These work like layout spacing variables in CSS.
    // Update them here to change shared panel rhythm.

    // Standard inner panel padding.
    pub(crate) const PANEL_PADDING: f32 = 12.0;

    // Standard horizontal padding for rows/list items.
    pub(crate) const ROW_PADDING: f32 = 14.0;

    // Standard gap between sections / conversation blocks.
    pub(crate) const SECTION_GAP: f32 = 12.0;

    // Standard gap between related action buttons/chips.
    pub(crate) const BUTTON_GAP: f32 = 6.0;

    pub(crate) fn value(role: AppSpacingRole) -> f32 {
        match role {
            AppSpacingRole::PanelPadding => PANEL_PADDING,
            AppSpacingRole::RowPadding => ROW_PADDING,
            AppSpacingRole::SectionGap => SECTION_GAP,
            AppSpacingRole::ButtonGap => BUTTON_GAP,
        }
    }
}

pub(crate) mod input {
    use super::AppInputStyle;
    use wgpui::theme;

    // ------------------------------------------------------------------------
    // INPUT TOKENS
    // ------------------------------------------------------------------------
    //
    // These are the shared "chrome" values for standard input fields.
    // If you want inputs globally darker/lighter/rounder, edit these.
    //
    // Background opacity of the shared input surface.
    pub(crate) const BACKGROUND_ALPHA: f32 = 0.78;

    // Border stroke width.
    pub(crate) const BORDER_WIDTH: f32 = 1.0;

    // Corner radius.
    pub(crate) const CORNER_RADIUS: f32 = 6.0;

    // Base colors.
    pub(crate) const BACKGROUND_COLOR: wgpui::Hsla = theme::bg::APP;
    pub(crate) const BORDER_COLOR: wgpui::Hsla = theme::border::DEFAULT;

    pub(crate) fn style() -> AppInputStyle {
        AppInputStyle {
            background: BACKGROUND_COLOR.with_alpha(BACKGROUND_ALPHA),
            border: BORDER_COLOR,
            border_width: BORDER_WIDTH,
            corner_radius: CORNER_RADIUS,
        }
    }
}

pub(crate) mod button {
    use super::AppButtonRole;
    use wgpui::{Hsla, theme};

    // ------------------------------------------------------------------------
    // BUTTON TOKENS
    // ------------------------------------------------------------------------
    //
    // PRIMARY
    // Used by prominent call-to-action buttons.
    //
    // SECONDARY / TERTIARY / DISABLED
    // Used by shared button painters outside the custom Mission Control CTA.
    //
    // If you think in CSS, this section is closest to:
    // --button-primary-bg
    // --button-primary-border
    // --button-primary-text
    // etc.

    pub(crate) const PRIMARY_GLOW_COLOR: u32 = 0x0891B2;
    pub(crate) const PRIMARY_GLOW_ALPHA: f32 = 0.08;
    pub(crate) const PRIMARY_BACKGROUND: u32 = 0x121419;
    pub(crate) const PRIMARY_BORDER: u32 = 0x0891B2;
    pub(crate) const PRIMARY_HIGHLIGHT: u32 = 0x03857F;
    pub(crate) const PRIMARY_HIGHLIGHT_ALPHA: f32 = 0.34;
    pub(crate) const PRIMARY_LABEL_SIZE: f32 = 18.0;
    pub(crate) const PRIMARY_LABEL_COLOR: u32 = 0xFFFFFF;
    pub(crate) const PRIMARY_CORNER_RADIUS: f32 = 10.0;
    pub(crate) const PRIMARY_OUTER_RADIUS: f32 = 14.0;

    pub(crate) const SECONDARY_CORNER_RADIUS: f32 = 6.0;
    pub(crate) const TERTIARY_CORNER_RADIUS: f32 = 6.0;
    pub(crate) const DISABLED_CORNER_RADIUS: f32 = 6.0;

    // Shared label font sizes.
    pub(crate) fn label_font_size(role: AppButtonRole) -> f32 {
        match role {
            AppButtonRole::Primary => PRIMARY_LABEL_SIZE,
            AppButtonRole::Secondary | AppButtonRole::Tertiary | AppButtonRole::Disabled => {
                theme::font_size::SM
            }
        }
    }

    // Shared label text colors.
    pub(crate) fn label_color(role: AppButtonRole) -> Hsla {
        match role {
            AppButtonRole::Primary => Hsla::from_hex(PRIMARY_LABEL_COLOR),
            AppButtonRole::Secondary => theme::text::PRIMARY,
            AppButtonRole::Tertiary => theme::text::SECONDARY,
            AppButtonRole::Disabled => theme::text::MUTED,
        }
    }
}

pub(crate) fn app_text_style(role: AppTextRole) -> AppTextStyle {
    text::style(role)
}

pub(crate) fn app_input_style() -> AppInputStyle {
    input::style()
}

pub(crate) fn app_spacing(role: AppSpacingRole) -> f32 {
    spacing::value(role)
}
