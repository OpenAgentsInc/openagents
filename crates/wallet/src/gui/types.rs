//! Shared types for wallet GUI.

use spark::{Balance, Payment};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletTab {
    Send,
    Receive,
    History,
}

#[derive(Debug, Clone)]
pub enum WalletCommand {
    RefreshBalance,
    RequestReceive {
        amount: Option<u64>,
    },
    SendPayment {
        destination: String,
        amount: Option<u64>,
    },
    LoadPayments {
        offset: u32,
        limit: u32,
    },
}

#[derive(Debug, Clone)]
pub enum WalletUpdate {
    Balance(Balance),
    ReceiveReady {
        payload: String,
        amount: Option<u64>,
    },
    SendSuccess {
        payment_id: String,
    },
    PaymentsLoaded {
        payments: Vec<Payment>,
        offset: u32,
        has_more: bool,
    },
    Error {
        message: String,
    },
}
