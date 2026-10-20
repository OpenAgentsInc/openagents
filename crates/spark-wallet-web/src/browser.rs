use openagents_spark::browser::BrowserWallet;
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, prelude::*};
use web_sys::{HtmlButtonElement, HtmlInputElement, HtmlTextAreaElement};
use zeroize::Zeroizing;
struct State {
    wallet: Option<BrowserWallet>,
    busy: bool,
    can_pay: bool,
    uncertain: bool,
}
fn document() -> web_sys::Document {
    web_sys::window().unwrap().document().unwrap()
}
fn text(id: &str, value: &str) {
    if let Some(e) = document().get_element_by_id(id) {
        e.set_text_content(Some(value));
    }
}
fn input(id: &str) -> String {
    document()
        .get_element_by_id(id)
        .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
        .map(|e| e.value())
        .unwrap_or_default()
}
fn enabled(state: &State) {
    for id in ["restore", "refresh", "receive", "review", "pay"] {
        if let Some(e) = document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into::<HtmlButtonElement>().ok())
        {
            e.set_disabled(
                state.busy
                    || if id == "restore" {
                        state.wallet.is_some()
                    } else {
                        state.wallet.is_none()
                            || (id == "pay" && !state.can_pay)
                            || (id == "review" && state.uncertain)
                    },
            );
        }
    }
    for id in ["payment", "amount"] {
        if let Some(input) = document()
            .get_element_by_id(id)
            .and_then(|e| e.dyn_into::<HtmlInputElement>().ok())
        {
            input.set_disabled(state.busy || state.uncertain);
        }
    }
}
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    let root = document()
        .get_element_by_id("wallet")
        .ok_or(JsValue::NULL)?;
    root.set_inner_html(r#"<h1>Restore a wallet in this browser</h1>
<p>The default browser wallet asks your phone to approve payments. Restoring here gives this tab control of your wallet.</p>
<p>Use a private computer you trust. We keep this wallet in this tab only. Closing or forgetting it requires your words again.</p>
<section id="restore-section"><label for="words">12 recovery words</label><textarea id="words" autocomplete="off" autocapitalize="off" spellcheck="false" rows="3"></textarea>
<label><input id="consent" type="checkbox">I want this browser tab to control my wallet.</label><button id="restore" type="button">Restore wallet</button></section>
<p id="status" role="status" aria-live="polite"></p><section id="wallet-section" hidden><h2>Balance</h2><p id="balance">—</p><button id="refresh" type="button" disabled>Refresh</button>
<h2>Receive</h2><label for="amount">Amount (₿)</label><input id="amount" type="number" min="1" step="1"><button id="receive" type="button" disabled>Create invoice</button><p id="invoice" class="request"></p>
<h2>Send</h2><label for="payment">Lightning invoice</label><input id="payment" type="text" autocomplete="off" spellcheck="false"><button id="review" type="button" disabled>Review payment</button><p id="quote"></p><button id="pay" type="button" disabled>Confirm payment</button>
<h2>Recent payments</h2><div id="history"></div></section><button id="forget" type="button">Forget this wallet</button><p><a href="/settings/profile">Back to profile</a></p>"#);
    let state = Rc::new(RefCell::new(State {
        wallet: None,
        busy: false,
        can_pay: false,
        uncertain: false,
    }));
    for id in ["restore", "refresh", "receive", "review", "pay", "forget"] {
        let shared = state.clone();
        let listener = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
            if id == "forget" {
                if let Some(words) = document()
                    .get_element_by_id("words")
                    .and_then(|e| e.dyn_into::<HtmlTextAreaElement>().ok())
                {
                    words.set_value("");
                }
                shared.borrow_mut().wallet.take();
                // Replacing the document ends SDK tasks and drops this Wasm instance, even during a network wait.
                let _ = web_sys::window()
                    .unwrap()
                    .location()
                    .replace("/wallet/restore");
                return;
            }
            if shared.borrow().busy {
                return;
            }
            let words = if id == "restore" {
                let consent = document()
                    .get_element_by_id("consent")
                    .unwrap()
                    .dyn_into::<HtmlInputElement>()
                    .unwrap();
                if !consent.checked() {
                    text("status", "Choose whether to restore in this browser first.");
                    return;
                }
                let field = document()
                    .get_element_by_id("words")
                    .unwrap()
                    .dyn_into::<HtmlTextAreaElement>()
                    .unwrap();
                let words = Zeroizing::new(field.value());
                field.set_value("");
                Some(words)
            } else {
                None
            };
            let amount = input("amount");
            let payment = input("payment");
            let wallet = {
                let mut state = shared.borrow_mut();
                state.busy = true;
                let wallet = state.wallet.take();
                enabled(&state);
                wallet
            };
            text("status", "Working…");
            let shared = shared.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let mut wallet = wallet;
                let result: Result<String, String> = async {
                    if id == "restore" {
                        wallet = Some(BrowserWallet::restore(words.as_ref().unwrap()).await?);
                        document()
                            .get_element_by_id("wallet-section")
                            .unwrap()
                            .remove_attribute("hidden")
                            .ok();
                        document()
                            .get_element_by_id("restore-section")
                            .unwrap()
                            .set_attribute("hidden", "")
                            .ok();
                    }
                    let wallet = wallet.as_mut().ok_or("Restore your wallet first.")?;
                    match id {
                        "receive" => {
                            let amount = amount
                                .parse::<u64>()
                                .map_err(|_| "Choose a whole amount greater than zero.")?;
                            let invoice = wallet.invoice(amount).await?;
                            text("invoice", &invoice);
                            Ok("Invoice ready. Copy it into the wallet that will pay.".into())
                        }
                        "review" => {
                            let quote = wallet.quote(&payment).await?;
                            text(
                                "quote",
                                &format!(
                                    "Send {} · Fee {}",
                                    bitcoin_amount::Format::Bip177.show(quote.amount_sats),
                                    bitcoin_amount::Format::Bip177.show(quote.fee_sats)
                                ),
                            );
                            Ok("Check the amount and fee, then confirm the payment.".into())
                        }
                        "pay" => {
                            let paid = wallet.pay().await?;
                            text("quote", "");
                            Ok(format!(
                                "Payment {}. Refresh to check your history.",
                                paid.status
                            ))
                        }
                        _ => {
                            let balance = wallet.balance().await?;
                            text("balance", &bitcoin_amount::Format::Bip177.show(balance));
                            let payments = wallet.payments().await?;
                            let history = document().get_element_by_id("history").unwrap();
                            history.set_text_content(None);
                            for row in payments {
                                let item = document().create_element("p").unwrap();
                                item.set_text_content(Some(&format!(
                                    "{} {} · {} · {}",
                                    if row.received { "Received" } else { "Sent" },
                                    bitcoin_amount::Format::Bip177.show(row.amount_sats),
                                    row.method,
                                    row.status
                                )));
                                history.append_child(&item).ok();
                            }
                            Ok("Wallet refreshed.".into())
                        }
                    }
                }
                .await;
                let uncertain = wallet.as_ref().is_some_and(|wallet| wallet.uncertain());
                let can_pay = (id == "review" && result.is_ok()) || uncertain;
                text("status", &result.unwrap_or_else(|message| message));
                let mut state = shared.borrow_mut();
                state.wallet = wallet;
                state.busy = false;
                state.can_pay = can_pay;
                state.uncertain = uncertain;
                enabled(&state);
            });
        });
        document()
            .get_element_by_id(id)
            .unwrap()
            .add_event_listener_with_callback("click", listener.as_ref().unchecked_ref())?;
        listener.forget();
    }
    let shared = state.clone();
    let changed = Closure::<dyn FnMut(web_sys::Event)>::new(move |_| {
        let mut state = shared.borrow_mut();
        state.can_pay = false;
        if let Some(wallet) = state.wallet.as_mut() {
            wallet.cancel_quote();
        }
        text("quote", "");
        enabled(&state);
    });
    document()
        .get_element_by_id("payment")
        .unwrap()
        .add_event_listener_with_callback("input", changed.as_ref().unchecked_ref())?;
    changed.forget();
    let resumed = Closure::<dyn FnMut(web_sys::PageTransitionEvent)>::new(
        move |event: web_sys::PageTransitionEvent| {
            if event.persisted() {
                let _ = web_sys::window().unwrap().location().reload();
            }
        },
    );
    web_sys::window()
        .unwrap()
        .add_event_listener_with_callback("pageshow", resumed.as_ref().unchecked_ref())?;
    resumed.forget();
    Ok(())
}
