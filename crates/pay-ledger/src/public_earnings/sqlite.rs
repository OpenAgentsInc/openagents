use super::{Invoice, Published, decode_publication, encode_publication};
use crate::{Error, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, time::Duration};

pub(super) struct Backend {
    connection: Connection,
}
impl Backend {
    pub fn open(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
                .map_err(|_| Error::Denied("public receipt permissions"))?;
        }
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;
            CREATE TABLE IF NOT EXISTS named_invoice (
                payment_hash TEXT PRIMARY KEY, account TEXT NOT NULL, party TEXT NOT NULL,
                wallet TEXT NOT NULL, name TEXT NOT NULL, revision INTEGER NOT NULL,
                amount_msat INTEGER NOT NULL CHECK(amount_msat>0), issued_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL, tip INTEGER NOT NULL, paid_at INTEGER, checked_at INTEGER NOT NULL DEFAULT 0, verified_unpaid_at INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS named_invoice_account ON named_invoice(account,party,issued_at);
            CREATE TABLE IF NOT EXISTS published_earning (
                source_id TEXT PRIMARY KEY, account TEXT NOT NULL, party TEXT NOT NULL,
                source TEXT NOT NULL, event_id TEXT NOT NULL UNIQUE, event TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS published_earning_party ON published_earning(party,created_at);
            CREATE TABLE IF NOT EXISTS published_earning_version (
                event_id TEXT PRIMARY KEY, source_id TEXT NOT NULL REFERENCES published_earning(source_id) ON DELETE CASCADE,
                account TEXT NOT NULL, source TEXT NOT NULL, event TEXT NOT NULL
            );
            INSERT INTO published_earning_version SELECT event_id,source_id,account,source,event FROM published_earning WHERE true ON CONFLICT(event_id) DO NOTHING;
            CREATE TABLE IF NOT EXISTS receipt_service_key (id INTEGER PRIMARY KEY CHECK(id=1),secret TEXT NOT NULL);")?;
        Ok(Self { connection })
    }
}
fn invoice(row: &rusqlite::Row<'_>) -> rusqlite::Result<Invoice> {
    Ok(Invoice {
        payment_hash: row.get(0)?,
        account: row.get(1)?,
        party: row.get(2)?,
        wallet: row.get(3)?,
        name: row.get(4)?,
        revision: row.get(5)?,
        amount_msat: row.get(6)?,
        issued_at: row.get(7)?,
        expires_at: row.get(8)?,
        tip: row.get(9)?,
        paid_at: row.get(10)?,
    })
}
fn load(connection: &Connection, column: &str, value: &str) -> Result<Option<Published>> {
    let sql = format!("SELECT account,source,event FROM published_earning WHERE {column}=?");
    let row: Option<(String, String, String)> = connection
        .query_row(&sql, [value], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .optional()?;
    row.map(|(account, source, event)| decode_publication(account, &source, &event))
        .transpose()
}
impl super::Backend for Backend {
    fn insert_invoice(&self, row: &Invoice) -> Result<()> {
        self.connection.execute("INSERT INTO named_invoice(payment_hash,account,party,wallet,name,revision,amount_msat,issued_at,expires_at,tip,paid_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,NULL) ON CONFLICT(payment_hash) DO NOTHING",params![row.payment_hash,row.account,row.party,row.wallet,row.name,row.revision,row.amount_msat,row.issued_at,row.expires_at,row.tip])?;
        Ok(())
    }
    fn invoice(&self, hash: &str) -> Result<Option<Invoice>> {
        Ok(self.connection.query_row("SELECT payment_hash,account,party,wallet,name,revision,amount_msat,issued_at,expires_at,tip,paid_at FROM named_invoice WHERE payment_hash=?",[hash],invoice).optional()?)
    }
    fn mark_paid(&self, hash: &str, now: u64) -> Result<bool> {
        Ok(self.connection.execute(
            "UPDATE named_invoice SET paid_at=? WHERE payment_hash=? AND paid_at IS NULL",
            params![now, hash],
        )? != 0)
    }
    fn invoices(
        &self,
        account: &str,
        party: &str,
        paid: bool,
        limit: usize,
    ) -> Result<Vec<Invoice>> {
        let columns = "payment_hash,account,party,wallet,name,revision,amount_msat,issued_at,expires_at,tip,paid_at";
        let sql = if paid {
            format!(
                "SELECT {columns} FROM named_invoice WHERE account=? AND party=? AND tip=1 AND paid_at IS NOT NULL ORDER BY issued_at DESC,payment_hash LIMIT ?"
            )
        } else {
            format!(
                "UPDATE named_invoice SET checked_at=unixepoch() WHERE payment_hash IN (SELECT payment_hash FROM named_invoice WHERE account=? AND party=? AND tip=1 AND paid_at IS NULL ORDER BY checked_at,issued_at,payment_hash LIMIT ?) RETURNING {columns}"
            )
        };
        let mut statement = self.connection.prepare(&sql)?;
        Ok(statement
            .query_map(params![account, party, limit], invoice)?
            .collect::<std::result::Result<Vec<_>, _>>()?)
    }
    fn publication(&self, source: &str) -> Result<Option<Published>> {
        load(&self.connection, "source_id", source)
    }
    fn update(
        &mut self,
        source: &str,
        change: &mut dyn FnMut(Option<Published>) -> Result<Published>,
    ) -> Result<Published> {
        let tx = self
            .connection
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let result = change(load(&tx, "source_id", source)?)?;
        let (party, source_json, event_json) = encode_publication(&result)?;
        tx.execute("INSERT INTO published_earning VALUES (?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(source_id) DO UPDATE SET event_id=excluded.event_id,event=excluded.event,created_at=excluded.created_at",params![source,result.account,party,source_json,result.event.id,event_json,result.event.created_at])?;
        tx.execute("INSERT INTO published_earning_version VALUES (?1,?2,?3,?4,?5) ON CONFLICT(event_id) DO NOTHING",params![result.event.id,source,result.account,source_json,event_json])?;
        tx.commit()?;
        Ok(result)
    }
    fn published(&self, party: &str, limit: usize) -> Result<Vec<Published>> {
        let mut statement = self.connection.prepare("SELECT account,source,event FROM published_earning WHERE party=? ORDER BY created_at DESC,event_id LIMIT ?")?;
        let rows = statement
            .query_map(params![party, limit], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(|(a, s, e)| decode_publication(a, &s, &e))
            .collect()
    }
    fn event(&self, id: &str) -> Result<Option<Published>> {
        let row: Option<(String, String, String)> = self
            .connection
            .query_row(
                "SELECT account,source,event FROM published_earning_version WHERE event_id=?",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        row.map(|(a, s, e)| decode_publication(a, &s, &e))
            .transpose()
    }
    fn hide(&self, account: &str, party: &str, source: &str) -> Result<()> {
        self.connection.execute(
            "DELETE FROM published_earning WHERE account=? AND party=? AND source_id=?",
            params![account, party, source],
        )?;
        Ok(())
    }
    fn forget_account(&mut self, account: &str) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute("DELETE FROM published_earning WHERE account=?", [account])?;
        tx.execute("DELETE FROM named_invoice WHERE account=?", [account])?;
        tx.commit()?;
        Ok(())
    }
    fn verified_unpaid(&self, hash: &str, now: u64) -> Result<()> {
        self.connection.execute("UPDATE named_invoice SET verified_unpaid_at=? WHERE payment_hash=? AND paid_at IS NULL",params![now,hash])?;
        Ok(())
    }
    fn prune_unpaid(&self, now: u64) -> Result<()> {
        self.connection.execute(
            "DELETE FROM named_invoice WHERE paid_at IS NULL AND expires_at<? AND verified_unpaid_at>=expires_at+604800",
            [now.saturating_sub(7 * 24 * 60 * 60)],
        )?;
        Ok(())
    }
    fn service_secret(&self, candidate: &str) -> Result<String> {
        self.connection.execute(
            "INSERT INTO receipt_service_key VALUES (1,?) ON CONFLICT(id) DO NOTHING",
            [candidate],
        )?;
        Ok(self.connection.query_row(
            "SELECT secret FROM receipt_service_key WHERE id=1",
            [],
            |r| r.get(0),
        )?)
    }
}
