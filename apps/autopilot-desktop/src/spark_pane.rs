use wgpui::{Bounds, Point};

pub const SPARK_PANE_WIDTH: f32 = 820.0;
pub const SPARK_PANE_HEIGHT: f32 = 460.0;

const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
const CONTROL_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparkPaneAction {
    Refresh,
    GenerateSparkAddress,
    GenerateBitcoinAddress,
    CreateInvoice,
    SendPayment,
}

#[derive(Clone, Copy)]
pub struct SparkPaneLayout {
    pub refresh_button: Bounds,
    pub spark_address_button: Bounds,
    pub bitcoin_address_button: Bounds,
    pub invoice_amount_input: Bounds,
    pub create_invoice_button: Bounds,
    pub send_request_input: Bounds,
    pub send_amount_input: Bounds,
    pub send_payment_button: Bounds,
    pub details_origin: Point,
}

pub fn layout(content_bounds: Bounds) -> SparkPaneLayout {
    let origin_x = content_bounds.origin.x + PAD;
    let origin_y = content_bounds.origin.y + PAD;
    let usable_width = (content_bounds.size.width - PAD * 2.0).max(240.0);

    let top_button_width = ((usable_width - GAP * 2.0) / 3.0).max(96.0);
    let refresh_button = Bounds::new(origin_x, origin_y, top_button_width, CONTROL_HEIGHT);
    let spark_address_button = Bounds::new(
        refresh_button.origin.x + refresh_button.size.width + GAP,
        origin_y,
        top_button_width,
        CONTROL_HEIGHT,
    );
    let bitcoin_address_button = Bounds::new(
        spark_address_button.origin.x + spark_address_button.size.width + GAP,
        origin_y,
        top_button_width,
        CONTROL_HEIGHT,
    );

    let invoice_row_y = origin_y + CONTROL_HEIGHT + 10.0;
    let invoice_input_width = (usable_width * 0.28).clamp(110.0, 180.0);
    let invoice_amount_input =
        Bounds::new(origin_x, invoice_row_y, invoice_input_width, CONTROL_HEIGHT);
    let create_invoice_button = Bounds::new(
        invoice_amount_input.origin.x + invoice_amount_input.size.width + GAP,
        invoice_row_y,
        (usable_width - invoice_input_width - GAP).max(120.0),
        CONTROL_HEIGHT,
    );

    let send_request_y = invoice_row_y + CONTROL_HEIGHT + 10.0;
    let send_request_input = Bounds::new(origin_x, send_request_y, usable_width, CONTROL_HEIGHT);

    let send_row_y = send_request_y + CONTROL_HEIGHT + 10.0;
    let send_amount_width = (usable_width * 0.28).clamp(110.0, 180.0);
    let send_amount_input = Bounds::new(origin_x, send_row_y, send_amount_width, CONTROL_HEIGHT);
    let send_payment_button = Bounds::new(
        send_amount_input.origin.x + send_amount_input.size.width + GAP,
        send_row_y,
        (usable_width - send_amount_width - GAP).max(120.0),
        CONTROL_HEIGHT,
    );

    let details_origin = Point::new(origin_x, send_row_y + CONTROL_HEIGHT + 14.0);

    SparkPaneLayout {
        refresh_button,
        spark_address_button,
        bitcoin_address_button,
        invoice_amount_input,
        create_invoice_button,
        send_request_input,
        send_amount_input,
        send_payment_button,
        details_origin,
    }
}

pub fn hit_action(layout: SparkPaneLayout, point: Point) -> Option<SparkPaneAction> {
    if layout.refresh_button.contains(point) {
        return Some(SparkPaneAction::Refresh);
    }
    if layout.spark_address_button.contains(point) {
        return Some(SparkPaneAction::GenerateSparkAddress);
    }
    if layout.bitcoin_address_button.contains(point) {
        return Some(SparkPaneAction::GenerateBitcoinAddress);
    }
    if layout.create_invoice_button.contains(point) {
        return Some(SparkPaneAction::CreateInvoice);
    }
    if layout.send_payment_button.contains(point) {
        return Some(SparkPaneAction::SendPayment);
    }
    None
}

pub fn hits_input(layout: SparkPaneLayout, point: Point) -> bool {
    layout.invoice_amount_input.contains(point)
        || layout.send_request_input.contains(point)
        || layout.send_amount_input.contains(point)
}

#[cfg(test)]
mod tests {
    use super::{SparkPaneAction, hit_action, hits_input, layout};
    use wgpui::{Bounds, Point};

    #[test]
    fn layout_rows_are_monotonic() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 460.0);
        let layout = layout(bounds);

        assert!(layout.refresh_button.origin.y < layout.invoice_amount_input.origin.y);
        assert!(layout.invoice_amount_input.origin.y < layout.send_request_input.origin.y);
        assert!(layout.send_request_input.origin.y < layout.send_amount_input.origin.y);
        assert!(layout.details_origin.y > layout.send_amount_input.origin.y);
    }

    #[test]
    fn hit_action_detects_buttons() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 460.0);
        let layout = layout(bounds);

        let refresh = Point::new(
            layout.refresh_button.origin.x + 3.0,
            layout.refresh_button.origin.y + 3.0,
        );
        assert_eq!(hit_action(layout, refresh), Some(SparkPaneAction::Refresh));

        let send = Point::new(
            layout.send_payment_button.origin.x + 3.0,
            layout.send_payment_button.origin.y + 3.0,
        );
        assert_eq!(hit_action(layout, send), Some(SparkPaneAction::SendPayment));
    }

    #[test]
    fn hits_input_only_for_input_regions() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 460.0);
        let layout = layout(bounds);

        let invoice_input = Point::new(
            layout.invoice_amount_input.origin.x + 2.0,
            layout.invoice_amount_input.origin.y + 2.0,
        );
        assert!(hits_input(layout, invoice_input));

        let button_point = Point::new(
            layout.refresh_button.origin.x + 2.0,
            layout.refresh_button.origin.y + 2.0,
        );
        assert!(!hits_input(layout, button_point));
    }
}
