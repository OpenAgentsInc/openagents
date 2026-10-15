#![cfg(all(feature = "supervisor", unix))]
use lnd_sdk::{Client, service::LightningRail, supervisor::Regtest};
use std::time::Duration;

#[test]
#[ignore = "starts isolated Docker regtest nodes; scripts/lnd-sdk-regtest.sh"]
fn invoice_payment_lookup_and_reconnect() {
    let root = std::env::var_os("LND_SDK_REGTEST_ROOT").expect("explicit scratch root required");
    let nodes = Regtest::start(std::path::Path::new(&root)).unwrap();
    // x402 deliberately admits Bitcoin mainnet/testnet, never regtest.
    // An unfunded offline testnet node signs real LND invoices for this check.
    let pay_to = nodes.testnet(&["getinfo"]).unwrap()["identity_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    let reply = nodes
        .testnet(&[
            "addinvoice",
            "--amt_msat",
            "21001",
            "--description_hash",
            &hex::encode([8; 32]),
        ])
        .unwrap();
    let decoded = nostr::x402::decode_invoice(reply["payment_request"].as_str().unwrap()).unwrap();
    assert_eq!(hex::encode(decoded.payee()), pay_to);
    assert_eq!(decoded.amount_msat(), 21001);
    assert_eq!(decoded.description_hash(), [8; 32]);
    let reply = nodes
        .testnet(&[
            "addinvoice",
            "--amt_msat",
            "21001",
            "--memo",
            "inline description",
        ])
        .unwrap();
    assert!(nostr::x402::decode_invoice(reply["payment_request"].as_str().unwrap()).is_err());
    let alice_address = nodes.lncli("alice", &["newaddress", "p2wkh"]).unwrap()["address"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes
        .bitcoin(&["sendtoaddress", &alice_address, "1"])
        .unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            let balance = nodes.lncli("alice", &["walletbalance"])?;
            if balance["confirmed_balance"].as_str().unwrap_or("0") != "0" {
                Ok(())
            } else {
                Err(lnd_sdk::Error::NotSynced)
            }
        })
        .unwrap();
    let bob = nodes.lncli("bob", &["getinfo"]).unwrap()["identity_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    nodes
        .lncli("alice", &["connect", &format!("{bob}@bob:9735")])
        .unwrap();
    nodes
        .lncli(
            "alice",
            &[
                "openchannel",
                "--node_key",
                &bob,
                "--local_amt",
                "500000",
                "--push_amt",
                "200000",
                "--private",
            ],
        )
        .unwrap();
    nodes.mine(6).unwrap();
    nodes
        .wait(|| {
            let info = nodes.lncli("alice", &["getinfo"])?;
            if info["num_active_channels"].as_u64().unwrap_or(0) > 0 {
                Ok(())
            } else {
                Err(lnd_sdk::Error::NotSynced)
            }
        })
        .unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let (config, macaroon) = nodes.config("alice").unwrap();
        let alice = Client::connect(&config, macaroon).await.unwrap();
        let (mut config, macaroon) = nodes.config("bob").unwrap();
        // Docker can change an automatically published port on restart.
        // A stable local forwarder models the production node's fixed address.
        use std::sync::{
            Arc,
            atomic::{AtomicU16, Ordering},
        };
        let backend_port = Arc::new(AtomicU16::new(
            config.endpoint.rsplit(':').next().unwrap().parse().unwrap(),
        ));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_address = listener.local_addr().unwrap();
        let backend = backend_port.clone();
        let proxy = tokio::spawn(async move {
            while let Ok((mut incoming, _)) = listener.accept().await {
                let port = backend.load(Ordering::Relaxed);
                tokio::spawn(async move {
                    if let Ok(mut outgoing) =
                        tokio::net::TcpStream::connect(("127.0.0.1", port)).await
                    {
                        let _ = tokio::io::copy_bidirectional(&mut incoming, &mut outgoing).await;
                    }
                });
            }
        });
        config.endpoint = format!("https://{proxy_address}");
        let bob = Client::connect(&config, macaroon).await.unwrap();
        assert!(alice.balance().await.unwrap().lightning_total_sats > 0);
        let invoice_credential = bob
            .bake_macaroon(lnd_sdk::macaroon::Role::Invoice, 501)
            .await
            .unwrap();
        let invoicer = Client::connect(&config, invoice_credential).await.unwrap();
        let forbidden = alice.create_invoice(2000, [5; 32], 600).await.unwrap();
        assert!(matches!(
            invoicer
                .pay(&forbidden.bolt11, 1000, Duration::from_secs(5))
                .await,
            Err(lnd_sdk::Error::PermissionDenied)
        ));
        for (payer, receiver) in [(&alice, &invoicer), (&bob, &alice)] {
            let hint_deadline = std::time::Instant::now() + Duration::from_secs(30);
            let hints = loop {
                match receiver.private_route_hints().await {
                    Ok(hints) => break hints,
                    Err(lnd_sdk::Error::NotSynced) if std::time::Instant::now() < hint_deadline => {
                        tokio::time::sleep(Duration::from_millis(200)).await
                    }
                    Err(e) => panic!("reading private channel policies: {e}"),
                }
            };
            assert!(!hints.is_empty());
            let issued = receiver
                .create_invoice_with_hints(21001, [8; 32], 600, hints)
                .await
                .unwrap();
            let decoded: lnd_sdk::proto::lnrpc::Invoice = receiver
                .lookup_invoice(openagents_wallet::parse_hash32(&issued.payment_hash).unwrap())
                .await
                .unwrap();
            assert_eq!(decoded.description_hash, vec![8; 32]);
            assert_eq!(decoded.value_msat, 21001);
            let signed: lightning_invoice::Bolt11Invoice = issued.bolt11.parse().unwrap();
            assert_eq!(
                signed.recover_payee_pub_key().to_string(),
                receiver.node_id()
            );
            assert!(!signed.route_hints().is_empty());
            let proof = payer
                .pay(&issued.bolt11, 1000, Duration::from_secs(30))
                .await
                .unwrap();
            let recovered = payer
                .pay(&issued.bolt11, 1000, Duration::from_secs(5))
                .await
                .unwrap();
            assert_eq!(recovered.payment_hash, proof.payment_hash);
            assert_eq!(proof.amount_msat, 21001);
            assert!(proof.fee_msat <= 1000);
            let hash = openagents_wallet::parse_hash32(&proof.payment_hash).unwrap();
            assert_eq!(
                payer.lookup(hash).await.unwrap().unwrap().status,
                openagents_wallet::PaymentStatus::Succeeded
            );
            assert_eq!(receiver.received_msat(hash).await.unwrap(), Some(21001));
            assert_eq!(
                receiver.lookup(hash).await.unwrap().unwrap().status,
                openagents_wallet::PaymentStatus::Succeeded
            );
        }

        // Keep the consumer's checkpoint while a real LND restart cuts its stream.
        #[derive(Default)]
        struct Sink(Vec<u64>);
        impl lnd_sdk::invoices::SettlementSink for Sink {
            fn position(&self) -> lnd_sdk::Result<u64> {
                Ok(self.0.last().copied().unwrap_or(0))
            }
            fn commit(&mut self, i: &lnd_sdk::proto::lnrpc::Invoice) -> lnd_sdk::Result<()> {
                self.0.push(i.settle_index);
                Ok(())
            }
        }
        let mut sink = Sink::default();
        bob.settlements(&mut sink, 1).await.unwrap();
        let hints = bob.private_route_hints().await.unwrap();
        let issued = bob
            .create_invoice_with_hints(31001, [6; 32], 600, hints)
            .await
            .unwrap();
        let watcher = bob.clone();
        let delivery = tokio::spawn(async move {
            watcher.settlements(&mut sink, 1).await?;
            Ok::<_, lnd_sdk::Error>(sink.0)
        });
        tokio::time::sleep(Duration::from_millis(100)).await;
        nodes.command(&["restart", "bob"]).unwrap();
        let (restarted, _) = nodes.config("bob").unwrap();
        assert_eq!(restarted.certificate_sha256, config.certificate_sha256);
        backend_port.store(
            restarted
                .endpoint
                .rsplit(':')
                .next()
                .unwrap()
                .parse()
                .unwrap(),
            Ordering::Relaxed,
        );

        nodes
            .wait(|| nodes.lncli("bob", &["getinfo"]).map(|_| ()))
            .unwrap();
        nodes
            .wait(|| {
                let info = nodes.lncli("bob", &["getinfo"])?;
                if info["num_active_channels"].as_u64().unwrap_or(0) > 0 {
                    Ok(())
                } else {
                    Err(lnd_sdk::Error::NotSynced)
                }
            })
            .unwrap();
        assert_eq!(bob.info().await.unwrap().identity_pubkey, bob.node_id());
        alice
            .pay(&issued.bolt11, 1000, Duration::from_secs(30))
            .await
            .unwrap();
        let indexes = tokio::time::timeout(Duration::from_secs(30), delivery)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(indexes, vec![1, 2]);
        proxy.abort();
    });
}
