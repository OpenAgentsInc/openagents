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
use crate::mirror::{MirrorPolicy, Receipt, receipt_msg};
use crate::output::{TreeOutput, state_tweak};
use crate::rules::{self, Op, Record, Reject, TreeParams, TreeState};
use crate::tree::{Tree, TreeError, exit_auth_msg, tagged_hash};
use crate::unroll::{self, UnrollPackage};

#[cfg(feature = "gcp-sealing")]
pub mod sealed;

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
    /// The transition handed out by `prepare`, waiting for `finish`.
    pending: Option<Prepared>,
    height: u32,
    exit_deadline: u32,
    /// (position, height received) of each waiting exit request.
    exit_requests: Vec<(u64, u32)>,
    mirrors: Option<MirrorPolicy>,
    guardian: Option<XOnlyPublicKey>,
    halted: bool,
}

/// Tag of the owner's `HALT` message.
pub const TAG_HALT: &str = "LayerTreeTEE/halt";

/// Default forced-inclusion deadline `X` in blocks (spec 2.2 item 7).
pub const EXIT_DEADLINE: u32 = 6;

/// What the guardian signs to halt a tree.
pub fn halt_msg(tree_id: &[u8; 32]) -> [u8; 32] {
    tagged_hash(TAG_HALT, &[tree_id])
}

/// A transition checked and ready for its parties' signatures.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// Unsigned; input 0's witness is filled in by `finish`.
    pub tx: Transaction,
    pub prevouts: Vec<TxOut>,
    pub op: Op,
    pub record: Record,
    /// The new state's unroll, already signed with `K_{k+1}` (now erased).
    pub unroll: Option<UnrollPackage>,
    /// The new state, when the tree continues.
    pub next: Option<TreeState>,
    /// What every party signs: the key-path sighash of input 0.
    pub sighash: [u8; 32],
}

impl Enclave {
    /// Generate a key in the guest. Hardware evidence is requested separately.
    /// This method accepts no private key and returns no key material.
    pub fn launch_in_guest(cfg: Config) -> Enclave {
        let secp = Secp256k1::new();
        let (key, _) = secp.generate_keypair(&mut bitcoin::secp256k1::rand::thread_rng());
        Self::from_guest_key(cfg, key)
    }

    fn from_guest_key(cfg: Config, key: SecretKey) -> Enclave {
        let secp = Secp256k1::new();
        let params = TreeParams {
            enclave_key: XOnlyPublicKey::from(key.public_key(&secp)),
            operator_key: cfg.operator_key,
            tree_id: cfg.tree_id,
            depth: cfg.depth,
            dust_min: cfg.dust_min,
            unroll_delay: cfg.unroll_delay,
            migration_m: cfg.migration_m,
        };
        Enclave {
            secp,
            key,
            params,
            radix: cfg.radix,
            state: None,
            next_unroll: None,
            compromised: false,
            pending: None,
            height: 0,
            exit_deadline: EXIT_DEADLINE,
            exit_requests: Vec::new(),
            mirrors: None,
            guardian: None,
            halted: false,
        }
    }

