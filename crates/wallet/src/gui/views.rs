//! Maud/HTMX views for Wallet GUI

use maud::{html, Markup, DOCTYPE};

/// Base layout template
fn layout(title: &str, content: Markup) -> String {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " - OpenAgents Wallet" }
                script src="https://unpkg.com/htmx.org@1.9.10" {}
                style {
                    r#"
                    :root {
                        --bg-primary: #1a1a1a;
                        --bg-secondary: #2a2a2a;
                        --text-primary: #e0e0e0;
                        --text-secondary: #a0a0a0;
                        --accent: #3b82f6;
                        --border: #3a3a3a;
                        --success: #22c55e;
                        --error: #ef4444;
                    }

                    * {
                        margin: 0;
                        padding: 0;
                        box-sizing: border-box;
                    }

                    body {
                        font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
                        background: var(--bg-primary);
                        color: var(--text-primary);
                        line-height: 1.6;
                    }

                    .container {
                        max-width: 1200px;
                        margin: 0 auto;
                        padding: 20px;
                    }

                    header {
                        background: var(--bg-secondary);
                        border-bottom: 1px solid var(--border);
                        padding: 20px 0;
                        margin-bottom: 30px;
                    }

                    nav {
                        display: flex;
                        gap: 20px;
                        align-items: center;
                    }

                    nav h1 {
                        font-size: 24px;
                        margin-right: auto;
                    }

                    nav a {
                        color: var(--text-secondary);
                        text-decoration: none;
                        padding: 8px 16px;
                        transition: color 0.2s;
                    }

                    nav a:hover {
                        color: var(--accent);
                    }

                    .card {
                        background: var(--bg-secondary);
                        border: 1px solid var(--border);
                        padding: 24px;
                        margin-bottom: 20px;
                    }

                    .card h2 {
                        margin-bottom: 16px;
                        font-size: 20px;
                    }

                    .stat-grid {
                        display: grid;
                        grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
                        gap: 16px;
                        margin-bottom: 20px;
                    }

                    .stat {
                        background: var(--bg-primary);
                        border: 1px solid var(--border);
                        padding: 16px;
                    }

                    .stat-label {
                        color: var(--text-secondary);
                        font-size: 14px;
                        margin-bottom: 8px;
                    }

                    .stat-value {
                        font-size: 24px;
                        font-weight: 600;
                        color: var(--success);
                    }

                    .form-group {
                        margin-bottom: 16px;
                    }

                    label {
                        display: block;
                        margin-bottom: 8px;
                        color: var(--text-secondary);
                    }

                    input, textarea {
                        width: 100%;
                        padding: 12px;
                        background: var(--bg-primary);
                        border: 1px solid var(--border);
                        color: var(--text-primary);
                        font-size: 14px;
                    }

                    input:focus, textarea:focus {
                        outline: none;
                        border-color: var(--accent);
                    }

                    button {
                        background: var(--accent);
                        color: white;
                        border: none;
                        padding: 12px 24px;
                        font-size: 14px;
                        cursor: pointer;
                        transition: opacity 0.2s;
                    }

                    button:hover {
                        opacity: 0.9;
                    }

                    button:disabled {
                        opacity: 0.5;
                        cursor: not-allowed;
                    }

                    .transaction-list {
                        list-style: none;
                    }

                    .transaction-item {
                        background: var(--bg-primary);
                        border: 1px solid var(--border);
                        padding: 16px;
                        margin-bottom: 8px;
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                    }

                    .relay-list {
                        list-style: none;
                    }

                    .relay-item {
                        background: var(--bg-primary);
                        border: 1px solid var(--border);
                        padding: 12px;
                        margin-bottom: 8px;
                        font-family: monospace;
                    }

                    code {
                        background: var(--bg-primary);
                        padding: 2px 6px;
                        font-family: monospace;
                        color: var(--accent);
                    }
                    "#
                }
            }
            body {
                header {
                    div .container {
                        nav {
                            h1 { "OpenAgents Wallet" }
                            a href="/" { "Dashboard" }
                            a href="/send" { "Send" }
                            a href="/receive" { "Receive" }
                            a href="/history" { "History" }
                            a href="/settings" { "Settings" }
                        }
                    }
                }
                main .container {
                    (content)
                }
            }
        }
    }.into_string()
}

