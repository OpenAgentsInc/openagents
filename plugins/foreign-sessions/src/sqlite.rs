//! A minimal, read-only SQLite file reader over bounded range reads.
//!
//! The opencode CLI keeps its sessions in `~/.local/share/opencode/opencode.db`
//! and Devin keeps per-session ACP message databases; both are ordinary
//! SQLite files, and both can exceed the host's whole-file read bound. This
//! module reads them the way SQLite's format was designed to be read —
//! page at a time, through the bounded `read_file_range` import — without a
//! C-backed SQLite crate, which would not build for `wasm32-unknown-unknown`.
//!
//! Scope is deliberately narrow: the 100-byte header, table b-trees
//! (interior `0x05` / leaf `0x0d`), index b-trees (`0x02` / `0x0a`),
//! varints, record serial types, and overflow-page chains. UTF-8 text
//! encoding only. Nothing here writes, and the write-ahead log is *never*
//! parsed — a caller that sees a `-wal` file beside the database discloses
//! it instead (`wal_unread`), because rows not yet checkpointed are
//! invisible to this reader.
//!
//! The posture is the plugin family's: foreign bytes are untrusted input,
//! so every page number, offset, varint, and length is checked, every walk
//! is depth- and budget-bounded, and anything unexpected is a typed
//! [`Refusal`] — never a panic.

use openagents_pdk::Refusal;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::rc::Rc;

/// Bytes at rest, addressable by bounded range reads. The real source is a
/// mounted file behind the host's `read_file_range` import; tests read from
/// an in-memory buffer.
pub trait ByteSource {
    fn read_at(&self, offset: u64, len: u32) -> Result<Vec<u8>, Refusal>;
}

impl ByteSource for [u8] {
    fn read_at(&self, offset: u64, len: u32) -> Result<Vec<u8>, Refusal> {
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(self.len());
        let end = start.saturating_add(len as usize).min(self.len());
        Ok(self[start..end].to_vec())
    }
}

/// A file inside one of the manifest's read-only mounts, as a [`ByteSource`].
/// The path is mount-relative, exactly as [`crate::Host::read_range`] takes it.
pub struct MountedFile<'a> {
    pub host: &'a dyn crate::Host,
    pub path: String,
}

impl ByteSource for MountedFile<'_> {
    fn read_at(&self, offset: u64, len: u32) -> Result<Vec<u8>, Refusal> {
        self.host.read_range(&self.path, offset, len)
    }
}

/// One decoded column value of a record.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Int(i64),
    Float(f64),
    Text(String),
    Blob(Vec<u8>),
    /// A value the per-row payload cap cut short; carries the bytes that
    /// were within reach (possibly none), so a caller can still sniff a
    /// prefix before deciding to re-read the row with a larger cap.
    Truncated(Vec<u8>),
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(value) => Some(*value),
            _ => None,
        }
    }
}

/// One row of `sqlite_master`: a table or index, its root page, and the
/// column order parsed from its `CREATE` statement — parsed, not assumed,
/// because ORM-managed schemas order columns however their migrations did.
#[derive(Debug, Clone)]
pub struct MasterEntry {
    /// `"table"`, `"index"`, `"view"`, or `"trigger"`.
    pub kind: String,
    pub name: String,
    pub tbl_name: String,
    pub rootpage: u32,
    pub sql: String,
    /// Column names in on-disk record order (tables) or key order (indexes).
    pub columns: Vec<String>,
}

fn corrupt(reason: &str) -> Refusal {
    Refusal::unsupported(format!("sqlite: {reason}"))
}

const MAX_DEPTH: usize = 32;
const CACHE_PAGES: usize = 1024;

/// A read-only view of one SQLite database file.
pub struct Sqlite<'a> {
    src: &'a dyn ByteSource,
    page_size: u32,
    usable: u64,
    /// In-header database size in pages; 0 when the header did not say.
    page_count: u32,
    /// Page reads still allowed; exhaustion is disclosed, not fatal-silent.
    budget: usize,
    pub pages_read: usize,
    pub bytes_read: u64,
    /// True once a read was refused because the budget ran out. A caller
    /// that sees an `Err` should check this to tell honest truncation from
    /// real corruption.
    pub budget_exhausted: bool,
    cache: BTreeMap<u32, Rc<Vec<u8>>>,
}

