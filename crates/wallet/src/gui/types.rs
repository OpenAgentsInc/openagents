//! Shared types for wallet GUI.

use spark::Balance;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletTab {
    Send,
    Receive,
}

#[derive(Debug, Clone)]
pub enum WalletCommand {
    RefreshBalance,
    RequestReceive { amount: Option<u64> },
    SendPayment { destination: String, amount: Option<u64> },
}

#[derive(Debug, Clone)]
pub enum WalletUpdate {
    Balance(Balance),
    ReceiveReady { payload: String, amount: Option<u64> },
    SendSuccess { payment_id: String },
    Error { message: String },
}
