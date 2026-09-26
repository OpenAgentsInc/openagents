//! Evidence for the pinned Block NIP lane.
//!
//! The source inventory names all pinned specifications. Behavior fixtures
//! establish only their named subset, not complete client or relay conformance.
//! Updating the source pin never upgrades a fixture's behavioral coverage.

/// The Block lane commit recorded in `nips/manifest.json`.
pub const BLOCK_COMMIT: &str = "781d39510cf23cfe224e8f521ae06a23377e06de";

/// Source revision against which the original fifteen subset fixtures were
/// written. New modules carry separate behavior tests for later additions.
pub const BASELINE_FIXTURE_COMMIT: &str = "8342dfcc5890b81a269a8ec3db73a8a56f76ce79";

/// Specification files at that commit, excluding the local index.
pub static FILES: &[(&str, &str)] = &[
    ("NIP-AA.md", "NIP-42"),
    ("NIP-AE.md", "30174"),
    ("NIP-AM.md", "44200"),
    ("NIP-AO.md", "24200"),
    ("NIP-AP.md", "30175"),
    ("NIP-CW.md", "39006"),
    ("NIP-DV.md", "30622"),
    ("NIP-ER.md", "30300"),
    ("NIP-FI.md", "nip-fi+jwt"),
    ("NIP-GS.md", "nostr:git:v1:"),
    ("NIP-IA.md", "9035"),
    ("NIP-MP.md", "30621"),
    ("NIP-OA.md", "nostr:agent-auth:"),
    ("NIP-PL.md", "30350"),
    ("NIP-PMA.md", "30179"),
    ("NIP-RS.md", "30078"),
    ("NIP-WP.md", "9033"),
];

#[cfg(test)]
mod tests {
    use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
    use serde_json::{Value, json};
    use sha2::{Digest, Sha256};

    use super::*;
    use crate::channel_window::{accept_window, parse_window, render_window};
    use crate::domain::{
        Event, EventClass, RelaySigner, Tag, agent_observer_route, agent_turn_metric_owner,
        dm_visibility_channel, parse_identity_archive_request, validate_block_ingest,
        verify_agent_auth_attestation, verify_owner_attestation, workspace_icon,
    };
    use crate::git_sign::{sign_git_object, verify_git_object};
    use crate::nip44::{conversation_key, encrypt};
    use crate::push_lease::{
        LeaseLimits, PushDescriptor, accept_lease, application_body, author_may_read,
        descriptor_document, lease_matches, open_lease, validate_descriptor,
    };
    use crate::run::validate_block_refs;

    #[test]
    fn source_inventory_matches_the_current_manifest_independently_of_behavior() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nips");
        let manifest: Value =
            serde_json::from_str(&std::fs::read_to_string(root.join("manifest.json")).unwrap())
                .unwrap();
        let block = manifest["sources"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source| source["name"] == "block")
            .unwrap();
        assert_eq!(block["commit"], BLOCK_COMMIT);
        assert_eq!(block["files"], FILES.len());
        let mut found = std::fs::read_dir(root.join("block"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_encoded_bytes())
            .map(|bytes| String::from_utf8(bytes).unwrap())
            .filter(|name| name != "README.md")
            .collect::<Vec<_>>();
        found.sort();
        let expected = FILES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(found, expected);
        for (file, anchor) in FILES {
            let text = std::fs::read_to_string(root.join("block").join(file)).unwrap();
            assert!(text.contains(anchor), "{file} missing {anchor}");
        }
    }

    #[test]
    fn baseline_subset_fixtures_still_hold() {
        assert_eq!(
            BASELINE_FIXTURE_COMMIT,
            "8342dfcc5890b81a269a8ec3db73a8a56f76ce79"
        );
        check_aa();
        check_ae();
        check_am();
        check_ao();
        check_ap();
        check_cw();
        check_dv();
        check_er();
        check_gs();
        check_ia();
        check_mp();
        check_oa();
        check_pl();
        check_rs();
        check_wp();
        check_run_refs();
    }

    fn signer(byte: u8) -> (RelaySigner, SecretKey) {
        let secret = SecretKey::from_byte_array([byte; 32]).unwrap();
        let hex = [byte; 32]
            .iter()
            .map(|item| format!("{item:02x}"))
            .collect::<String>();
        (RelaySigner::from_secret_hex(&hex).unwrap(), secret)
    }