impl<'a> Sqlite<'a> {
    /// Open a database: read and validate the 100-byte header. Text
    /// encoding must be UTF-8; anything else is refused, not guessed at.
    pub fn open(src: &'a dyn ByteSource, page_budget: usize) -> Result<Self, Refusal> {
        let header = src.read_at(0, 100)?;
        if header.len() < 100 {
            return Err(corrupt("the file is shorter than a database header"));
        }
        if &header[0..16] != b"SQLite format 3\0" {
            return Err(corrupt("the magic bytes are not SQLite's"));
        }
        let raw = u16::from_be_bytes([header[16], header[17]]);
        let page_size: u32 = if raw == 1 { 65_536 } else { u32::from(raw) };
        if !(512..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(corrupt("the page size is out of range"));
        }
        let reserved = u32::from(header[20]);
        if reserved >= page_size {
            return Err(corrupt("the reserved-space byte exceeds the page size"));
        }
        let encoding = u32::from_be_bytes([header[56], header[57], header[58], header[59]]);
        if encoding != 1 {
            return Err(corrupt("only UTF-8 text encoding is supported"));
        }
        let page_count = u32::from_be_bytes([header[28], header[29], header[30], header[31]]);
        Ok(Sqlite {
            src,
            page_size,
            usable: u64::from(page_size - reserved),
            page_count,
            budget: page_budget,
            pages_read: 0,
            bytes_read: 100,
            budget_exhausted: false,
            cache: BTreeMap::new(),
        })
    }

    /// The whole `sqlite_master` table (root page 1): every table and index
    /// with its root page and parsed column order.
    pub fn master(&mut self) -> Result<Vec<MasterEntry>, Refusal> {
        let mut entries = Vec::new();
        self.scan_table(1, 262_144, &mut |_rowid, values| {
            let text = |at: usize| values.get(at).and_then(Value::as_str).map(str::to_string);
            let (Some(kind), Some(name), Some(tbl_name)) = (text(0), text(1), text(2)) else {
                return true; // Not a row this reader understands; move on.
            };
            let rootpage = values.get(3).and_then(Value::as_int).unwrap_or(0);
            let Ok(rootpage) = u32::try_from(rootpage) else {
                return true;
            };
            let sql = text(4).unwrap_or_default();
            let columns = columns_of(&sql);
            entries.push(MasterEntry { kind, name, tbl_name, rootpage, sql, columns });
            true
        })?;
        Ok(entries)
    }

    /// One page's bytes, cached. Page numbers are 1-based.
    fn page(&mut self, number: u32) -> Result<Rc<Vec<u8>>, Refusal> {
        if number == 0 || (self.page_count != 0 && number > self.page_count) {
            return Err(corrupt("a page number points outside the database"));
        }
        if let Some(hit) = self.cache.get(&number) {
            return Ok(hit.clone());
        }
        if self.budget == 0 {
            self.budget_exhausted = true;
            return Err(corrupt("the page-read budget is exhausted"));
        }
        self.budget -= 1;
        let offset = u64::from(number - 1) * u64::from(self.page_size);
        let bytes = self.src.read_at(offset, self.page_size)?;
        if bytes.len() != self.page_size as usize {
            return Err(corrupt("a page read came back short"));
        }
        self.pages_read += 1;
        self.bytes_read += bytes.len() as u64;
        if self.cache.len() >= CACHE_PAGES {
            self.cache.clear();
        }
        let page = Rc::new(bytes);
        self.cache.insert(number, page.clone());
        Ok(page)
    }

    /// Walk a table b-tree in rowid order, decoding each leaf record. The
    /// callback returns `false` to stop early. Records longer than
    /// `payload_cap` come back with [`Value::Truncated`] tails.
    pub fn scan_table(
        &mut self,
        root: u32,
        payload_cap: usize,
        on_row: &mut dyn FnMut(i64, Vec<Value>) -> bool,
    ) -> Result<(), Refusal> {
        self.walk_table(root, payload_cap, 0, on_row).map(|_| ())
    }

