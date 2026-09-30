//! `DSK-04`: a phone nearby wants to connect.
//!
//! A phone on the same Wi-Fi found this computer and asked to connect. Both
//! screens show the same six-digit code; the person checks that they match
//! and clicks **Connect**. Nothing is connected without that click. The
//! prompt shows over every other screen until it is answered, the phone
//! gives up, or two minutes pass. **Connect** grants the same rights as a
//! QR pairing: everything an owner's phone uses.

use super::{bold, button, centered, quiet, stack, text};
use crate::control::NearbyPrompt;
use crate::model::Intent;
use rust_native::style::{Space, TextAlign};
use rust_native::{Axis, Node, TextRole};

/// The code in two groups of three, as the phone shows it: `482 913`.
pub fn grouped(code: &str) -> String {
    if code.len() == 6 && code.bytes().all(|b| b.is_ascii_digit()) {
        format!("{} {}", &code[..3], &code[3..])
    } else {
        code.to_owned()
    }
}

/// The phone's name as it gave it, or a plain word when it gave none.
fn name(prompt: &NearbyPrompt) -> &str {
    if prompt.label.trim().is_empty() {
        "A phone"
    } else {
        &prompt.label
    }
}

/// The prompt.
pub fn prompt(prompt: &NearbyPrompt) -> Node<Intent> {
    let id = prompt.id;
    let mut code = centered(bold("code", grouped(&prompt.code)));
    code.element = rust_native::Element::Text {
        value: grouped(&prompt.code),
        role: TextRole::Heading,
    };
    let children = vec![
        centered(text(
            "asks",
            format!("{} wants to connect.", name(prompt)),
            TextRole::Heading,
        )),
        centered(text(
            "check",
            "Check that your phone shows this code:",
            TextRole::Body,
        )),
        code,
        centered(text(
            "only",
            "Connect only if the codes match.",
            TextRole::Status,
        )),
        stack(
            "answers",
            Axis::Horizontal,
            Space::Sm,
            vec![
                quiet("decline", "Don't connect", Intent::NearbyDecline { id }),
                button("connect", "Connect", Intent::NearbyConnect { id }, true),
            ],
        ),
    ];
    let mut root = stack("nearby", Axis::Vertical, Space::Md, children);
    root.style.align = Some(TextAlign::Center);
    root
}
