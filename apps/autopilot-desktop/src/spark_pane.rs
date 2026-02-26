use wgpui::{Bounds, Point};

pub const SPARK_PANE_WIDTH: f32 = 820.0;
pub const SPARK_PANE_HEIGHT: f32 = 460.0;
pub const CREATE_INVOICE_PANE_WIDTH: f32 = 820.0;
pub const CREATE_INVOICE_PANE_HEIGHT: f32 = 280.0;
pub const PAY_INVOICE_PANE_WIDTH: f32 = 820.0;
pub const PAY_INVOICE_PANE_HEIGHT: f32 = 240.0;

const PAD: f32 = 12.0;
const GAP: f32 = 8.0;
const CONTROL_HEIGHT: f32 = 30.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SparkPaneAction {
    Refresh,
    GenerateSparkAddress,
    GenerateBitcoinAddress,
    CopySparkAddress,
    CreateInvoice,
    SendPayment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PayInvoicePaneAction {
    SendPayment,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateInvoicePaneAction {
    CreateInvoice,
    CopyInvoice,
}

#[derive(Clone, Copy)]
pub struct SparkPaneLayout {
    pub refresh_button: Bounds,
    pub spark_address_button: Bounds,
    pub bitcoin_address_button: Bounds,
    pub copy_spark_address_button: Bounds,
    pub invoice_amount_input: Bounds,
    pub create_invoice_button: Bounds,
    pub send_request_input: Bounds,
    pub send_amount_input: Bounds,
    pub send_payment_button: Bounds,
    pub details_origin: Point,
}

#[derive(Clone, Copy)]
pub struct PayInvoicePaneLayout {
    pub payment_request_input: Bounds,
    pub amount_input: Bounds,
    pub send_payment_button: Bounds,
    pub details_origin: Point,
}

#[derive(Clone, Copy)]
pub struct CreateInvoicePaneLayout {
    pub amount_input: Bounds,
    pub description_input: Bounds,
    pub expiry_input: Bounds,
    pub create_invoice_button: Bounds,
    pub copy_invoice_button: Bounds,
    pub details_origin: Point,
}

pub fn layout(content_bounds: Bounds) -> SparkPaneLayout {
    let origin_x = content_bounds.origin.x + PAD;
    let origin_y = content_bounds.origin.y + PAD;
    let usable_width = (content_bounds.size.width - PAD * 2.0).max(240.0);

    let top_button_width = ((usable_width - GAP * 3.0) / 4.0).max(88.0);
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
    let copy_spark_address_button = Bounds::new(
        bitcoin_address_button.origin.x + bitcoin_address_button.size.width + GAP,
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
        copy_spark_address_button,
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
    if layout.copy_spark_address_button.contains(point) {
        return Some(SparkPaneAction::CopySparkAddress);
    }
    if layout.create_invoice_button.contains(point) {
        return Some(SparkPaneAction::CreateInvoice);
    }
    if layout.send_payment_button.contains(point) {
        return Some(SparkPaneAction::SendPayment);
    }
    None
}

pub fn create_invoice_layout(content_bounds: Bounds) -> CreateInvoicePaneLayout {
    let origin_x = content_bounds.origin.x + PAD;
    let origin_y = content_bounds.origin.y + PAD;
    let usable_width = (content_bounds.size.width - PAD * 2.0).max(240.0);

    let amount_width = (usable_width * 0.22).clamp(120.0, 180.0);
    let expiry_width = (usable_width * 0.22).clamp(120.0, 180.0);
    let create_invoice_button = Bounds::new(
        origin_x + amount_width + GAP + expiry_width + GAP,
        origin_y,
        (usable_width - amount_width - expiry_width - GAP * 2.0).max(140.0),
        CONTROL_HEIGHT,
    );
    let amount_input = Bounds::new(origin_x, origin_y, amount_width, CONTROL_HEIGHT);
    let expiry_input = Bounds::new(
        amount_input.origin.x + amount_input.size.width + GAP,
        origin_y,
        expiry_width,
        CONTROL_HEIGHT,
    );

    let description_y = origin_y + CONTROL_HEIGHT + 10.0;
    let description_input = Bounds::new(origin_x, description_y, usable_width, CONTROL_HEIGHT);

    let copy_y = description_y + CONTROL_HEIGHT + 10.0;
    let copy_invoice_button = Bounds::new(
        origin_x,
        copy_y,
        (usable_width * 0.34).clamp(140.0, 240.0),
        CONTROL_HEIGHT,
    );
    let details_origin = Point::new(origin_x, copy_y + CONTROL_HEIGHT + 14.0);

    CreateInvoicePaneLayout {
        amount_input,
        description_input,
        expiry_input,
        create_invoice_button,
        copy_invoice_button,
        details_origin,
    }
}

pub fn hit_create_invoice_action(
    layout: CreateInvoicePaneLayout,
    point: Point,
) -> Option<CreateInvoicePaneAction> {
    if layout.create_invoice_button.contains(point) {
        return Some(CreateInvoicePaneAction::CreateInvoice);
    }
    if layout.copy_invoice_button.contains(point) {
        return Some(CreateInvoicePaneAction::CopyInvoice);
    }
    None
}

pub fn hits_create_invoice_input(layout: CreateInvoicePaneLayout, point: Point) -> bool {
    layout.amount_input.contains(point)
        || layout.description_input.contains(point)
        || layout.expiry_input.contains(point)
}

pub fn pay_invoice_layout(content_bounds: Bounds) -> PayInvoicePaneLayout {
    let origin_x = content_bounds.origin.x + PAD;
    let origin_y = content_bounds.origin.y + PAD;
    let usable_width = (content_bounds.size.width - PAD * 2.0).max(240.0);

    let request_y = origin_y;
    let payment_request_input = Bounds::new(origin_x, request_y, usable_width, CONTROL_HEIGHT);

    let send_row_y = request_y + CONTROL_HEIGHT + 10.0;
    let amount_width = (usable_width * 0.28).clamp(110.0, 180.0);
    let amount_input = Bounds::new(origin_x, send_row_y, amount_width, CONTROL_HEIGHT);
    let send_payment_button = Bounds::new(
        amount_input.origin.x + amount_input.size.width + GAP,
        send_row_y,
        (usable_width - amount_width - GAP).max(120.0),
        CONTROL_HEIGHT,
    );
    let details_origin = Point::new(origin_x, send_row_y + CONTROL_HEIGHT + 14.0);

    PayInvoicePaneLayout {
        payment_request_input,
        amount_input,
        send_payment_button,
        details_origin,
    }
}

pub fn hit_pay_invoice_action(
    layout: PayInvoicePaneLayout,
    point: Point,
) -> Option<PayInvoicePaneAction> {
    if layout.send_payment_button.contains(point) {
        return Some(PayInvoicePaneAction::SendPayment);
    }
    None
}

pub fn hits_pay_invoice_input(layout: PayInvoicePaneLayout, point: Point) -> bool {
    layout.payment_request_input.contains(point) || layout.amount_input.contains(point)
}

pub fn hits_input(layout: SparkPaneLayout, point: Point) -> bool {
    layout.invoice_amount_input.contains(point)
        || layout.send_request_input.contains(point)
        || layout.send_amount_input.contains(point)
}

#[cfg(test)]
mod tests {
    use super::{
        CreateInvoicePaneAction, PayInvoicePaneAction, SparkPaneAction, create_invoice_layout,
        hit_action, hit_create_invoice_action, hit_pay_invoice_action, hits_create_invoice_input,
        hits_input, hits_pay_invoice_input, layout, pay_invoice_layout,
    };
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

        let copy = Point::new(
            layout.copy_spark_address_button.origin.x + 3.0,
            layout.copy_spark_address_button.origin.y + 3.0,
        );
        assert_eq!(
            hit_action(layout, copy),
            Some(SparkPaneAction::CopySparkAddress)
        );
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

    #[test]
    fn pay_invoice_layout_rows_are_monotonic() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 240.0);
        let layout = pay_invoice_layout(bounds);

        assert!(layout.payment_request_input.origin.y < layout.amount_input.origin.y);
        assert!(layout.details_origin.y > layout.amount_input.origin.y);
    }

    #[test]
    fn pay_invoice_hit_detection_matches_controls() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 240.0);
        let layout = pay_invoice_layout(bounds);

        let send = Point::new(
            layout.send_payment_button.origin.x + 3.0,
            layout.send_payment_button.origin.y + 3.0,
        );
        assert_eq!(
            hit_pay_invoice_action(layout, send),
            Some(PayInvoicePaneAction::SendPayment)
        );

        let request_input = Point::new(
            layout.payment_request_input.origin.x + 2.0,
            layout.payment_request_input.origin.y + 2.0,
        );
        assert!(hits_pay_invoice_input(layout, request_input));
    }

    #[test]
    fn create_invoice_layout_rows_are_monotonic() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 280.0);
        let layout = create_invoice_layout(bounds);

        assert!(layout.amount_input.origin.y < layout.description_input.origin.y);
        assert!(layout.description_input.origin.y < layout.copy_invoice_button.origin.y);
        assert!(layout.details_origin.y > layout.copy_invoice_button.origin.y);
    }

    #[test]
    fn create_invoice_hit_detection_matches_controls() {
        let bounds = Bounds::new(0.0, 0.0, 820.0, 280.0);
        let layout = create_invoice_layout(bounds);

        let create_point = Point::new(
            layout.create_invoice_button.origin.x + 3.0,
            layout.create_invoice_button.origin.y + 3.0,
        );
        assert_eq!(
            hit_create_invoice_action(layout, create_point),
            Some(CreateInvoicePaneAction::CreateInvoice)
        );

        let copy_point = Point::new(
            layout.copy_invoice_button.origin.x + 3.0,
            layout.copy_invoice_button.origin.y + 3.0,
        );
        assert_eq!(
            hit_create_invoice_action(layout, copy_point),
            Some(CreateInvoicePaneAction::CopyInvoice)
        );

        let description_input = Point::new(
            layout.description_input.origin.x + 2.0,
            layout.description_input.origin.y + 2.0,
        );
        assert!(hits_create_invoice_input(layout, description_input));
    }
}
