//! The receipt book's Postgres adapter, shared across account-service replicas.
use pay_ledger::{
    Error, Result,
    public_earnings::{Backend, Invoice, Published, decode_publication, encode_publication},
};
use tenancy::db::{Database, Tx};

pub(super) struct Postgres {
    database: Database,
}
impl Postgres {
    pub fn new(database: Database) -> Self {
        Self { database }
    }
}
fn unavailable(_: tenancy::db::Error) -> Error {
    Error::Denied("public receipt database is unavailable")
}
fn number(value: u64) -> Result<i64> {
    value
        .try_into()
        .map_err(|_| Error::Invalid("receipt integer"))
}
fn unsigned(value: i64) -> Result<u64> {
    value
        .try_into()
        .map_err(|_| Error::Invalid("stored receipt integer"))
}
fn invoice(row: &tenancy::db::PgRow) -> Result<Invoice> {
    Ok(Invoice {
        payment_hash: row.get(0),
        account: row.get(1),
        party: row.get(2),
        wallet: row.get(3),
        name: row.get(4),
        revision: unsigned(row.get(5))?,
        amount_msat: unsigned(row.get(6))?,
        issued_at: unsigned(row.get(7))?,
        expires_at: unsigned(row.get(8))?,
        tip: row.get(9),
        paid_at: row.get::<_, Option<i64>>(10).map(unsigned).transpose()?,
    })
}
fn publication(row: &tenancy::db::PgRow) -> Result<Published> {
    decode_publication(
        row.get(0),
        &row.get::<_, String>(1),
        &row.get::<_, String>(2),
    )
}
const INVOICE: &str =
    "payment_hash,account,party,wallet,name,revision,amount_msat,issued_at,expires_at,tip,paid_at";
