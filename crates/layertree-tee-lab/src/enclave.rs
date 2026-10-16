//! An in-process model of the enclave co-signer.
//!
//! What the model keeps from the real thing:
//!
//! * the signing key is generated inside and no method returns it;
//! * the only way to get a signature from it is `authorize`, which runs
//!   the full rule predicate (`rules::check`) first;
//! * each state's unroll key is generated, used once to pre-sign that
//!   state's unroll, and erased; `ephemeral_keys_held` lets tests see it.
//!
//! What it cannot model: memory encryption, the hardware report, sealing,
//! side channels. Those are in the spec's threat model.

use bitcoin::key::{Keypair, TapTweak, XOnlyPublicKey};
use bitcoin::secp256k1::{All, Message, Secp256k1, SecretKey, schnorr::Signature};
use bitcoin::{OutPoint, Transaction, TxOut, Witness};

use crate::attest::{FakeVendor, Platform, Report, report_data};
use crate::output::{TreeOutput, state_tweak};
use crate::rules::{self, Op, Record, Reject, TreeParams, TreeState};
use crate::tree::{Tree, TreeError};
use crate::unroll::{self, UnrollPackage};

/// Parameters chosen by the operator; the enclave adds its own key.
#[derive(Debug, Clone)]
pub struct Config {
    pub operator_key: [u8; 32],
    pub tree_id: [u8; 32],
    pub depth: u32,
    pub dust_min: u64,
    pub unroll_delay: u16,
    pub migration_m: Option<[u8; 32]>,
    pub radix: usize,
}

/// The result of a signed transition.
#[derive(Debug, Clone)]
pub struct Signed {
    /// The transaction with input 0's key-path witness filled in.
    pub tx: Transaction,
    pub record: Record,
    /// The unroll of the new state; `None` after a terminal exit or a
    /// migration.
    pub unroll: Option<UnrollPackage>,
}

pub struct Enclave {
    secp: Secp256k1<All>,
    key: SecretKey,
    params: TreeParams,
    radix: usize,
    state: Option<TreeState>,
    next_unroll: Option<SecretKey>,
    /// Signs anything that passes; a test-only stand-in for a broken
    /// enclave, used to show what light clients catch.
    compromised: bool,
}

impl Enclave {
    /// Boot: make the key inside, bind it and the parameters into the
    /// report the hardware signs.
    pub fn launch(
        cfg: Config,
        measurement: [u8; 48],
        vendor: &FakeVendor,
        platform: Platform,
    ) -> (Enclave, Report) {
        let secp = Secp256k1::new();
        let (key, pk) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        let params = TreeParams {
            enclave_key: XOnlyPublicKey::from(pk),
            operator_key: cfg.operator_key,
            tree_id: cfg.tree_id,
            depth: cfg.depth,
            dust_min: cfg.dust_min,
            unroll_delay: cfg.unroll_delay,
            migration_m: cfg.migration_m,
        };
        let report = vendor.sign_report(&secp, platform, measurement, report_data(&params));
        let e = Enclave {
            secp,
            key,
            params,
            radix: cfg.radix,
            state: None,
            next_unroll: None,
            compromised: false,
        };
        (e, report)
    }

    pub fn params(&self) -> &TreeParams {
        &self.params
    }

    pub fn state(&self) -> Option<&TreeState> {
        self.state.as_ref()
    }

    /// Fresh `K_{k+1}` for the next tree output. Callers put it in output
    /// 0 of the transaction they are building.
    pub fn next_unroll_key(&mut self) -> XOnlyPublicKey {
        if let Some(mut old) = self.next_unroll.take() {
            old.non_secure_erase();
        }
        let (sk, pk) = self
            .secp
            .generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        self.next_unroll = Some(sk);
        XOnlyPublicKey::from(pk)
    }

    /// Ephemeral secrets currently in memory (0 or 1).
    pub fn ephemeral_keys_held(&self) -> usize {
        self.next_unroll.is_some() as usize
    }

