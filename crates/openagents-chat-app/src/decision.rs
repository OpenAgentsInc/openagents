//! The decision panel (`decision-panel`), painted in the scheme the app
//! draws with.

pub use decision_panel::*;

/// The panel's colors in the scheme the app paints with
/// ([`crate::visual::inks`]).
#[must_use]
pub fn paints() -> Paints {
    let inks = crate::visual::inks();
    Paints {
        card: inks.card,
        risk_low: inks.risk_low,
        risk_medium: inks.risk_medium,
        risk_high: inks.risk_high,
    }
}
