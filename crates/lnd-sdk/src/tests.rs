use super::*;
use crate::{
    invoices::SettlementSink,
    macaroon::{Macaroon, Role},
    proto::{lnrpc, routerrpc, verrpc},
    service::{LightningRail, Wallet},
};
use bitcoin::hashes::Hash;
use sha2::{Digest, Sha256};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tonic::{Request, Response, Status, codegen::BoxStream};

pub fn invoice_fixture(amount: u64) -> String {
    let key = bitcoin::secp256k1::SecretKey::from_slice(&[42; 32]).unwrap();
    lightning_invoice::InvoiceBuilder::new(lightning_invoice::Currency::BitcoinTestnet)
        .description_hash(bitcoin::hashes::sha256::Hash::from_byte_array([9; 32]))
        .payment_hash(bitcoin::hashes::sha256::Hash::hash(&[7; 32]))
        .payment_secret(lightning_invoice::PaymentSecret([3; 32]))
        .amount_milli_satoshis(amount)
        .current_timestamp()
        .min_final_cltv_expiry_delta(18)
        .build_signed(|m| bitcoin::secp256k1::Secp256k1::new().sign_ecdsa_recoverable(m, &key))
        .unwrap()
        .to_string()
}
#[derive(Default)]
struct Mock {
    minor: u32,
    changed: std::sync::atomic::AtomicBool,
    sends: AtomicUsize,
    subscriptions: Mutex<Vec<u64>>,
    reads: AtomicUsize,
}
#[tonic::async_trait]
impl lnrpc::lightning_server::Lightning for Mock {
    async fn get_info(
        &self,
        r: Request<lnrpc::GetInfoRequest>,
    ) -> std::result::Result<Response<lnrpc::GetInfoResponse>, Status> {
        assert_eq!(
            r.metadata().get("macaroon").unwrap(),
            "7175657565642d746573742d63726564656e7469616c"
        );
        self.reads.fetch_add(1, Ordering::SeqCst);
        #[allow(deprecated)]
        Ok(Response::new(lnrpc::GetInfoResponse {
            identity_pubkey: format!(
                "02{}",
                if self.changed.load(Ordering::SeqCst) {
                    "22"
                } else {
                    "11"
                }
                .repeat(32)
            ),
            chains: vec![lnrpc::Chain {
                chain: "bitcoin".into(),
                network: "testnet".into(),
            }],
            ..Default::default()
        }))
    }
    async fn list_channels(
        &self,
        _: Request<lnrpc::ListChannelsRequest>,
    ) -> std::result::Result<Response<lnrpc::ListChannelsResponse>, Status> {
        Ok(Response::new(lnrpc::ListChannelsResponse {
            channels: vec![lnrpc::Channel {
                chan_id: 1,
                active: true,
                private: true,
                ..Default::default()
            }],
            ..Default::default()
        }))
    }
    async fn get_chan_info(
        &self,
        _: Request<lnrpc::ChanInfoRequest>,
    ) -> std::result::Result<Response<lnrpc::ChannelEdge>, Status> {
        Err(Status::not_found("channel update is not in the graph yet"))
    }
    async fn list_invoices(
        &self,
        _: Request<lnrpc::ListInvoiceRequest>,
    ) -> std::result::Result<Response<lnrpc::ListInvoiceResponse>, Status> {
        Ok(Response::new(lnrpc::ListInvoiceResponse {
            invoices: vec![lnrpc::Invoice {
                state: 1,
                settle_index: 1,
                ..Default::default()
            }],
            ..Default::default()
        }))
    }
    async fn subscribe_invoices(
        &self,
        r: Request<lnrpc::InvoiceSubscription>,
    ) -> std::result::Result<Response<BoxStream<lnrpc::Invoice>>, Status> {
        let cursor = r.into_inner().settle_index;
        self.subscriptions.lock().unwrap().push(cursor);
        let items = if cursor == 1 {
            vec![
                Ok(lnrpc::Invoice {
                    state: 1,
                    settle_index: 2,
                    ..Default::default()
                }),
                Err(Status::unavailable("cut stream")),
            ]
        } else {
            vec![
                Ok(lnrpc::Invoice {
                    state: 1,
                    settle_index: 1,
                    ..Default::default()
                }),
                Ok(lnrpc::Invoice {
                    state: 1,
                    settle_index: 2,
                    ..Default::default()
                }),
                Ok(lnrpc::Invoice {
                    state: 1,
                    settle_index: 3,
                    ..Default::default()
                }),
            ]
        };
        Ok(Response::new(Box::pin(tokio_stream::iter(items))))
    }
}
#[tonic::async_trait]
impl verrpc::versioner_server::Versioner for Mock {
    async fn get_version(
        &self,
        _: Request<verrpc::VersionRequest>,
    ) -> std::result::Result<Response<verrpc::Version>, Status> {
        Ok(Response::new(verrpc::Version {
            app_major: 0,
            app_minor: self.minor,
            app_patch: 1,
            ..Default::default()
        }))
    }
}
#[tonic::async_trait]
impl routerrpc::router_server::Router for Mock {
    async fn send_payment_v2(
        &self,
        r: Request<routerrpc::SendPaymentRequest>,
    ) -> std::result::Result<Response<BoxStream<lnrpc::Payment>>, Status> {
        assert_eq!(r.into_inner().fee_limit_msat, 17);
        self.sends.fetch_add(1, Ordering::SeqCst);
        Err(Status::unavailable("interrupted after send"))
    }
    async fn track_payment_v2(
        &self,
        r: Request<routerrpc::TrackPaymentRequest>,
    ) -> std::result::Result<Response<BoxStream<lnrpc::Payment>>, Status> {
        let hash = hex::encode(r.into_inner().payment_hash);
        let p = lnrpc::Payment {
            payment_hash: hash,
            payment_preimage: hex::encode([7; 32]),
            status: 2,
            value_msat: 1000,
            fee_msat: 5,
            ..Default::default()
        };
        Ok(Response::new(Box::pin(tokio_stream::iter([Ok(p)]))))
    }
}
fn credential() -> Macaroon {
    Macaroon::new(Role::Payout, 7, b"queued-test-credential".to_vec()).unwrap()
}
async fn server(minor: u32) -> (transport::Config, Arc<Mock>, tokio::task::JoinHandle<()>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let fingerprint = Sha256::digest(cert.cert.der()).into();
    let identity =
        tonic::transport::Identity::from_pem(cert.cert.pem(), cert.signing_key.serialize_pem());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let mock = Arc::new(Mock {
        minor,
        ..Default::default()
    });
    let service = mock.clone();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(tonic::transport::ServerTlsConfig::new().identity(identity))
            .unwrap()
            .add_service(lnrpc::lightning_server::LightningServer::from_arc(
                service.clone(),
            ))
            .add_service(verrpc::versioner_server::VersionerServer::from_arc(
                service.clone(),
            ))
            .add_service(routerrpc::router_server::RouterServer::from_arc(service))
            .serve_with_incoming(tokio_stream::wrappers::TcpListenerStream::new(listener))
            .await
            .unwrap();
    });
    (
        transport::Config::new(format!("https://localhost:{port}"), fingerprint),
        mock,
        task,
    )
}
#[tokio::test]
async fn pinned_tls_credentials_version_and_single_send_recovery() {
    let (mut config, mock, task) = server(21).await;
    let client = Client::connect(&config, credential()).await.unwrap();
    assert_eq!(
        client
            .send_payment(&invoice_fixture(1000), 17, Duration::from_secs(3))
            .await,
        Err(Error::Unavailable)
    );
    assert_eq!(mock.sends.load(Ordering::SeqCst), 1);
    let hash = Sha256::digest([7; 32]).into();
    assert_eq!(
        client.lookup(hash).await.unwrap().unwrap().status,
        openagents_wallet::PaymentStatus::Succeeded
    );
    assert_eq!(mock.sends.load(Ordering::SeqCst), 1);
    mock.changed.store(true, Ordering::SeqCst);
    assert_eq!(
        client
            .send_payment(&invoice_fixture(1000), 17, Duration::from_secs(2))
            .await,
        Err(Error::NodeMismatch)
    );
    assert_eq!(mock.sends.load(Ordering::SeqCst), 1);
    config.certificate_sha256 = [0; 32];
    assert!(matches!(
        Client::connect(&config, credential()).await,
        Err(Error::CertificatePin)
    ));
    task.abort();
    let (config, _, task) = server(20).await;
    assert!(matches!(
        Client::connect(&config, credential()).await,
        Err(Error::VersionMismatch)
    ));
    task.abort();
}
#[tokio::test]
async fn active_private_channel_waits_for_its_graph_entry() {
    let (config, _, task) = server(21).await;
    let client = Client::connect(&config, credential()).await.unwrap();
    assert_eq!(client.private_route_hints().await, Err(Error::NotSynced));
    task.abort();
}
#[derive(Default)]
struct Sink(Vec<u64>);
impl SettlementSink for Sink {
    fn position(&self) -> Result<u64> {
        Ok(self.0.last().copied().unwrap_or(0))
    }
    fn commit(&mut self, i: &lnrpc::Invoice) -> Result<()> {
        self.0.push(i.settle_index);
        Ok(())
    }
}
#[tokio::test]
async fn interrupted_settlement_stream_resumes_and_deduplicates() {
    let (config, mock, task) = server(21).await;
    let client = Client::connect(&config, credential()).await.unwrap();
    let mut sink = Sink::default();
    tokio::time::timeout(Duration::from_secs(5), client.settlements(&mut sink, 3))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sink.0, vec![1, 2, 3]);
    assert_eq!(*mock.subscriptions.lock().unwrap(), vec![1, 2]);
    task.abort();
}
#[test]
fn credentials_and_status_errors_do_not_disclose_bytes() {
    assert_eq!(format!("{}", credential()), "macaroon(Payout, 7)");
    assert!(!format!("{:?}", credential()).contains("queued-test"));
    let e = Error::from(Status::permission_denied("must not appear"));
    assert_eq!(e, Error::PermissionDenied);
    assert_eq!(
        Error::from(Status::unknown("permission denied")),
        Error::PermissionDenied
    );
    assert!(!e.to_string().contains("must not appear"));
}
#[tokio::test]
async fn blocking_adapter_can_be_called_from_an_async_runtime() {
    use openagents_wallet::LightningWallet;
    let fake = Arc::new(fake::Fake::default());
    let wallet = Wallet::new(fake).unwrap();
    assert_eq!(wallet.balance().unwrap().onchain_total_sats, 0);
    drop(wallet);
}
proptest::proptest! {
    #[test]
    fn signed_amount_conversion_is_checked(amount in proptest::prelude::any::<u64>()) {
        assert_eq!(crate::error::signed(amount,"amount").ok(),i64::try_from(amount).ok());
    }
    #[test]
    fn every_rpc_code_has_a_bounded_error(code in 0i32..1000){
        let error=Error::from(Status::new(tonic::Code::from_i32(code),"secret"));assert!(!error.to_string().contains("secret"));
    }
}

