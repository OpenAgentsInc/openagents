use crate::components::atoms::{AmountDirection, BitcoinAmount, BitcoinNetwork, NetworkBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

/// Balance breakdown by payment method
#[derive(Debug, Clone, Default)]
pub struct WalletBalance {
    pub spark_sats: u64,
    pub lightning_sats: u64,
    pub onchain_sats: u64,
}

impl WalletBalance {
    pub fn new(spark: u64, lightning: u64, onchain: u64) -> Self {
        Self {
            spark_sats: spark,
            lightning_sats: lightning,
            onchain_sats: onchain,
        }
    }

    pub fn total(&self) -> u64 {
        self.spark_sats + self.lightning_sats + self.onchain_sats
    }
}

pub struct BalanceCard {
    id: Option<ComponentId>,
    balance: WalletBalance,
    network: BitcoinNetwork,
    show_breakdown: bool,
}

impl BalanceCard {
    pub fn new(balance: WalletBalance) -> Self {
        Self {
            id: None,
            balance,
            network: BitcoinNetwork::Mainnet,
            show_breakdown: true,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn network(mut self, network: BitcoinNetwork) -> Self {
        self.network = network;
        self
    }

    pub fn show_breakdown(mut self, show: bool) -> Self {
        self.show_breakdown = show;
        self
    }

    pub fn balance(&self) -> &WalletBalance {
        &self.balance
    }

    pub fn set_balance(&mut self, balance: WalletBalance) {
        self.balance = balance;
    }
}

impl Default for BalanceCard {
    fn default() -> Self {
        Self::new(WalletBalance::default())
    }
}

impl Component for BalanceCard {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = 16.0;
        let mut y = bounds.origin.y + padding;

        // Header row: "Balance" + Network badge
        let header_text = cx.text.layout(
            "Balance",
            Point::new(bounds.origin.x + padding, y),
            theme::font_size::SM,
            theme::text::MUTED,
        );
        cx.scene.draw_text(header_text);

        // Network badge on the right
        let badge_w = 64.0;
        let badge_h = 20.0;
        let badge_x = bounds.origin.x + bounds.size.width - padding - badge_w;
        let mut network_badge = NetworkBadge::new(self.network);
        network_badge.paint(Bounds::new(badge_x, y - 2.0, badge_w, badge_h), cx);

        y += 28.0;

        // Total balance (large)
        let mut total_amount = BitcoinAmount::new(self.balance.total())
            .font_size(theme::font_size::XXL)
            .direction(AmountDirection::Neutral);
        total_amount.paint(
            Bounds::new(
                bounds.origin.x + padding,
                y,
                bounds.size.width - padding * 2.0,
                32.0,
            ),
            cx,
        );

        y += 40.0;

        // Breakdown if enabled
        if self.show_breakdown {
            // Separator line
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    bounds.origin.x + padding,
                    y,
                    bounds.size.width - padding * 2.0,
                    1.0,
                ))
                .with_background(theme::border::DEFAULT),
            );

            y += 16.0;

            let row_height = 24.0;
            let label_x = bounds.origin.x + padding;
            let value_x = bounds.origin.x + bounds.size.width - padding - 100.0;

            // Spark balance
            let spark_label = cx.text.layout(
                "✦ Spark",
                Point::new(label_x, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(spark_label);
            let mut spark_amount =
                BitcoinAmount::new(self.balance.spark_sats).font_size(theme::font_size::SM);
            spark_amount.paint(Bounds::new(value_x, y, 100.0, row_height), cx);
            y += row_height;

            // Lightning balance
            let ln_label = cx.text.layout(
                "⚡ Lightning",
                Point::new(label_x, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(ln_label);
            let mut ln_amount =
                BitcoinAmount::new(self.balance.lightning_sats).font_size(theme::font_size::SM);
            ln_amount.paint(Bounds::new(value_x, y, 100.0, row_height), cx);
            y += row_height;

            // On-chain balance
            let chain_label = cx.text.layout(
                "₿ On-chain",
                Point::new(label_x, y),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(chain_label);
            let mut chain_amount =
                BitcoinAmount::new(self.balance.onchain_sats).font_size(theme::font_size::SM);
            chain_amount.paint(Bounds::new(value_x, y, 100.0, row_height), cx);
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = if self.show_breakdown { 180.0 } else { 100.0 };
        (Some(300.0), Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wallet_balance() {
        let balance = WalletBalance::new(10000, 5000, 2000);
        assert_eq!(balance.total(), 17000);
    }

    #[test]
    fn test_balance_card() {
        let balance = WalletBalance::new(100000, 50000, 0);
        let card = BalanceCard::new(balance).network(BitcoinNetwork::Testnet);
        assert_eq!(card.balance().total(), 150000);
    }
}