    fn walk_table(
        &mut self,
        page_no: u32,
        payload_cap: usize,
        depth: usize,
        on_row: &mut dyn FnMut(i64, Vec<Value>) -> bool,
    ) -> Result<bool, Refusal> {
        if depth > MAX_DEPTH {
            return Err(corrupt("the b-tree is deeper than any honest database"));
        }
        let page = self.page(page_no)?;
        let info = page_info(&page, page_no)?;
        match info.kind {
            0x0d => {
                for at in 0..info.ncells {
                    let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                    let (payload, rowid) = self.cell_payload(&page, cell, 0x0d, payload_cap)?;
                    let rowid = rowid.ok_or_else(|| corrupt("a table leaf cell has no rowid"))?;
                    let values = decode_record(&payload.bytes, payload.total)
                        .map_err(|()| corrupt("a row record does not decode"))?;
                    if !on_row(rowid, values) {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            0x05 => {
                for at in 0..info.ncells {
                    let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                    let child = read_u32(&page, cell)?;
                    if !self.walk_table(child, payload_cap, depth + 1, on_row)? {
                        return Ok(false);
                    }
                }
                self.walk_table(info.right_most, payload_cap, depth + 1, on_row)
            }
            other => Err(corrupt(&format!("unexpected table page type 0x{other:02x}"))),
        }
    }

    /// One row by rowid: a point lookup down the table b-tree.
    pub fn find_by_rowid(
        &mut self,
        root: u32,
        rowid: i64,
        payload_cap: usize,
    ) -> Result<Option<Vec<Value>>, Refusal> {
        let mut page_no = root;
        for _ in 0..MAX_DEPTH {
            let page = self.page(page_no)?;
            let info = page_info(&page, page_no)?;
            match info.kind {
                0x0d => {
                    for at in 0..info.ncells {
                        let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                        let (payload, found) = self.cell_payload(&page, cell, 0x0d, payload_cap)?;
                        if found == Some(rowid) {
                            let values = decode_record(&payload.bytes, payload.total)
                                .map_err(|()| corrupt("a row record does not decode"))?;
                            return Ok(Some(values));
                        }
                    }
                    return Ok(None);
                }
                0x05 => {
                    let mut next = info.right_most;
                    for at in 0..info.ncells {
                        let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                        let key = varint(&page, cell + 4)
                            .ok_or_else(|| corrupt("an interior cell key does not decode"))?
                            .0;
                        if rowid <= key {
                            next = read_u32(&page, cell)?;
                            break;
                        }
                    }
                    page_no = next;
                }
                other => {
                    return Err(corrupt(&format!("unexpected table page type 0x{other:02x}")))
                }
            }
        }
        Err(corrupt("the b-tree is deeper than any honest database"))
    }

    /// Walk an index b-tree, emitting every entry whose *first* key column
    /// equals `first` (BINARY collation: byte comparison), in key order.
    /// Subtrees that cannot contain a match are pruned, so this is a seek,
    /// not a full scan. Each emitted record's last value is the rowid.
    pub fn index_scan_eq(
        &mut self,
        root: u32,
        first: &str,
        payload_cap: usize,
        on_row: &mut dyn FnMut(Vec<Value>) -> bool,
    ) -> Result<(), Refusal> {
        self.walk_index_eq(root, first, payload_cap, 0, on_row).map(|_| ())
    }

    fn walk_index_eq(
        &mut self,
        page_no: u32,
        first: &str,
        payload_cap: usize,
        depth: usize,
        on_row: &mut dyn FnMut(Vec<Value>) -> bool,
    ) -> Result<bool, Refusal> {
        if depth > MAX_DEPTH {
            return Err(corrupt("the b-tree is deeper than any honest database"));
        }
        let page = self.page(page_no)?;
        let info = page_info(&page, page_no)?;
        match info.kind {
            0x0a => {
                for at in 0..info.ncells {
                    let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                    let (payload, _) = self.cell_payload(&page, cell, 0x0a, payload_cap)?;
                    let values = decode_record(&payload.bytes, payload.total)
                        .map_err(|()| corrupt("an index record does not decode"))?;
                    match cmp_first(&values, first) {
                        Ordering::Less => continue,
                        Ordering::Equal => {
                            if !on_row(values) {
                                return Ok(false);
                            }
                        }
                        Ordering::Greater => return Ok(false),
                    }
                }
                Ok(true)
            }
            0x02 => {
                for at in 0..info.ncells {
                    let cell = cell_offset(&page, info.cell_ptr_base, at)?;
                    let child = read_u32(&page, cell)?;
                    let (payload, _) = self.cell_payload(&page, cell + 4, 0x02, payload_cap)?;
                    let key = decode_record(&payload.bytes, payload.total)
                        .map_err(|()| corrupt("an index record does not decode"))?;
                    match cmp_first(&key, first) {
                        // Everything under this child sorts before the key,
                        // which itself sorts before the target: prune both.
                        Ordering::Less => continue,
                        Ordering::Equal => {
                            if !self.walk_index_eq(child, first, payload_cap, depth + 1, on_row)? {
                                return Ok(false);
                            }
                            if !on_row(key) {
                                return Ok(false);
                            }
                        }
                        Ordering::Greater => {
                            // The child may still hold trailing matches; the
                            // key itself is already past the target.
                            return self
                                .walk_index_eq(child, first, payload_cap, depth + 1, on_row)
                                .map(|_| false);
                        }
                    }
                }
                self.walk_index_eq(info.right_most, first, payload_cap, depth + 1, on_row)
            }
            other => Err(corrupt(&format!("unexpected index page type 0x{other:02x}"))),
        }
    }

    /// A cell's payload bytes (following overflow pages as needed, up to
    /// `cap`) plus its rowid for table-leaf cells. `at` addresses the first
    /// varint of the cell — for index interior cells, *after* the 4-byte
    /// child pointer.
    fn cell_payload(
        &mut self,
        page: &[u8],
        at: usize,
        kind: u8,
        cap: usize,
    ) -> Result<(Payload, Option<i64>), Refusal> {
        let (total, n1) =
            varint(page, at).ok_or_else(|| corrupt("a cell length does not decode"))?;
        if total < 0 {
            return Err(corrupt("a cell claims a negative payload length"));
        }
        let total = total as u64;
        let mut pos = at + n1;
        let mut rowid = None;
        if kind == 0x0d {
            let (id, n2) =
                varint(page, pos).ok_or_else(|| corrupt("a cell rowid does not decode"))?;
            rowid = Some(id);
            pos += n2;
        }

        let usable = self.usable;
        let x = if kind == 0x0d { usable - 35 } else { ((usable - 12) * 64 / 255) - 23 };
        let local = if total <= x {
            total
        } else {
            let m = ((usable - 12) * 32 / 255) - 23;
            let k = m + (total - m) % (usable - 4);
            if k <= x {
                k
            } else {
                m
            }
        };
        let local = local as usize;
        if pos + local > page.len() {
            return Err(corrupt("a cell's local payload runs off its page"));
        }

        let want = usize::try_from(total).unwrap_or(usize::MAX).min(cap);
        let mut bytes = Vec::with_capacity(want.min(local + 64));
        bytes.extend_from_slice(&page[pos..pos + local.min(want)]);

        if (total as usize) > local && bytes.len() < want {
            // The 4-byte pointer to the first overflow page sits after the
            // local payload; each overflow page starts with the next pointer.
            let mut next = read_u32(page, pos + local)?;
            let chunk = (usable - 4) as usize;
            let mut hops = want / chunk.max(1) + 2;
            while next != 0 && bytes.len() < want {
                if hops == 0 {
                    return Err(corrupt("an overflow chain loops or overruns its payload"));
                }
                hops -= 1;
                let overflow = self.page(next)?;
                next = read_u32(&overflow, 0)?;
                let take = chunk.min(want - bytes.len()).min(overflow.len() - 4);
                bytes.extend_from_slice(&overflow[4..4 + take]);
            }
        }
        Ok((Payload { total, bytes }, rowid))
    }
}

struct Payload {
    /// The record's full length on disk; `bytes` may hold less (the cap).
    total: u64,
    bytes: Vec<u8>,
}

struct PageInfo {
    kind: u8,
    ncells: usize,
    cell_ptr_base: usize,
    right_most: u32,
}

/// Parse a b-tree page's header. Page 1 carries the 100-byte file header
/// first; cell pointer offsets are always relative to the page start.
fn page_info(page: &[u8], number: u32) -> Result<PageInfo, Refusal> {
    let hdr = if number == 1 { 100 } else { 0 };
    if page.len() < hdr + 12 {
        return Err(corrupt("a page is too short for its header"));
    }
    let kind = page[hdr];
    let interior = kind == 0x02 || kind == 0x05;
    let ncells = u16::from_be_bytes([page[hdr + 3], page[hdr + 4]]) as usize;
    let cell_ptr_base = hdr + if interior { 12 } else { 8 };
    let right_most = if interior { read_u32(page, hdr + 8)? } else { 0 };
    if cell_ptr_base + 2 * ncells > page.len() {
        return Err(corrupt("a cell pointer array runs off its page"));
    }
    Ok(PageInfo { kind, ncells, cell_ptr_base, right_most })
}

fn cell_offset(page: &[u8], base: usize, index: usize) -> Result<usize, Refusal> {
    let at = base + 2 * index;
    let offset = u16::from_be_bytes([page[at], page[at + 1]]) as usize;
    if offset >= page.len() {
        return Err(corrupt("a cell pointer points off its page"));
    }
    Ok(offset)
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, Refusal> {
    if at + 4 > bytes.len() {
        return Err(corrupt("a page number field runs off its page"));
    }
    Ok(u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]))
}

/// Decode a SQLite varint: 1–9 bytes, big-endian base-128, the ninth byte
/// contributing all eight bits. Answers the value and the bytes consumed.
pub fn varint(buf: &[u8], at: usize) -> Option<(i64, usize)> {
    let mut value: i64 = 0;
    for step in 0..9 {
        let byte = *buf.get(at + step)?;
        if step == 8 {
            return Some(((value << 8) | i64::from(byte), 9));
        }
        value = (value << 7) | i64::from(byte & 0x7f);
        if byte & 0x80 == 0 {
            return Some((value, step + 1));
        }
    }
    None
}

/// Decode a record's serial-type header and values. `total` is the record's
/// on-disk length; when `payload` holds less, values past the cut come back
/// as [`Value::Truncated`] instead of failing the whole row. `Err(())` means
/// the record is malformed (or so heavily capped its header is unreadable).
pub fn decode_record(payload: &[u8], total: u64) -> Result<Vec<Value>, ()> {
    let (header_len, n) = varint(payload, 0).ok_or(())?;
    if header_len < 0 {
        return Err(());
    }
    let header_len = header_len as u64;
    if header_len > total || header_len as usize > payload.len() {
        return Err(()); // The header itself is cut or lying.
    }
    let mut types = Vec::new();
    let mut pos = n;
    while (pos as u64) < header_len {
        let (serial, used) = varint(payload, pos).ok_or(())?;
        types.push(serial);
        pos += used;
    }
    if pos as u64 != header_len {
        return Err(());
    }

    let mut values = Vec::with_capacity(types.len());
    let mut body = header_len as usize;
    for serial in types {
        let size = serial_size(serial).ok_or(())? as usize;
        let end = body.saturating_add(size);
        if end as u64 > total {
            return Err(()); // The record claims more bytes than it has.
        }
        if end <= payload.len() {
            values.push(decode_value(serial, &payload[body..end]));
        } else if body < payload.len() {
            values.push(Value::Truncated(payload[body..].to_vec()));
        } else {
            values.push(Value::Truncated(Vec::new()));
        }
        body = end;
    }
    Ok(values)
}

fn serial_size(serial: i64) -> Option<u64> {
    match serial {
        0 | 8 | 9 => Some(0),
        1 => Some(1),
        2 => Some(2),
        3 => Some(3),
        4 => Some(4),
        5 => Some(6),
        6 | 7 => Some(8),
        n if n >= 12 => Some((n as u64 - 12) / 2),
        _ => None,
    }
}

fn decode_value(serial: i64, bytes: &[u8]) -> Value {
    match serial {
        0 => Value::Null,
        8 => Value::Int(0),
        9 => Value::Int(1),
        1..=6 => {
            let mut value: i64 = if bytes.first().is_some_and(|b| b & 0x80 != 0) { -1 } else { 0 };
            for byte in bytes {
                value = (value << 8) | i64::from(*byte);
            }
            Value::Int(value)
        }
        7 => {
            let mut raw = [0u8; 8];
            raw.copy_from_slice(bytes);
            Value::Float(f64::from_be_bytes(raw))
        }
        n if n >= 13 && n % 2 == 1 => Value::Text(String::from_utf8_lossy(bytes).into_owned()),
        _ => Value::Blob(bytes.to_vec()),
    }
}

/// SQLite's BINARY-collation ordering of a record's first column against a
/// target text value: NULLs and numbers sort before text, blobs after.
fn cmp_first(values: &[Value], target: &str) -> Ordering {
    match values.first() {
        Some(Value::Text(text)) => text.as_bytes().cmp(target.as_bytes()),
        Some(Value::Blob(_)) => Ordering::Greater,
        // NULL, numbers, a pruned prefix, or nothing: sorts before text.
        _ => Ordering::Less,
    }
}

/// Column names in declaration order, parsed from a `CREATE TABLE` or
/// `CREATE INDEX` statement: the comma-separated head identifiers between
/// the statement's parentheses, with table-level constraints skipped.
/// Answering the *actual* order matters because ORM-managed schemas
/// (drizzle here) order columns by migration history, not by intuition.
pub fn columns_of(sql: &str) -> Vec<String> {
    let Some(open) = sql.find('(') else {
        return Vec::new();
    };
    let body = &sql[open + 1..];
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut end = body.len();
    for (at, ch) in body.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    end = at;
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    let mut columns = Vec::new();
    for segment in split_top_level(&body[..end]) {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let name = head_identifier(segment);
        if name.is_empty() {
            continue;
        }
        let upper = name.to_ascii_uppercase();
        if matches!(upper.as_str(), "CONSTRAINT" | "PRIMARY" | "FOREIGN" | "UNIQUE" | "CHECK") {
            continue;
        }
        columns.push(name);
    }
    columns
}

/// Split on commas that sit outside parentheses and quotes.
fn split_top_level(body: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    let mut start = 0usize;
    for (at, ch) in body.char_indices() {
        if let Some(q) = quote {
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' | '`' => quote = Some(ch),
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&body[start..at]);
                start = at + 1;
            }
            _ => {}
        }
    }
    parts.push(&body[start..]);
    parts
}

/// The first identifier of a column definition, unquoting `` ` ``, `"`,
/// and `[...]` styles.
fn head_identifier(segment: &str) -> String {
    let segment = segment.trim_start();
    let mut chars = segment.chars();
    match chars.next() {
        Some(open @ ('`' | '"')) => chars.take_while(|c| *c != open).collect(),
        Some('[') => chars.take_while(|c| *c != ']').collect(),
        Some(first) if first.is_ascii_alphanumeric() || first == '_' => {
            std::iter::once(first)
                .chain(chars.take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$'))
                .collect()
        }
        _ => String::new(),
    }
}

/// Sniff a JSON object's `"type"` (or any named string field) from a byte
/// prefix — for rows whose payload was capped before the JSON could parse.
pub fn sniff_json_str(prefix: &[u8], field: &str) -> Option<String> {
    let text = String::from_utf8_lossy(prefix);
    let needle = format!("\"{field}\"");
    let at = text.find(&needle)?;
    let rest = &text[at + needle.len()..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