/// Dashboard page
pub fn dashboard_page(npub: &str, display_name: &str, balance_sats: u64) -> String {
    let content = html! {
        div .card {
            h2 { "Identity" }
            div {
                p { strong { "Name: " } (display_name) }
                p { strong { "Nostr Public Key: " } code { (npub) } }
            }
        }

        div .stat-grid {
            div .stat {
                div .stat-label { "Total Balance" }
                div .stat-value { (balance_sats) " sats" }
            }
            div .stat {
                div .stat-label { "Spark L2" }
                div .stat-value { "0 sats" }
            }
            div .stat {
                div .stat-label { "Lightning" }
                div .stat-value { "0 sats" }
            }
            div .stat {
                div .stat-label { "On-chain" }
                div .stat-value { "0 sats" }
            }
        }

        div .card {
            h2 { "Quick Actions" }
            div style="display: flex; gap: 12px;" {
                a href="/send" {
                    button { "Send Payment" }
                }
                a href="/receive" {
                    button { "Receive Payment" }
                }
                a href="/history" {
                    button { "View History" }
                }
            }
        }
    };

    layout("Dashboard", content)
}

/// Send payment page
pub fn send_page() -> String {
    let content = html! {
        div .card {
            h2 { "Send Payment" }
            form method="post" action="/send" {
                div .form-group {
                    label for="address" { "Destination Address" }
                    input type="text" id="address" name="address" placeholder="Bitcoin address, Lightning invoice, or Spark address" required;
                }
                div .form-group {
                    label for="amount" { "Amount (sats)" }
                    input type="number" id="amount" name="amount" placeholder="1000" min="1" required;
                }
                button type="submit" { "Send Payment" }
            }
        }
    };

    layout("Send Payment", content)
}

/// Receive payment page
#[allow(dead_code)]
pub fn receive_page(address: &str) -> String {
    let content = html! {
        div .card {
            h2 { "Receive Payment" }
            div .form-group {
                label { "Your Spark Address" }
                code style="display: block; padding: 12px; background: var(--bg-primary);" {
                    (address)
                }
            }
            p style="color: var(--text-secondary); margin-top: 16px;" {
                "Share this address to receive payments via Spark L2"
            }
        }

        div .card {
            h2 { "Generate Lightning Invoice" }
            form method="post" action="/invoice" {
                div .form-group {
                    label for="invoice_amount" { "Amount (sats)" }
                    input type="number" id="invoice_amount" name="amount" placeholder="1000" min="1" required;
                }
                div .form-group {
                    label for="description" { "Description (optional)" }
                    input type="text" id="description" name="description" placeholder="Payment for...";
                }
                button type="submit" { "Generate Invoice" }
            }
        }
    };

    layout("Receive Payment", content)
}

/// Transaction history page
pub fn history_page(transactions: &[Transaction]) -> String {
    let content = html! {
        div .card {
            h2 { "Transaction History" }
            @if transactions.is_empty() {
                p style="color: var(--text-secondary);" { "No transactions yet" }
            } @else {
                ul .transaction-list {
                    @for tx in transactions {
                        li .transaction-item {
                            div {
                                div { strong { (tx.tx_type) } }
                                div style="color: var(--text-secondary); font-size: 14px;" {
                                    (tx.timestamp)
                                }
                            }
                            div style="text-align: right;" {
                                div {
                                    @if tx.amount_sats > 0 {
                                        span style="color: var(--success);" { "+" (tx.amount_sats) " sats" }
                                    } @else {
                                        span style="color: var(--error);" { (tx.amount_sats) " sats" }
                                    }
                                }
                                div style="color: var(--text-secondary); font-size: 14px;" {
                                    (tx.status)
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    layout("Transaction History", content)
}

/// Settings page
pub fn settings_page(relays: &[String]) -> String {
    let content = html! {
        div .card {
            h2 { "Nostr Relays" }
            ul .relay-list {
                @for relay in relays {
                    li .relay-item { (relay) }
                }
            }
            form method="post" action="/settings/relays" style="margin-top: 16px;" {
                div .form-group {
                    label for="relay_url" { "Add Relay" }
                    input type="text" id="relay_url" name="relay_url" placeholder="wss://relay.example.com";
                }
                button type="submit" { "Add Relay" }
            }
        }

        div .card {
            h2 { "Network" }
            p { strong { "Bitcoin Network: " } "Mainnet" }
            p style="color: var(--text-secondary); margin-top: 8px;" {
                "Change network settings via config file"
            }
        }
    };

    layout("Settings", content)
}

/// Transaction data structure (for now - will integrate with actual Spark transactions)
#[allow(dead_code)]
pub struct Transaction {
    pub tx_type: String,
    pub timestamp: String,
    pub amount_sats: i64,
    pub status: String,
}
