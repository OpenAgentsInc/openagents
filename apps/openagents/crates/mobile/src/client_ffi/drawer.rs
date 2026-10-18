//! Pin-move controls derived from the shared Rust workspace order.
use super::CoreClient;

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DrawerPinMove {
    pub after: Option<String>,
    pub before: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct DrawerPinHints {
    pub id: String,
    pub earlier: Option<DrawerPinMove>,
    pub later: Option<DrawerPinMove>,
}
fn hints(ids: &[&str]) -> Vec<DrawerPinHints> {
    ids.iter()
        .enumerate()
        .map(|(at, id)| DrawerPinHints {
            id: (*id).into(),
            earlier: at.checked_sub(1).map(|before| DrawerPinMove {
                after: before.checked_sub(1).map(|after| ids[after].into()),
                before: Some(ids[before].into()),
            }),
            later: ids.get(at + 1).map(|after| DrawerPinMove {
                after: Some((*after).into()),
                before: ids.get(at + 2).map(|before| (*before).into()),
            }),
        })
        .collect()
}
#[uniffi::export]
impl CoreClient {
    /// Present moves as typed intents; `move_pin` remains the write authority.
    pub fn drawer_pin_moves(&self) -> Vec<DrawerPinHints> {
        let workspace = self.client.workspace();
        if !workspace.pins_ready {
            return vec![];
        }
        hints(
            &workspace
                .front
                .pinned
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_move_hints_preserve_the_rust_order_and_handle_both_edges() {
        let moves = hints(&["first", "middle", "last"]);
        assert!(moves[0].earlier.is_none());
        assert!(moves[2].later.is_none());
        assert_eq!(
            moves[1].earlier,
            Some(DrawerPinMove {
                after: None,
                before: Some("first".into())
            })
        );
        assert_eq!(
            moves[1].later,
            Some(DrawerPinMove {
                after: Some("last".into()),
                before: None
            })
        );
        assert!(hints(&[]).is_empty());
        assert_eq!(
            hints(&["only"])[0],
            DrawerPinHints {
                id: "only".into(),
                earlier: None,
                later: None
            }
        );
    }
}