    /// Make the model's fake vendor report for a fresh guest key.
    pub fn launch(
        cfg: Config,
        measurement: [u8; 48],
        vendor: &FakeVendor,
        platform: Platform,
    ) -> (Enclave, Report) {
        let enclave = Self::launch_in_guest(cfg);
        let report = vendor.sign_report(
            &enclave.secp,
            platform,
            measurement,
            report_data(&enclave.params),
        );
        (enclave, report)
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

    /// The block height the host reports (from headers it verified, in a
    /// real enclave). Drives the forced-inclusion deadline.
    pub fn set_height(&mut self, height: u32) {
        self.height = self.height.max(height);
    }

    /// Blocks an exit request may wait before every non-exit transition is
    /// refused (`X`, spec 2.2 item 7; 6 by default).
    pub fn set_exit_deadline(&mut self, blocks: u32) {
        self.exit_deadline = blocks;
    }

    /// Require `m` of these mirrors' receipts before any signature.
    pub fn set_mirror_policy(&mut self, policy: MirrorPolicy) {
        self.mirrors = Some(policy);
    }

    /// The owner's guardian key, the only key that can sign `HALT`.
    pub fn set_guardian(&mut self, key: XOnlyPublicKey) {
        self.guardian = Some(key);
    }

    pub fn halted(&self) -> bool {
        self.halted
    }

    /// Exit requests waiting, as (position, height received).
    pub fn exit_requests(&self) -> &[(u64, u32)] {
        &self.exit_requests
    }

    /// A member asks to exit directly, over the attested channel, signed by
    /// the leaf's owner over `exit_auth_msg(tree_id, leaf)`. From now the
    /// clock runs: after the deadline only exits are signed.
    pub fn request_exit(&mut self, pos: u64, owner_sig: &Signature) -> Result<(), Reject> {
        let state = self.state.as_ref().ok_or(Reject::NotAllowed)?;
        let leaf = state.tree.get(pos).ok_or(TreeError::PositionEmpty)?;
        let owner = XOnlyPublicKey::from_slice(&leaf.owner_key)
            .map_err(|_| Reject::BadSignature("owner key"))?;
        let msg = Message::from_digest(exit_auth_msg(&self.params.tree_id, leaf));
        self.secp
            .verify_schnorr(owner_sig, &msg, &owner)
            .map_err(|_| Reject::BadSignature("exit request"))?;
        if !self.exit_requests.iter().any(|(p, _)| *p == pos) {
            self.exit_requests.push((pos, self.height));
        }
        Ok(())
    }

    /// The owner's kill switch: a guardian signature over `halt_msg`.
    /// Afterwards only exits are signed and heartbeats stop, so the tree
    /// unrolls Δ blocks after its last transition.
    pub fn halt(&mut self, sig: &Signature) -> Result<(), Reject> {
        let g = self.guardian.ok_or(Reject::NotAllowed)?;
        let msg = Message::from_digest(halt_msg(&self.params.tree_id));
        self.secp
            .verify_schnorr(sig, &msg, &g)
            .map_err(|_| Reject::BadSignature("halt"))?;
        self.halted = true;
        Ok(())
    }

    /// The liveness rules every transition passes before anything else.
    fn admit(&self, op: &Op) -> Result<(), Reject> {
        let exit = matches!(op, Op::Exit { .. });
        if self.halted && !exit {
            return Err(Reject::Halted);
        }
        let overdue = self
            .exit_requests
            .iter()
            .any(|(_, h)| self.height.saturating_sub(*h) >= self.exit_deadline);
        if overdue && !exit {
            return Err(Reject::ExitCensored);
        }
        Ok(())
    }

    /// Phase one of two-phase signing (spec 3.2): check everything but the
    /// parties' signatures, pre-sign the next state's unroll with
    /// `K_{k+1}`, erase it, and hand back the unsigned transaction with the
    /// unroll. Parties check the unroll (`verify_package`) before they sign.
    pub fn prepare(
        &mut self,
        tx: Transaction,
        prevouts: &[TxOut],
        op: &Op,
    ) -> Result<Prepared, Reject> {
        let state = self.state.clone().ok_or(Reject::NotAllowed)?;
        self.admit(op)?;
        let next_pk = match &self.next_unroll {
            Some(sk) => XOnlyPublicKey::from(sk.public_key(&self.secp)),
            None => return Err(Reject::NotAllowed),
        };
        let (fx, record) = if self.compromised {
            let fx = rules::effects(&self.secp, &self.params, &state, op)?;
            let rec = rules::record(&state, &fx, op, next_pk);
            (fx, rec)
        } else {
            rules::check_unsigned(&self.secp, &self.params, &state, &tx, prevouts, op, next_pk)?
        };
        let mut sk = self.next_unroll.take().expect("checked above");
        let new_root = fx.tree.root()?;
        let continues = record.next_unroll_key.is_some();
        let next = continues.then(|| TreeState {
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
        let unroll = next.as_ref().map(|n| {
            unroll::build(
                &self.secp,
                &self.params,
                n.output,
                n.outpoint,
                new_root.sum,
                n.tree.leaves(),
                &sk,
                self.radix,
            )
        });
        sk.non_secure_erase();
        let prepared = Prepared {
            sighash: rules::sighash(&tx, prevouts),
            tx,
            prevouts: prevouts.to_vec(),
            op: op.clone(),
            record,
            unroll,
            next,
        };
        self.pending = Some(prepared.clone());
        Ok(prepared)
    }

    /// Phase two: the parties' signatures over the prepared transaction and
    /// the mirrors' receipts arrive. Only then does the enclave sign input
    /// 0 by the key path and advance (the lab treats a signed transition as
    /// confirmed).
    pub fn finish(&mut self, sigs: &[Signature], receipts: &[Receipt]) -> Result<Signed, Reject> {
        let pp = self.pending.clone().ok_or(Reject::NotAllowed)?;
        let state = self.state.clone().ok_or(Reject::NotAllowed)?;
        // A HALT or a stale exit request may have arrived in between.
        self.admit(&pp.op)?;
        if !self.compromised {
            rules::check_signatures(&self.secp, &self.params, &state, &pp.op, pp.sighash, sigs)?;
        }
        if let Some(policy) = &self.mirrors {
            let msg = receipt_msg(&pp.record, pp.unroll.as_ref());
            if policy.count(&self.secp, msg, receipts) < policy.m {
                return Err(Reject::MissingReceipts);
            }
        }

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
        let sig = self
            .secp
            .sign_schnorr(&Message::from_digest(pp.sighash), &kp);
        let mut tx = pp.tx;
        let mut w = Witness::new();
        w.push(sig.as_ref());
        tx.input[0].witness = w;

        self.state = pp.next;
        self.pending = None;
        // Served or moot exit requests leave the queue.
        match (&self.state, &pp.op) {
            (None, _) => self.exit_requests.clear(),
            (Some(st), op) => {
                let served = match op {
                    Op::Exit { pos, .. } => Some(*pos),
                    _ => None,
                };
                self.exit_requests
                    .retain(|(p, _)| Some(*p) != served && st.tree.get(*p).is_some());
            }
        }
        Ok(Signed {
            tx,
            record: pp.record,
            unroll: pp.unroll,
        })
    }

    /// Both phases in one call, for tests and callers that hold every
    /// signature already. The parties' signatures are checked before
    /// anything is consumed, so a refused call leaves the enclave as it
    /// was. With a mirror policy set, use `prepare` and `finish`.
    pub fn authorize(
        &mut self,
        tx: Transaction,
        prevouts: &[TxOut],
        op: &Op,
        sigs: &[Signature],
    ) -> Result<Signed, Reject> {
        let state = self.state.clone().ok_or(Reject::NotAllowed)?;
        self.admit(op)?;
        if !self.compromised {
            let next_pk = match &self.next_unroll {
                Some(sk) => XOnlyPublicKey::from(sk.public_key(&self.secp)),
                None => return Err(Reject::NotAllowed),
            };
            rules::check(
                &self.secp,
                &self.params,
                &state,
                &tx,
                prevouts,
                op,
                sigs,
                next_pk,
            )?;
        }
        self.prepare(tx, prevouts, op)?;
        self.finish(sigs, &[])
    }

    /// Test hook: an enclave whose rule check is gone.
    #[doc(hidden)]
    pub fn break_for_test(&mut self) {
        self.compromised = true;
    }
}