    fn xonly(pubkey: &str) -> XOnlyPublicKey {
        let mut bytes = [0_u8; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&pubkey[index * 2..index * 2 + 2], 16).unwrap();
        }
        XOnlyPublicKey::from_byte_array(bytes).unwrap()
    }

    fn ciphertext(author: &SecretKey, peer: &XOnlyPublicKey) -> String {
        encrypt("memory", &conversation_key(author, peer), [3_u8; 32]).unwrap()
    }

    fn attest(owner: &SecretKey, agent: &str) -> (String, String) {
        let keypair = Keypair::from_secret_key(&Secp256k1::signing_only(), owner);
        let pubkey = keypair.x_only_public_key().0.to_string();
        let digest: [u8; 32] =
            Sha256::digest(format!("nostr:agent-auth:{agent}:").as_bytes()).into();
        let signature = Secp256k1::signing_only()
            .sign_schnorr_no_aux_rand(&digest, &keypair)
            .to_string();
        (pubkey, signature)
    }

    fn check_aa() {
        let (agent, _) = signer(0x51);
        let (_, owner_secret) = signer(0x52);
        let (owner_pub, signature) = attest(&owner_secret, agent.pubkey());
        let event = agent.sign(
            10,
            22_242,
            vec![Tag::new(vec![
                "auth".into(),
                owner_pub,
                String::new(),
                signature,
            ])],
            String::new(),
        );
        assert!(verify_agent_auth_attestation(&event).unwrap().is_some());
    }

    fn check_ae() {
        let (agent, secret) = signer(0x53);
        let (owner, _) = signer(0x54);
        let event = agent.sign(
            10,
            30_174,
            vec![
                Tag::new(vec!["d".into(), "ab".repeat(32)]),
                Tag::new(vec!["p".into(), owner.pubkey().to_owned()]),
            ],
            ciphertext(&secret, &xonly(owner.pubkey())),
        );
        validate_block_ingest(&event, 10).unwrap();
    }

    fn check_am() {
        let (agent, secret) = signer(0x55);
        let (owner, _) = signer(0x56);
        let event = agent.sign(
            10,
            44_200,
            vec![
                Tag::new(vec!["p".into(), owner.pubkey().to_owned()]),
                Tag::new(vec!["agent".into(), agent.pubkey().to_owned()]),
            ],
            ciphertext(&secret, &xonly(owner.pubkey())),
        );
        assert_eq!(agent_turn_metric_owner(&event).unwrap(), owner.pubkey());
    }

    fn check_ao() {
        let (agent, secret) = signer(0x57);
        let (owner, _) = signer(0x58);
        let event = agent.sign(
            10,
            24_200,
            vec![
                Tag::new(vec!["p".into(), owner.pubkey().to_owned()]),
                Tag::new(vec!["agent".into(), agent.pubkey().to_owned()]),
                Tag::new(vec!["frame".into(), "telemetry".into()]),
            ],
            ciphertext(&secret, &xonly(owner.pubkey())),
        );
        assert!(agent_observer_route(&event).unwrap().is_some());
    }

    fn check_ap() {
        let (agent, _) = signer(0x59);
        let persona = agent.sign(
            10,
            30_175,
            vec![Tag::new(vec!["d".into(), "guide".into()])],
            "public".into(),
        );
        validate_block_ingest(&persona, 10).unwrap();
        let catalog = agent.sign(
            10,
            30_178,
            vec![Tag::new(vec!["d".into(), "team".into()])],
            "public".into(),
        );
        validate_block_ingest(&catalog, 10).unwrap();
    }

    fn check_cw() {
        let signer = RelaySigner::from_secret_hex(&"61".repeat(32)).unwrap();
        let row = Event {
            id: "12".repeat(32),
            pubkey: signer.pubkey().to_owned(),
            created_at: 40,
            kind: 1,
            tags: vec![Tag::new(vec!["h".into(), "room".into()])],
            content: String::new(),
            sig: "22".repeat(64),
        };
        let request = parse_window(&json!({"#h": ["room"], "top_level": true}))
            .unwrap()
            .unwrap();
        let page = render_window(std::slice::from_ref(&row), &request, false, &signer, 50).unwrap();
        accept_window(&page.events, "room", None, signer.pubkey()).unwrap();
        assert!(!page.has_more);
    }

    fn check_dv() {
        let event = Event {
            id: "12".repeat(32),
            pubkey: "11".repeat(32),
            created_at: 10,
            kind: 41_010,
            tags: vec![Tag::new(vec!["h".into(), "dm-channel".into()])],
            content: String::new(),
            sig: "22".repeat(64),
        };
        assert_eq!(dm_visibility_channel(&event).unwrap(), "dm-channel");
    }

    fn check_er() {
        let (author, secret) = signer(0x62);
        let event = author.sign(
            10,
            30_300,
            vec![
                Tag::new(vec!["d".into(), "reminder".into()]),
                Tag::new(vec!["not_before".into(), "20".into()]),
            ],
            ciphertext(&secret, &xonly(author.pubkey())),
        );
        validate_block_ingest(&event, 10).unwrap();
    }

    fn check_gs() {
        let (_, secret) = signer(0x63);
        let signed = sign_git_object(&secret, b"blob", 1_700_000_000, None).unwrap();
        let verified = verify_git_object(&signed.armor, b"blob", None).unwrap();
        assert!(verified.status.contains("GOODSIG"));
    }

    fn check_ia() {
        let (owner, _) = signer(0x64);
        let (target, _) = signer(0x65);
        let event = owner.sign(
            1_000,
            9_035,
            vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["p".into(), target.pubkey().to_owned()]),
            ],
            String::new(),
        );
        let request = parse_identity_archive_request(&event, 1_000).unwrap();
        assert!(request.archive);
    }

    fn check_mp() {
        let event = Event {
            id: "12".repeat(32),
            pubkey: "11".repeat(32),
            created_at: 10,
            kind: 30_621,
            tags: vec![Tag::new(vec!["d".into(), "project".into()])],
            content: String::new(),
            sig: "22".repeat(64),
        };
        validate_block_ingest(&event, 10).unwrap();
    }

    fn check_oa() {
        let (agent, _) = signer(0x66);
        let (_, owner_secret) = signer(0x67);
        let (owner_pub, signature) = attest(&owner_secret, agent.pubkey());
        let event = agent.sign(
            10,
            1,
            vec![Tag::new(vec![
                "auth".into(),
                owner_pub,
                String::new(),
                signature,
            ])],
            String::new(),
        );
        assert!(verify_owner_attestation(&event).unwrap().is_some());
    }

    fn check_pl() {
        let (author, author_secret) = signer(0x68);
        let (executor, executor_secret) = signer(0x69);
        let descriptor = PushDescriptor {
            origin: "ws://127.0.0.1:7447".into(),
            key_id: "current".into(),
            pubkey: executor.pubkey().to_owned(),
            app_profile: "com.openagents.relay/ios".into(),
            transport: "apns".into(),
            push_kinds: vec![1],
            limits: LeaseLimits::default(),
        };
        validate_descriptor(&descriptor).unwrap();
        assert!(
            descriptor_document(&descriptor)["push_kinds"]
                .as_array()
                .is_some()
                || descriptor_document(&descriptor).get("push_kinds").is_some()
        );
        let plaintext = json!({
            "v": 1,
            "origin": descriptor.origin,
            "app_profile": descriptor.app_profile,
            "transport": "apns",
            "endpoint": "token",
            "generation": 2,
            "active": true,
            "subscriptions": [{"filter": {"kinds": [1], "#p": [author.pubkey()]}, "class": "silent"}]
        })
        .to_string();
        let encrypted = encrypt(
            &plaintext,
            &conversation_key(&author_secret, &xonly(executor.pubkey())),
            [4_u8; 32],
        )
        .unwrap();
        let opened = open_lease(&encrypted, &executor_secret, &xonly(author.pubkey())).unwrap();
        let lease = author.sign(
            50,
            30_350,
            vec![
                Tag::new(vec!["d".into(), "install".into()]),
                Tag::new(vec!["expiration".into(), "4000".into()]),
                Tag::new(vec!["exec".into(), "current".into()]),
            ],
            encrypted,
        );
        let accepted = accept_lease(&lease, &opened, 60, &descriptor, None, &[], 0).unwrap();
        let message = author.sign(
            70,
            1,
            vec![Tag::new(vec!["p".into(), author.pubkey().to_owned()])],
            "ping".into(),
        );
        assert!(lease_matches(&accepted, &message, 70));
        assert!(author_may_read(&message, author.pubkey()));
        let body = application_body("apns").unwrap();
        assert!(!body.contains(&message.id));
    }

    fn check_rs() {
        assert_eq!(EventClass::from_kind(30_078), EventClass::Addressable);
    }

    fn check_wp() {
        let event = Event {
            id: "12".repeat(32),
            pubkey: "11".repeat(32),
            created_at: 10,
            kind: 9_033,
            tags: vec![Tag::new(vec![
                "icon".into(),
                "https://example.com/icon.png".into(),
            ])],
            content: String::new(),
            sig: "22".repeat(64),
        };
        assert_eq!(
            workspace_icon(&event).unwrap(),
            "https://example.com/icon.png"
        );
    }

    fn check_run_refs() {
        let mut data = serde_json::Map::new();
        data.insert(
            "block_refs".into(),
            json!([{"kind": 44200, "id": "ab".repeat(32), "role": "metric", "durable": true}]),
        );
        validate_block_refs(&data).unwrap();
        data.insert(
            "block_refs".into(),
            json!([{"kind": 24200, "id": "ab".repeat(32), "role": "telemetry", "durable": true}]),
        );
        assert!(validate_block_refs(&data).is_err());
    }
}
