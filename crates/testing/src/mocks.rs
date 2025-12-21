//! Mock Implementations
//!
//! Provides mock implementations of common traits for testing.

use std::sync::{Arc, Mutex};

/// Mock wallet service for testing
///
/// Allows tests to control wallet behavior without real Bitcoin operations.
pub struct MockWallet {
    balance: Arc<Mutex<u64>>,
    payments: Arc<Mutex<Vec<Payment>>>,
}

impl MockWallet {
    /// Create a new mock wallet with initial balance
    pub fn new(initial_balance: u64) -> Self {
        Self {
            balance: Arc::new(Mutex::new(initial_balance)),
            payments: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get current balance
    pub fn get_balance(&self) -> u64 {
        *self.balance.lock().unwrap()
    }

    /// Record a payment
    pub fn send_payment(&self, address: String, amount: u64) -> Result<String, String> {
        let mut balance = self.balance.lock().unwrap();

        if *balance < amount {
            return Err("Insufficient balance".to_string());
        }

        *balance -= amount;

        let payment = Payment {
            address,
            amount,
            tx_id: format!("mock_tx_{}", uuid::Uuid::new_v4()),
        };

        let tx_id = payment.tx_id.clone();
        self.payments.lock().unwrap().push(payment);

        Ok(tx_id)
    }

    /// Get payment history
    pub fn get_payments(&self) -> Vec<Payment> {
        self.payments.lock().unwrap().clone()
    }
}

/// Recorded payment for testing
#[derive(Clone, Debug)]
pub struct Payment {
    pub address: String,
    pub amount: u64,
    pub tx_id: String,
}

/// Mock relay pool for testing Nostr operations
pub struct MockRelayPool {
    published_events: Arc<Mutex<Vec<String>>>,
    subscriptions: Arc<Mutex<Vec<String>>>,
}

impl MockRelayPool {
    /// Create a new mock relay pool
    pub fn new() -> Self {
        Self {
            published_events: Arc::new(Mutex::new(Vec::new())),
            subscriptions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record an event publication
    pub fn publish(&self, event_id: String) {
        self.published_events.lock().unwrap().push(event_id);
    }

    /// Record a subscription
    pub fn subscribe(&self, filter: String) {
        self.subscriptions.lock().unwrap().push(filter);
    }

    /// Get all published events
    pub fn get_published_events(&self) -> Vec<String> {
        self.published_events.lock().unwrap().clone()
    }

    /// Get all subscriptions
    pub fn get_subscriptions(&self) -> Vec<String> {
        self.subscriptions.lock().unwrap().clone()
    }
}

impl Default for MockRelayPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_wallet_balance() {
        let wallet = MockWallet::new(1000);
        assert_eq!(wallet.get_balance(), 1000);

        wallet.send_payment("addr1".to_string(), 300).unwrap();
        assert_eq!(wallet.get_balance(), 700);
    }

    #[test]
    fn test_mock_wallet_insufficient_balance() {
        let wallet = MockWallet::new(100);
        let result = wallet.send_payment("addr1".to_string(), 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_mock_relay_pool() {
        let pool = MockRelayPool::new();

        pool.publish("event1".to_string());
        pool.publish("event2".to_string());

        let events = pool.get_published_events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0], "event1");
    }
}
