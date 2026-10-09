//! A store document as rows: each collection in the document is a table
//! of `(key columns..., record jsonb)`, and the rest of the document is
//! `identity.stores.rest`. Reading puts the rows back where they came
//! from, so the document (and its digest) round-trips exactly.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Map, Value};
use tokio_postgres::types::ToSql;

use super::{Database, Error, Tx, in_store_tx};

/// One step of a collection's path in the document.
#[derive(Clone, Copy, Debug)]
pub enum Seg {
    /// An object field.
    Field(&'static str),
    /// Every entry of an object, its key becoming a key column.
    Each,
}

/// A map in the document kept as a table.
#[derive(Debug)]
pub struct Collection {
    /// The schema-qualified table.
    pub table: &'static str,
    /// Where the map is. The last step is a [`Seg::Field`].
    pub path: &'static [Seg],
    /// The key columns: one per [`Seg::Each`] in the path, then the map's
    /// own key.
    pub keys: &'static [&'static str],
    /// Columns fixed for this collection (a table shared by several).
    pub fixed: &'static [(&'static str, &'static str)],
}

/// A store's layout: its name and its collections, parents first.
#[derive(Debug)]
pub struct Layout {
    pub store: &'static str,
    pub collections: &'static [Collection],
    /// Whether sealed revisions are archived in `audit.revisions`.
    pub revisions: bool,
}

/// The rows of one collection: key values to record.
type Rows = BTreeMap<Vec<String>, Value>;

/// Split a document into its rest and each collection's rows.
pub fn explode(layout: &Layout, mut doc: Value) -> Result<(Value, Vec<Rows>), Error> {
    let mut all = Vec::with_capacity(layout.collections.len());
    // Children first, so a parent's record no longer holds them.
    for collection in layout.collections.iter().rev() {
        let mut rows = Rows::new();
        let mut keys = Vec::new();
        take(&mut doc, collection.path, &mut keys, &mut rows)
            .map_err(|e| Error(format!("{}: {e}", collection.table)))?;
        all.push(rows);
    }
    all.reverse();
    Ok((doc, all))
}

fn take(
    node: &mut Value,
    path: &[Seg],
    keys: &mut Vec<String>,
    rows: &mut Rows,
) -> Result<(), String> {
    match path {
        [Seg::Field(name)] => {
            let Some(object) = node.as_object_mut() else {
                return Ok(());
            };
            match object.remove(*name) {
                None | Some(Value::Null) => Ok(()),
                Some(Value::Object(map)) => {
                    for (key, record) in map {
                        let mut full = keys.clone();
                        full.push(key);
                        rows.insert(full, record);
                    }
                    Ok(())
                }
                Some(_) => Err(format!("`{name}` is not a map")),
            }
        }
        [Seg::Field(name), rest @ ..] => match node.get_mut(*name) {
            Some(child) => take(child, rest, keys, rows),
            None => Ok(()),
        },
        [Seg::Each, rest @ ..] => {
            let Some(object) = node.as_object_mut() else {
                return Ok(());
            };
            for (key, child) in object.iter_mut() {
                keys.push(key.clone());
                let out = take(child, rest, keys, rows);
                keys.pop();
                out?;
            }
            Ok(())
        }
        [] => Err("empty path".into()),
    }
}

/// Put the rows back into the rest, making an empty map for every
/// collection that has none.
pub fn assemble(layout: &Layout, mut doc: Value, all: Vec<Rows>) -> Result<Value, Error> {
    for (collection, rows) in layout.collections.iter().zip(all) {
        ensure(&mut doc, collection.path);
        for (keys, record) in rows {
            put(&mut doc, collection.path, &keys, record)
                .map_err(|e| Error(format!("{}: {e}", collection.table)))?;
        }
    }
    Ok(doc)
}

