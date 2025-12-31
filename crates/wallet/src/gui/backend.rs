//! Backend worker for wallet GUI commands.

use anyhow::{Context, Result};
use spark::{Network, SparkSigner, SparkWallet, WalletConfig};
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::storage::identities::{DEFAULT_IDENTITY_NAME, current_identity};
use crate::storage::keychain::{SecureKeychain, WALLET_PASSWORD_ENV};

use super::types::{WalletCommand, WalletUpdate};

pub struct WalletBackendHandle {
    pub sender: UnboundedSender<WalletCommand>,
    pub receiver: UnboundedReceiver<WalletUpdate>,
}

impl WalletBackendHandle {
    pub fn split(
        self,
    ) -> (
        UnboundedSender<WalletCommand>,
        UnboundedReceiver<WalletUpdate>,
    ) {
        (self.sender, self.receiver)
    }
}

pub fn start_backend(handle: tokio::runtime::Handle) -> WalletBackendHandle {
    let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
    let (update_tx, update_rx) = mpsc::unbounded_channel();

    handle.spawn(async move {
        if let Err(err) = run_backend(cmd_rx, update_tx.clone()).await {
            let _ = update_tx.send(WalletUpdate::Error {
                message: err.to_string(),
            });
        }
    });

    WalletBackendHandle {
        sender: cmd_tx,
        receiver: update_rx,
    }
}

async fn run_backend(
    mut cmd_rx: UnboundedReceiver<WalletCommand>,
    update_tx: UnboundedSender<WalletUpdate>,
) -> Result<()> {
    let mut wallet = match load_wallet().await {
        Ok(wallet) => {
            send_initial(&wallet, &update_tx).await;
            Some(wallet)
        }
        Err(err) => {
            let _ = update_tx.send(WalletUpdate::Error {
                message: err.to_string(),
            });
            None
        }
    };

    while let Some(cmd) = cmd_rx.recv().await {
        if wallet.is_none() {
            wallet = match load_wallet().await {
                Ok(wallet) => {
                    send_initial(&wallet, &update_tx).await;
                    Some(wallet)
                }
                Err(err) => {
                    let _ = update_tx.send(WalletUpdate::Error {
                        message: err.to_string(),
                    });
                    None
                }
            };
        }

        let Some(wallet_ref) = wallet.as_ref() else {
            continue;
        };

        match cmd {
            WalletCommand::RefreshBalance => match wallet_ref.get_balance().await {
                Ok(balance) => {
                    let _ = update_tx.send(WalletUpdate::Balance(balance));
                }
                Err(err) => {
                    let _ = update_tx.send(WalletUpdate::Error {
                        message: err.to_string(),
                    });
                }
            },
            WalletCommand::RequestReceive { amount } => {
                if let Some(sats) = amount {
                    match wallet_ref.create_invoice(sats, None, None).await {
                        Ok(response) => {
                            let _ = update_tx.send(WalletUpdate::ReceiveReady {
                                payload: response.payment_request,
                                amount: Some(sats),
                            });
                        }
                        Err(err) => {
                            let _ = update_tx.send(WalletUpdate::Error {
                                message: err.to_string(),
                            });
                        }
                    }
                } else {
                    match wallet_ref.get_spark_address().await {
                        Ok(address) => {
                            let _ = update_tx.send(WalletUpdate::ReceiveReady {
                                payload: address,
                                amount: None,
                            });
                        }
                        Err(err) => {
                            let _ = update_tx.send(WalletUpdate::Error {
                                message: err.to_string(),
                            });
                        }
                    }
                }
            }
            WalletCommand::SendPayment {
                destination,
                amount,
            } => match wallet_ref.send_payment_simple(&destination, amount).await {
                Ok(response) => {
                    let _ = update_tx.send(WalletUpdate::SendSuccess {
                        payment_id: response.payment.id,
                    });
                    if let Ok(balance) = wallet_ref.get_balance().await {
                        let _ = update_tx.send(WalletUpdate::Balance(balance));
                    }
                }
                Err(err) => {
                    let _ = update_tx.send(WalletUpdate::Error {
                        message: err.to_string(),
                    });
                }
            },
            WalletCommand::LoadPayments { offset, limit } => {
                match wallet_ref.list_payments(Some(limit), Some(offset)).await {
                    Ok(payments) => {
                        let has_more = payments.len() as u32 == limit;
                        let _ = update_tx.send(WalletUpdate::PaymentsLoaded {
                            payments,
                            offset,
                            has_more,
                        });
                    }
                    Err(err) => {
                        let _ = update_tx.send(WalletUpdate::Error {
                            message: err.to_string(),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

async fn send_initial(wallet: &SparkWallet, update_tx: &UnboundedSender<WalletUpdate>) {
    if let Ok(balance) = wallet.get_balance().await {
        let _ = update_tx.send(WalletUpdate::Balance(balance));
    }

    match wallet.get_spark_address().await {
        Ok(address) => {
            let _ = update_tx.send(WalletUpdate::ReceiveReady {
                payload: address,
                amount: None,
            });
        }
        Err(err) => {
            let _ = update_tx.send(WalletUpdate::Error {
                message: err.to_string(),
            });
        }
    }
}

async fn load_wallet() -> Result<SparkWallet> {
    let identity_name = current_identity().unwrap_or_else(|_| DEFAULT_IDENTITY_NAME.to_string());
    let mnemonic = load_mnemonic_for(identity_name).await?;

    let signer = SparkSigner::from_mnemonic(&mnemonic, "")
        .map_err(|err| anyhow::anyhow!(err.to_string()))?;

    let network = if std::env::var("MAINNET").is_ok() {
        Network::Mainnet
    } else {
        Network::Regtest
    };

    let config = WalletConfig {
        network,
        api_key: std::env::var("BREEZ_API_KEY").ok(),
        ..Default::default()
    };

    SparkWallet::new(signer, config)
        .await
        .context("Failed to connect to Spark network")
}

async fn load_mnemonic_for(identity: String) -> Result<String> {
    tokio::task::spawn_blocking(move || {
        if !SecureKeychain::has_mnemonic_for(&identity) {
            anyhow::bail!(
                "No wallet found for identity '{}'. Run 'openagents wallet init' first.",
                identity
            );
        }

        if SecureKeychain::is_password_protected_for(&identity) {
            let password = std::env::var(WALLET_PASSWORD_ENV).map_err(|_| {
                anyhow::anyhow!(
                    "Wallet is password protected. Set {} to unlock.",
                    WALLET_PASSWORD_ENV
                )
            })?;
            return SecureKeychain::retrieve_mnemonic_with_password_for(&identity, &password);
        }

        SecureKeychain::retrieve_mnemonic_for(&identity)
    })
    .await
    .context("Failed to access keychain")?
}