fn in_transaction(tx: &mut Tx, source: &str) -> Result<Option<Published>> {
    tx.query(
        "SELECT account,source,event FROM money.published_earnings WHERE source_id=$1 FOR UPDATE",
        &[&source],
    )
    .map_err(unavailable)?
    .first()
    .map(publication)
    .transpose()
}
impl Backend for Postgres {
    fn insert_invoice(&self, row: &Invoice) -> Result<()> {
        self.database.execute("INSERT INTO money.named_invoices(payment_hash,account,party,wallet,name,revision,amount_msat,issued_at,expires_at,tip,paid_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NULL) ON CONFLICT(payment_hash) DO NOTHING",&[&row.payment_hash,&row.account,&row.party,&row.wallet,&row.name,&number(row.revision)?,&number(row.amount_msat)?,&number(row.issued_at)?,&number(row.expires_at)?,&row.tip]).map_err(unavailable)?;
        Ok(())
    }
    fn invoice(&self, hash: &str) -> Result<Option<Invoice>> {
        self.database
            .query(
                &format!("SELECT {INVOICE} FROM money.named_invoices WHERE payment_hash=$1"),
                &[&hash],
            )
            .map_err(unavailable)?
            .first()
            .map(invoice)
            .transpose()
    }
    fn mark_paid(&self, hash: &str, now: u64) -> Result<bool> {
        Ok(self.database.execute("UPDATE money.named_invoices SET paid_at=$1 WHERE payment_hash=$2 AND paid_at IS NULL",&[&number(now)?,&hash]).map_err(unavailable)?!=0)
    }
    fn invoices(
        &self,
        account: &str,
        party: &str,
        paid: bool,
        limit: usize,
    ) -> Result<Vec<Invoice>> {
        let sql = if paid {
            format!(
                "SELECT {INVOICE} FROM money.named_invoices WHERE account=$1 AND party=$2 AND tip AND paid_at IS NOT NULL ORDER BY issued_at DESC,payment_hash LIMIT $3"
            )
        } else {
            format!(
                "UPDATE money.named_invoices SET checked_at=EXTRACT(EPOCH FROM clock_timestamp())::bigint WHERE payment_hash IN (SELECT payment_hash FROM money.named_invoices WHERE account=$1 AND party=$2 AND tip AND paid_at IS NULL ORDER BY checked_at,issued_at,payment_hash LIMIT $3 FOR UPDATE SKIP LOCKED) RETURNING {INVOICE}"
            )
        };
        self.database
            .query(&sql, &[&account, &party, &(limit as i64)])
            .map_err(unavailable)?
            .iter()
            .map(invoice)
            .collect()
    }
    fn publication(&self, source: &str) -> Result<Option<Published>> {
        self.database
            .query(
                "SELECT account,source,event FROM money.published_earnings WHERE source_id=$1",
                &[&source],
            )
            .map_err(unavailable)?
            .first()
            .map(publication)
            .transpose()
    }
    fn update(
        &mut self,
        source: &str,
        change: &mut dyn FnMut(Option<Published>) -> Result<Published>,
    ) -> Result<Published> {
        let mut tx = self.database.transaction().map_err(unavailable)?;
        tx.lock(&format!("public-earning:{source}"))
            .map_err(unavailable)?;
        let result = change(in_transaction(&mut tx, source)?)?;
        let (party, source_json, event_json) = encode_publication(&result)?;
        tx.execute("INSERT INTO money.published_earnings VALUES ($1,$2,$3,$4,$5,$6,$7) ON CONFLICT(source_id) DO UPDATE SET event_id=excluded.event_id,event=excluded.event,created_at=excluded.created_at",&[&source,&result.account,&party,&source_json,&result.event.id,&event_json,&number(result.event.created_at)?]).map_err(unavailable)?;
        tx.execute("INSERT INTO money.published_earning_versions VALUES ($1,$2,$3,$4,$5) ON CONFLICT(event_id) DO NOTHING",&[&result.event.id,&source,&result.account,&source_json,&event_json]).map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(result)
    }
    fn published(&self, party: &str, limit: usize) -> Result<Vec<Published>> {
        self.database.query("SELECT account,source,event FROM money.published_earnings WHERE party=$1 ORDER BY created_at DESC,event_id LIMIT $2",&[&party,&(limit as i64)]).map_err(unavailable)?.iter().map(publication).collect()
    }
    fn event(&self, id: &str) -> Result<Option<Published>> {
        self.database
            .query(
                "SELECT account,source,event FROM money.published_earning_versions WHERE event_id=$1",
                &[&id],
            )
            .map_err(unavailable)?
            .first()
            .map(publication)
            .transpose()
    }
    fn hide(&self, account: &str, party: &str, source: &str) -> Result<()> {
        self.database.execute("DELETE FROM money.published_earnings WHERE account=$1 AND party=$2 AND source_id=$3",&[&account,&party,&source]).map_err(unavailable)?;
        Ok(())
    }
    fn forget_account(&mut self, account: &str) -> Result<()> {
        let mut tx = self.database.transaction().map_err(unavailable)?;
        tx.execute(
            "DELETE FROM money.published_earnings WHERE account=$1",
            &[&account],
        )
        .map_err(unavailable)?;
        tx.execute(
            "DELETE FROM money.named_invoices WHERE account=$1",
            &[&account],
        )
        .map_err(unavailable)?;
        tx.commit().map_err(unavailable)?;
        Ok(())
    }
    fn verified_unpaid(&self, hash: &str, now: u64) -> Result<()> {
        self.database.execute("UPDATE money.named_invoices SET verified_unpaid_at=$1 WHERE payment_hash=$2 AND paid_at IS NULL",&[&number(now)?,&hash]).map_err(unavailable)?;
        Ok(())
    }
    fn prune_unpaid(&self, now: u64) -> Result<()> {
        self.database
            .execute(
                "DELETE FROM money.named_invoices WHERE paid_at IS NULL AND expires_at<$1 AND verified_unpaid_at>=expires_at+604800",
                &[&number(now.saturating_sub(7 * 24 * 60 * 60))?],
            )
            .map_err(unavailable)?;
        Ok(())
    }
    fn service_secret(&self, candidate: &str) -> Result<String> {
        self.database
            .execute(
                "INSERT INTO money.receipt_service_key VALUES (1,$1) ON CONFLICT(id) DO NOTHING",
                &[&candidate],
            )
            .map_err(unavailable)?;
        let rows = self
            .database
            .query(
                "SELECT secret FROM money.receipt_service_key WHERE id=1",
                &[],
            )
            .map_err(unavailable)?;
        rows.first()
            .map(|row| row.get(0))
            .ok_or(Error::Invalid("receipt signing key disappeared"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};
    #[test]
    fn shared_postgres_receipts_survive_reopen_and_keep_private_proofs_out() {
        let Ok(dsn) = std::env::var("OPENAGENTS_PROFILE_RECEIPTS_TEST_POSTGRES") else {
            return;
        };
        assert!(dsn.contains("host=127.0.0.1") && dsn.contains("dbname=receipt_test"));
        let db = Database::connect(&dsn).unwrap();
        let mut first =
            pay_ledger::public_earnings::Store::with_backend(Box::new(Postgres::new(db.clone())));
        let second = pay_ledger::public_earnings::Store::with_backend(Box::new(Postgres::new(db)));
        let account = format!("receipt-test-{:032x}", secp256k1::rand::random::<u128>());
        let preimage = hex::encode(sha2::Sha256::digest(account.as_bytes()));
        let hash = hex::encode(Sha256::digest(hex::decode(&preimage).unwrap()));
        let invoice = Invoice {
            payment_hash: hash.clone(),
            account: account.clone(),
            party: "11".repeat(32),
            wallet: format!("02{}", "22".repeat(32)),
            name: "receipt-test".into(),
            revision: 1,
            amount_msat: 12000,
            issued_at: 100,
            expires_at: 3700,
            tip: true,
            paid_at: None,
        };
        first.remember(&invoice).unwrap();
        assert!(
            second
                .tips(&account, &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
        second.confirm(&hash, &preimage, 200).unwrap();
        let row = first.tips(&account, &invoice.party, 20).unwrap().remove(0);
        let signer = first.signer(&"04".repeat(32)).unwrap();
        assert_eq!(
            signer.pubkey(),
            second.signer(&"05".repeat(32)).unwrap().pubkey()
        );
        let event = first
            .publish(&account, &row.source, &row.receipt, &signer, 201)
            .unwrap();
        assert!(second.event(&event.id).unwrap().is_some());
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(
            !encoded.contains(&hash) && !encoded.contains(&preimage) && !encoded.contains(&account)
        );
        first
            .hide(&account, &invoice.party, &row.receipt.source)
            .unwrap();
        assert!(second.event(&event.id).unwrap().is_none());
        first.forget_account(&account).unwrap();
        assert!(
            second
                .tips(&account, &invoice.party, 20)
                .unwrap()
                .is_empty()
        );
    }
}