fn ensure(node: &mut Value, path: &[Seg]) {
    let Some(object) = node.as_object_mut() else {
        return;
    };
    match path {
        [Seg::Field(name)] => {
            object
                .entry(name.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
        }
        [Seg::Field(name), rest @ ..] => {
            let child = object
                .entry(name.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            ensure(child, rest);
        }
        [Seg::Each, rest @ ..] => {
            for child in object.values_mut() {
                ensure(child, rest);
            }
        }
        [] => {}
    }
}

fn put(node: &mut Value, path: &[Seg], keys: &[String], record: Value) -> Result<(), String> {
    let object = node.as_object_mut().ok_or("parent is not an object")?;
    match path {
        [Seg::Field(name)] => {
            let [key] = keys else {
                return Err("key count".into());
            };
            let map = object
                .entry(name.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            map.as_object_mut()
                .ok_or("not a map")?
                .insert(key.clone(), record);
            Ok(())
        }
        [Seg::Field(name), rest @ ..] => {
            let child = object
                .entry(name.to_string())
                .or_insert_with(|| Value::Object(Map::new()));
            put(child, rest, keys, record)
        }
        [Seg::Each, rest @ ..] => {
            let (first, others) = keys.split_first().ok_or("key count")?;
            let child = object
                .get_mut(first)
                .ok_or_else(|| format!("a row names `{first}`, which is not in the document"))?;
            put(child, rest, others, record)
        }
        [] => Err("empty path".into()),
    }
}

fn read_sql(layout: &Layout) -> String {
    let mut sql = String::from("SELECT s.revision, s.rest");
    for collection in layout.collections {
        let keys = collection.keys.join(", ");
        let filter = filter(collection, 1);
        sql.push_str(&format!(
            ", (SELECT coalesce(jsonb_agg(jsonb_build_array(jsonb_build_array({keys}), record)), '[]'::jsonb) FROM {}{filter})",
            collection.table
        ));
    }
    sql.push_str(" FROM identity.stores s WHERE s.store = $1");
    sql
}

fn filter(collection: &Collection, _from: usize) -> String {
    if collection.fixed.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = collection
        .fixed
        .iter()
        .map(|(column, value)| format!("{column} = '{}'", value.replace('\'', "''")))
        .collect();
    format!(" WHERE {}", parts.join(" AND "))
}

fn rows_of(value: Value) -> Result<Rows, Error> {
    let Value::Array(items) = value else {
        return Err(Error("collection rows are not an array".into()));
    };
    let mut rows = Rows::new();
    for item in items {
        let Value::Array(mut pair) = item else {
            return Err(Error("a row is not a pair".into()));
        };
        let record = pair.pop().unwrap_or(Value::Null);
        let keys = match pair.pop() {
            Some(Value::Array(keys)) => keys
                .into_iter()
                .map(|key| match key {
                    Value::String(text) => Ok(text),
                    other => Err(Error(format!("a key is not text: {other}"))),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(Error("a row has no keys".into())),
        };
        rows.insert(keys, record);
    }
    Ok(rows)
}

/// Where a load reads from: a held transaction or the pool.
enum Source<'a> {
    Tx(&'a mut Tx),
    Pool(&'a Database),
}

impl Source<'_> {
    fn query(
        &mut self,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<tokio_postgres::Row>, Error> {
        match self {
            Self::Tx(tx) => tx.query(sql, params),
            Self::Pool(database) => database.query(sql, params),
        }
    }

    fn database(&self) -> &Database {
        match self {
            Self::Tx(tx) => tx.database(),
            Self::Pool(database) => database,
        }
    }
}

fn load_from(source: &mut Source<'_>, layout: &Layout) -> Result<Option<(i64, Arc<Value>)>, Error> {
    let head = source.query(
        "SELECT revision FROM identity.stores WHERE store = $1",
        &[&layout.store],
    )?;
    let Some(head) = head.first() else {
        return Ok(None);
    };
    let revision: i64 = head.get(0);
    if let Some(doc) = source.database().cached(layout.store, revision) {
        return Ok(Some((revision, doc)));
    }
    let rows = source.query(&read_sql(layout), &[&layout.store])?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };
    let revision: i64 = row.get(0);
    let rest: Value = row.get(1);
    let mut all = Vec::with_capacity(layout.collections.len());
    for index in 0..layout.collections.len() {
        all.push(rows_of(row.get(index + 2))?);
    }
    let doc = Arc::new(assemble(layout, rest, all)?);
    source
        .database()
        .remember(layout.store, revision, doc.clone());
    Ok(Some((revision, doc)))
}

/// The store's document read in `tx`.
pub fn load_tx(tx: &mut Tx, layout: &Layout) -> Result<Option<(i64, Arc<Value>)>, Error> {
    load_from(&mut Source::Tx(tx), layout)
}

/// The store's current document and revision, or `None` when the store
/// was never installed. Inside this thread's writer lock it reads in the
/// lock's transaction; otherwise one snapshot from the pool.
pub fn load(database: &Database, layout: &Layout) -> Result<Option<(i64, Arc<Value>)>, Error> {
    if super::holds(layout.store) {
        in_store_tx(database, layout.store, |tx| {
            load_from(&mut Source::Tx(tx), layout)
        })
    } else {
        load_from(&mut Source::Pool(database), layout)
    }
}

/// Write `doc` as the store's new revision: the rows that changed, the
/// rest, and (for a sealed store) the archived revision. Runs in this
/// thread's writer lock when held (and commits it, taking the lock again
/// for whatever the holder does next), else in a transaction of its own.
pub fn save(database: &Database, layout: &Layout, doc: &Value) -> Result<i64, Error> {
    let holding = super::holds(layout.store);
    let revision = in_store_tx(database, layout.store, |tx| write(tx, layout, doc))?;
    database.forget(layout.store);
    if holding {
        // Make the write durable before the caller is told it happened;
        // the holder keeps the store's lock in a new transaction.
        super::recommit(database, layout.store)?;
    }
    Ok(revision)
}

/// Write `doc` in `tx`, which holds the store's lock.
pub fn write(tx: &mut Tx, layout: &Layout, doc: &Value) -> Result<i64, Error> {
    let current = load_from(&mut Source::Tx(tx), layout)?;
    let (old_rest, old_rows) = match &current {
        Some((_, doc)) => {
            let (rest, rows) = explode(layout, (**doc).clone())?;
            (Some(rest), rows)
        }
        None => (
            None,
            layout.collections.iter().map(|_| Rows::new()).collect(),
        ),
    };
    let (rest, new_rows) = explode(layout, doc.clone())?;
    // Deletes first, children first; then upserts, parents first. The
    // foreign keys and unique constraints are deferred to the end anyway.
    for (collection, (old, new)) in layout
        .collections
        .iter()
        .zip(old_rows.iter().zip(new_rows.iter()))
        .rev()
    {
        for keys in old.keys().filter(|keys| !new.contains_key(*keys)) {
            delete_row(tx, collection, keys)?;
        }
    }
    for (collection, (old, new)) in layout
        .collections
        .iter()
        .zip(old_rows.iter().zip(new_rows.iter()))
    {
        for (keys, record) in new {
            if old.get(keys) != Some(record) {
                upsert_row(tx, collection, keys, record)?;
            }
        }
    }
    let revision = match current {
        Some((revision, _)) => {
            if old_rest.as_ref() != Some(&rest) {
                tx.execute(
                    "UPDATE identity.stores SET revision = revision + 1, rest = $2, updated_at = now() WHERE store = $1",
                    &[&layout.store, &rest],
                )?;
            } else {
                tx.execute(
                    "UPDATE identity.stores SET revision = revision + 1, updated_at = now() WHERE store = $1",
                    &[&layout.store],
                )?;
            }
            revision + 1
        }
        None => {
            tx.execute(
                "INSERT INTO identity.stores (store, revision, rest) VALUES ($1, 1, $2)",
                &[&layout.store, &rest],
            )?;
            1
        }
    };
    if layout.revisions {
        archive(tx, layout.store, doc)?;
    }
    // Check the deferred constraints now, so a violation is this save's
    // error rather than a failed commit nobody hears about.
    tx.execute("SET CONSTRAINTS ALL IMMEDIATE", &[])?;
    tx.execute("SET CONSTRAINTS ALL DEFERRED", &[])?;
    Ok(revision)
}

/// Archive a sealed document under its digest. The same digest twice is
/// the same revision; a different document under it is refused.
pub fn archive(tx: &mut Tx, store: &str, doc: &Value) -> Result<(), Error> {
    let digest = doc
        .get("digest")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if digest.is_empty() {
        return Ok(());
    }
    let sequence = doc.get("sequence").and_then(Value::as_i64);
    let supersedes = doc.get("supersedes").and_then(Value::as_str);
    let inserted = tx.execute(
        "INSERT INTO audit.revisions (store, digest, sequence, supersedes, document)
         VALUES ($1, $2, $3, $4, $5) ON CONFLICT (store, digest) DO NOTHING",
        &[&store, &digest, &sequence, &supersedes, doc],
    )?;
    if inserted == 0 {
        let same = tx.query(
            "SELECT document = $3 FROM audit.revisions WHERE store = $1 AND digest = $2",
            &[&store, &digest, doc],
        )?;
        if !same.first().is_some_and(|row| row.get::<_, bool>(0)) {
            return Err(Error("archived revision content mismatch".into()));
        }
    }
    Ok(())
}

/// A sealed revision of `store` by digest.
pub fn revision(database: &Database, store: &str, digest: &str) -> Result<Option<Value>, Error> {
    let rows = database.query(
        "SELECT document FROM audit.revisions WHERE store = $1 AND digest = $2",
        &[&store, &digest],
    )?;
    Ok(rows.first().map(|row| row.get(0)))
}

fn columns(collection: &Collection) -> Vec<&'static str> {
    let mut columns: Vec<&str> = collection.keys.to_vec();
    columns.extend(collection.fixed.iter().map(|(column, _)| *column));
    columns
}

fn upsert_row(
    tx: &mut Tx,
    collection: &Collection,
    keys: &[String],
    record: &Value,
) -> Result<(), Error> {
    let columns = columns(collection);
    let mut values: Vec<String> = Vec::new();
    for index in 0..collection.keys.len() {
        values.push(format!("${}", index + 1));
    }
    for (_, value) in collection.fixed {
        values.push(format!("'{}'", value.replace('\'', "''")));
    }
    let record_at = collection.keys.len() + 1;
    let sql = format!(
        "INSERT INTO {} ({}, record) VALUES ({}, ${record_at}) ON CONFLICT ({}) DO UPDATE SET record = EXCLUDED.record",
        collection.table,
        columns.join(", "),
        values.join(", "),
        columns.join(", "),
    );
    let mut params: Vec<&(dyn ToSql + Sync)> =
        keys.iter().map(|k| k as &(dyn ToSql + Sync)).collect();
    params.push(record);
    tx.execute(&sql, &params)?;
    Ok(())
}

fn delete_row(tx: &mut Tx, collection: &Collection, keys: &[String]) -> Result<(), Error> {
    let mut clauses: Vec<String> = collection
        .keys
        .iter()
        .enumerate()
        .map(|(index, column)| format!("{column} = ${}", index + 1))
        .collect();
    for (column, value) in collection.fixed {
        clauses.push(format!("{column} = '{}'", value.replace('\'', "''")));
    }
    let sql = format!(
        "DELETE FROM {} WHERE {}",
        collection.table,
        clauses.join(" AND ")
    );
    let params: Vec<&(dyn ToSql + Sync)> = keys.iter().map(|k| k as &(dyn ToSql + Sync)).collect();
    tx.execute(&sql, &params)?;
    Ok(())
}

// --- The layouts of the stores kept in the database ---

/// `accounts.json`.
pub const ACCOUNTS: Layout = Layout {
    store: "accounts",
    revisions: true,
    collections: &[
        Collection {
            table: "identity.accounts",
            path: &[Seg::Field("accounts")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "workspace.workspaces",
            path: &[Seg::Field("workspaces")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "workspace.memberships",
            path: &[Seg::Field("workspaces"), Seg::Each, Seg::Field("members")],
            keys: &["workspace_id", "account_id"],
            fixed: &[],
        },
        Collection {
            table: "workspace.invitations",
            path: &[Seg::Field("invitations")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "identity.linked_identities",
            path: &[Seg::Field("identities"), Seg::Field("github")],
            keys: &["provider_id"],
            fixed: &[("provider", "github")],
        },
    ],
};

/// `sessions.json`.
pub const SESSIONS: Layout = Layout {
    store: "sessions",
    revisions: true,
    collections: &[
        Collection {
            table: "identity.account_sessions",
            path: &[Seg::Field("book"), Seg::Field("sessions")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "identity.device_sign_ins",
            path: &[Seg::Field("book"), Seg::Field("devices")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "identity.recoveries",
            path: &[Seg::Field("book"), Seg::Field("recoveries")],
            keys: &["id"],
            fixed: &[],
        },
        Collection {
            table: "identity.credentials",
            path: &[Seg::Field("book"), Seg::Field("credentials")],
            keys: &["account_id"],
            fixed: &[],
        },
        Collection {
            table: "identity.onboarding_budgets",
            path: &[Seg::Field("book"), Seg::Field("onboarding")],
            keys: &["id"],
            fixed: &[],
        },
    ],
};

/// `keys.json`.
pub const KEYS: Layout = Layout {
    store: "keys",
    revisions: false,
    collections: &[Collection {
        table: "identity.bearer_keys",
        path: &[Seg::Field("keys")],
        keys: &["id"],
        fixed: &[],
    }],
};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_document_splits_into_rows_and_back() {
        let doc = json!({
            "v": "x", "sequence": 3, "digest": "sha256:ab",
            "accounts": {"a1": {"id": "a1", "principals": ["key:1"]}},
            "workspaces": {"w1": {"id": "w1", "members": {"a1": {"account": "a1", "role": "owner"}}},
                           "w2": {"id": "w2", "members": {}}},
            "invitations": {},
            "identities": {"github": {"7": {"account": "a1"}}},
            "referrals": {"sources": []}
        });
        let (rest, rows) = explode(&ACCOUNTS, doc.clone()).unwrap();
        assert_eq!(
            rest,
            json!({"v": "x", "sequence": 3, "digest": "sha256:ab", "identities": {}, "referrals": {"sources": []}})
        );
        assert_eq!(rows[0].len(), 1);
        assert_eq!(rows[1].len(), 2);
        assert_eq!(
            rows[2].keys().next().unwrap(),
            &vec!["w1".to_string(), "a1".to_string()]
        );
        let back = assemble(&ACCOUNTS, rest, rows).unwrap();
        assert_eq!(back, doc);
    }
}