    /// Adopt the creation output (spec 6.5), funded to the output for
    /// `tree`'s root and the key from `next_unroll_key`, and pre-sign its
    /// unroll. Members check that unroll before they fund.
    pub fn open(&mut self, tree: Tree, outpoint: OutPoint) -> Result<UnrollPackage, Reject> {
        if self.state.is_some() || tree.depth() != self.params.depth {
            return Err(Reject::NotAllowed);
        }
        for l in tree.leaves().values() {
            l.validate()?;
        }
        let root = tree.root()?;
        let mut sk = self.next_unroll.take().ok_or(Reject::NotAllowed)?;
        let output = TreeOutput {
            root_hash: root.hash,
            unroll_key: XOnlyPublicKey::from(sk.public_key(&self.secp)),
        };
        let pkg = unroll::build(
            &self.secp,
            &self.params,
            output,
            outpoint,
            root.sum,
            tree.leaves(),
            &sk,
            self.radix,
        );
        sk.non_secure_erase();
        self.state = Some(TreeState {
            tree,
            outpoint,
            output,
        });
        Ok(pkg)
    }

    /// The one signing path. Runs the predicate; if it passes, pre-signs
    /// the next state's unroll with `K_{k+1}`, erases it, signs input 0 by
    /// the key path, and advances (the lab treats a signed transition as
    /// confirmed).
    pub fn authorize(
        &mut self,
        mut tx: Transaction,
        prevouts: &[TxOut],
        op: &Op,
        sigs: &[Signature],
    ) -> Result<Signed, Reject> {
        let state = self.state.clone().ok_or(Reject::NotAllowed)?;
        let next_pk = match &self.next_unroll {
            Some(sk) => XOnlyPublicKey::from(sk.public_key(&self.secp)),
            None => return Err(Reject::NotAllowed),
        };
        let (fx, record) = if self.compromised {
            let fx = rules::effects(&self.secp, &self.params, &state, op)?;
            let rec = rules::record(&state, &fx, op, next_pk);
            (fx, rec)
        } else {
            rules::check(
                &self.secp,
                &self.params,
                &state,
                &tx,
                prevouts,
                op,
                sigs,
                next_pk,
            )?
        };

        let mut sk = self.next_unroll.take().expect("checked above");
        let new_root = fx.tree.root()?;
        let continues = record.next_unroll_key.is_some();
        let unroll = continues.then(|| {
            let output = TreeOutput {
                root_hash: new_root.hash,
                unroll_key: next_pk,
            };
            let op0 = OutPoint {
                txid: tx.compute_txid(),
                vout: 0,
            };
            unroll::build(
                &self.secp,
                &self.params,
                output,
                op0,
                new_root.sum,
                fx.tree.leaves(),
                &sk,
                self.radix,
            )
        });
        sk.non_secure_erase();

        // Key-path signature: Q tweaked by H_state(r), then by the taptree.
        let merkle = state
            .output
            .spend_info(
                &self.secp,
                &self.params.enclave_key,
                self.params.unroll_delay,
            )
            .merkle_root();
        let kp = Keypair::from_secret_key(&self.secp, &self.key)
            .add_xonly_tweak(&self.secp, &state_tweak(&state.output.root_hash))
            .map_err(|_| Reject::Tree(TreeError::SumOverflow))?
            .tap_tweak(&self.secp, merkle)
            .to_keypair();
        let digest = rules::sighash(&tx, prevouts);
        let sig = self.secp.sign_schnorr(&Message::from_digest(digest), &kp);
        let mut w = Witness::new();
        w.push(sig.as_ref());
        tx.input[0].witness = w;

        self.state = continues.then(|| TreeState {
            tree: fx.tree.clone(),
            outpoint: OutPoint {
                txid: tx.compute_txid(),
                vout: 0,
            },
            output: TreeOutput {
                root_hash: new_root.hash,
                unroll_key: next_pk,
            },
        });
        Ok(Signed { tx, record, unroll })
    }

    /// Test hook: an enclave whose rule check is gone.
    #[doc(hidden)]
    pub fn break_for_test(&mut self) {
        self.compromised = true;
    }
}
