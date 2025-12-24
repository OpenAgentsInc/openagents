use crate::color::Hsla;

pub mod bg {
    use super::Hsla;

    pub const APP: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.0, a: 1.0 };
    pub const SURFACE: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.04, a: 1.0 };
    pub const ELEVATED: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.08, a: 1.0 };
}

pub mod text {
    use super::Hsla;

    pub const PRIMARY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.9, a: 1.0 };
    pub const SECONDARY: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.6, a: 1.0 };
    pub const MUTED: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.4, a: 1.0 };
}

pub mod accent {
    use super::Hsla;

    pub const PRIMARY: Hsla = Hsla { h: 43.0, s: 1.0, l: 0.5, a: 1.0 };
    pub const SECONDARY: Hsla = Hsla { h: 210.0, s: 1.0, l: 0.5, a: 1.0 };
}

pub mod border {
    use super::Hsla;

    pub const DEFAULT: Hsla = Hsla { h: 0.0, s: 0.0, l: 0.2, a: 1.0 };
    pub const FOCUS: Hsla = Hsla { h: 43.0, s: 1.0, l: 0.5, a: 1.0 };
}

pub mod status {
    use super::Hsla;

    pub const SUCCESS: Hsla = Hsla { h: 145.0, s: 1.0, l: 0.39, a: 1.0 };
    pub const WARNING: Hsla = Hsla { h: 43.0, s: 1.0, l: 0.5, a: 1.0 };
    pub const ERROR: Hsla = Hsla { h: 0.0, s: 0.73, l: 0.5, a: 1.0 };
}