struct Delegate;
impl pay_ledger::payout::Rails for Delegate {
    fn lightning_invoice(
        &self,
        _: &str,
        amount_msat: i64,
    ) -> std::result::Result<pay_ledger::payout::Invoice, String> {
        let bolt11 = invoice_fixture(amount_msat.try_into().unwrap());
        Ok(pay_ledger::payout::Invoice {
            payment_hash: crate::router::invoice(&bolt11)
                .unwrap()
                .payment_hash()
                .to_string(),
            bolt11,
            amount_msat,
        })
    }
    fn pay_lightning(
        &self,
        _: &pay_ledger::payout::Invoice,
        _: i64,
    ) -> pay_ledger::payout::Outcome {
        panic!("the old rail must not send")
    }
    fn lookup_lightning(&self, _: &str) -> std::result::Result<pay_ledger::payout::Lookup, String> {
        panic!("the old rail must not recover an LND send")
    }
    fn fund_spark(&self, _: u64) -> std::result::Result<(), String> {
        panic!("no Spark payment")
    }
    fn pay_spark(&self, _: &str, _: u64, _: &str) -> pay_ledger::payout::Outcome {
        panic!("no Spark payment")
    }
    fn lookup_spark(&self, _: &str) -> std::result::Result<pay_ledger::payout::Lookup, String> {
        panic!("no Spark payment")
    }
}
#[test]
fn payout_worker_recovers_an_unknown_send_without_sending_again() {
    use openagents_wallet::{PaymentDirection, PaymentRecord, PaymentStatus};
    use pay_ledger::payout::{Policy, tick};
    use pay_ledger::{Ledger, Payee, PayoutState, Rail, SettlementInput, Split};
    const START: i64 = 1_792_022_400;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    for (key, at) in [("h1", 100), ("h2", 101)] {
        ledger
            .record_settlement(SettlementInput {
                key: key.into(),
                resource: "/v1/plugins/demo/invoke".into(),
                plugin_id: Some("test".into()),
                release_id: None,
                price_msat: 605_000,
                received_msat: 605_000,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: START + at,
                split: Split::Plugin {
                    author: "test-payee".into(),
                    fee_msat: 600_000,
                },
            })
            .unwrap();
    }
    ledger
        .register_payee(Payee {
            party: "test-payee".into(),
            destination_kind: "lud16".into(),
            destination_value: "test@example.com".into(),
            source: "test".into(),
            verified_at: START,
        })
        .unwrap();
    let fake = Arc::new(fake::Fake::default());
    let rails = service::Rails {
        wallet: Wallet::new(fake.clone()).unwrap(),
        delegate: Delegate,
        payment_timeout: Duration::from_secs(2),
    };
    let mut next = 0;
    let mut run = |ledger: &mut Ledger, now: i64| {
        tick(
            ledger,
            &rails,
            &Policy::default(),
            now,
            &mut |l, party, _| l.payee(party),
            &mut || {
                next += 1;
                format!("00000000-0000-4000-8000-{next:012x}")
            },
        )
        .unwrap()
    };
    run(&mut ledger, START + 200);
    assert_eq!(ledger.payouts(None).unwrap()[0].state, PayoutState::Unknown);
    drop(ledger);
    let mut ledger = Ledger::open(&path).unwrap();
    run(&mut ledger, START + 260);
    assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
    let hash = Sha256::digest([7; 32]).into();
    fake.records.lock().unwrap().insert(
        hash,
        PaymentRecord {
            payment_hash: hex::encode(hash),
            direction: PaymentDirection::Outbound,
            status: PaymentStatus::Succeeded,
            amount_msat: Some(ledger.payouts(None).unwrap()[0].amount_msat as u64),
            fee_msat: Some(3),
            preimage: None,
            bolt11: None,
            updated_at: 270,
        },
    );
    run(&mut ledger, START + 320);
    assert_eq!(ledger.payouts(None).unwrap()[0].state, PayoutState::Sent);
    assert_eq!(fake.sends.load(Ordering::SeqCst), 1);
}

