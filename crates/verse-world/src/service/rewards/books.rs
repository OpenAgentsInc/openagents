//! Per-character receipt namespaces remain stable when an actor changes worlds.
use super::*;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::service) struct Book {
    pub(in crate::service) character: u64,
    base_revision: u64,
    pub(super) revision: u64,
    root: history::Root,
    legacy_root: history::Root,
    legacy_actor: u64,
    legacy_instance: u64,
}
fn portable(source: &[u8; 32]) -> bool {
    super::super::items::reserved(source)
        || super::super::outfits::reserved(source)
        || super::super::equipment::reserved(source)
        || super::super::progression::reserved(source)
}
fn normalized(mut source: [u8; 32]) -> [u8; 32] {
    if portable(&source) {
        source[8..16].fill(0);
    }
    source
}
pub(super) fn equivalent(a: &Transaction, b: &Transaction) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.actor = 1;
    b.actor = 1;
    a.instance = 1;
    b.instance = 1;
    a.source = normalized(a.source);
    b.source = normalized(b.source);
    a == b
}
impl Ledger {
    pub(in crate::service) fn realm_character(&self, actor: u64) -> Option<u64> {
        self.books.as_ref()?.get(&actor).map(|book| book.character)
    }
    pub(in crate::service) fn activate_books(
        &mut self,
        assignments: &[(u64, u64)],
        instance: u64,
    ) -> Result<(), String> {
        if let Some(books) = &self.books {
            if assignments
                .iter()
                .any(|(actor, id)| books.get(actor).is_none_or(|b| b.character != *id))
            {
                return Err("Realm receipt namespace is incompatible".into());
            }
            return Ok(());
        }
        if self
            .characters
            .keys()
            .any(|actor| !assignments.iter().any(|(a, _)| a == actor))
        {
            return Err("Realm rewards require registered character ownership".into());
        }
        let archive = self
            .archive
            .as_ref()
            .ok_or("Realm receipts require durable history")?;
        let mut root = self.root;
        for batch in self.receipts.chunks(ACTIVE_RECEIPTS) {
            root = archive.insert(root, batch)?;
        }
        let mut books = BTreeMap::new();
        let mut identities = BTreeSet::new();
        for &(actor, character) in assignments {
            if actor == 0
                || character == 0
                || !identities.insert(character)
                || books.contains_key(&actor)
            {
                return Err("Realm character receipt identity is duplicated".into());
            }
            books.insert(
                actor,
                Book {
                    character,
                    base_revision: self.revision,
                    revision: self.revision,
                    root: None,
                    legacy_root: root,
                    legacy_actor: actor,
                    legacy_instance: instance,
                },
            );
        }
        for &(actor, _) in assignments {
            self.characters.entry(actor).or_default();
        }
        self.root = root;
        self.receipts.clear();
        self.legacy_revision = Some(self.revision);
        self.books = Some(books);
        Ok(())
    }
    pub(super) fn book_receipt(
        &self,
        actor: u64,
        source: [u8; 32],
    ) -> Result<Option<Receipt>, String> {
        let book = self
            .books
            .as_ref()
            .and_then(|b| b.get(&actor))
            .ok_or("Realm reward character is not registered")?;
        let archive = self
            .archive
            .as_ref()
            .ok_or("Realm receipt history is missing")?;
        if let Some(receipt) =
            archive.character_receipt(book.root, book.character, normalized(source))?
        {
            return Ok(Some(receipt));
        }
        let mut legacy_source = source;
        if portable(&legacy_source) {
            legacy_source[8..16].copy_from_slice(&book.legacy_instance.to_be_bytes());
        }
        archive.get(book.legacy_root, book.legacy_actor, legacy_source)
    }
    pub(super) fn commit_book(
        &mut self,
        actor: u64,
        source: [u8; 32],
        mut receipt: Receipt,
        next: Character,
    ) -> Result<Receipt, String> {
        let book = self
            .books
            .as_ref()
            .and_then(|b| b.get(&actor))
            .ok_or("Realm reward character is not registered")?;
        let revision = book
            .revision
            .checked_add(1)
            .ok_or("Realm character receipt revisions exhausted")?;
        let global = self
            .revision
            .checked_add(1)
            .ok_or("Reward revisions exhausted")?;
        receipt.revision = revision;
        let archive = self
            .archive
            .as_ref()
            .ok_or("Realm receipt history is missing")?;
        let root = archive.insert_character(
            book.root,
            book.character,
            normalized(source),
            receipt.clone(),
        )?;
        let book = self.books.as_mut().unwrap().get_mut(&actor).unwrap();
        book.revision = revision;
        book.root = root;
        self.characters.insert(actor, next);
        self.revision = global;
        Ok(receipt)
    }
    pub(in crate::service) fn register_book(
        &mut self,
        actor: u64,
        character: u64,
        instance: u64,
    ) -> Result<(), String> {
        if character == 0 || instance == 0 {
            return Err("Realm character identity is empty".into());
        }
        self.put_book(
            actor,
            Book {
                character,
                base_revision: 0,
                revision: 0,
                root: None,
                legacy_root: None,
                legacy_actor: actor,
                legacy_instance: instance,
            },
            Character::default(),
        )
    }
    pub(in crate::service) fn take_book(
        &mut self,
        actor: u64,
    ) -> Result<(Book, Character), String> {
        let book = self
            .books
            .as_mut()
            .and_then(|b| b.remove(&actor))
            .ok_or("Realm character receipt namespace is missing")?;
        let character = self
            .characters
            .remove(&actor)
            .ok_or("Realm character state is missing")?;
        Ok((book, character))
    }
    pub(in crate::service) fn put_book(
        &mut self,
        actor: u64,
        book: Book,
        character: Character,
    ) -> Result<(), String> {
        let books = self
            .books
            .as_mut()
            .ok_or("Destination is not a realm receipt ledger")?;
        if actor == 0
            || books.contains_key(&actor)
            || books.values().any(|b| b.character == book.character)
            || self.characters.contains_key(&actor)
            || self.characters.len() >= MAX_CHARACTERS
        {
            return Err("Destination realm character receipt identity is incompatible".into());
        }
        self.revision = self.revision.max(book.revision);
        books.insert(actor, book);
        self.characters.insert(actor, character);
        Ok(())
    }
}
pub(super) fn validate(
    books: &BTreeMap<u64, Book>,
    characters: &BTreeMap<u64, Character>,
    archive: &history::History,
) -> Result<(), String> {
    if books.len() != characters.len() {
        return Err("Realm receipt bindings do not cover character state".into());
    }
    let mut ids = BTreeSet::new();
    for (actor, book) in books {
        if !characters.contains_key(actor)
            || book.character == 0
            || book.legacy_actor == 0
            || book.legacy_instance == 0
            || !ids.insert(book.character)
        {
            return Err("Saved realm receipt binding is invalid".into());
        }
        archive.validate(book.legacy_root, book.base_revision)?;
        archive.validate_character(book.root, book.character, book.base_revision, book.revision)?;
    }
    Ok(())
}