#[test]
fn a_succeeded_status_needs_a_valid_preimage_and_nonnegative_amounts() {
    let hash = hex::encode(Sha256::digest([7; 32]));
    let mut p = lnrpc::Payment {
        payment_hash: hash,
        payment_preimage: hex::encode([8; 32]),
        status: 2,
        value_msat: 1000,
        fee_msat: 1,
        ..Default::default()
    };
    assert_eq!(
        crate::router::record(&p),
        Err(Error::Invalid("payment proof"))
    );
    p.payment_preimage = hex::encode([7; 32]);
    p.fee_msat = -1;
    assert_eq!(
        crate::router::record(&p),
        Err(Error::Invalid("negative amount"))
    );
}

proptest::proptest! {
    #[test]
    fn settlement_replays_never_repeat_committed_effects(mut events in proptest::collection::vec(1u64..1000,0..100),saved in 0u64..1000){
        events.sort_unstable();let mut sink=Sink(vec![saved]);
        for index in &events {
            let invoice=lnrpc::Invoice{state:1,settle_index:*index,..Default::default()};
            crate::invoices::deliver(&mut sink,&invoice).unwrap();
            // A reconnect can replay the last event after every successful commit.
            crate::invoices::deliver(&mut sink,&invoice).unwrap();
        }
        events.retain(|i|*i>saved);events.dedup();
        assert_eq!(&sink.0[1..],events.as_slice());
    }
}
