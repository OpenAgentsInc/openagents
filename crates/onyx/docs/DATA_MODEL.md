# Onyx Data Model

Document format specification for Onyx notes.

## File Format

Onyx uses plain UTF-8 Markdown files (.md) with optional YAML frontmatter.

### Basic Structure

```markdown
---
title: My Note Title
tags: [idea, project]
created: 2024-01-15T10:30:00Z
modified: 2024-01-16T14:22:00Z
---

# My Note Title

Content goes here with **bold**, *italic*, and [[wiki-links]].
```

### No Frontmatter

Files without frontmatter are valid. Metadata is inferred:
- `title`: First H1 heading, or filename without extension
- `tags`: Empty
- `created`: File creation time from filesystem
- `modified`: File modification time from filesystem

```markdown
# Quick Note

Just some content without frontmatter.
```

## Frontmatter Schema

YAML frontmatter is delimited by `---` on its own line.

### Required Fields

None. All fields are optional.

### Standard Fields

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Note title (overrides H1) |
| `tags` | string[] | List of tags for categorization |
| `created` | datetime | ISO 8601 creation timestamp |
| `modified` | datetime | ISO 8601 last modified timestamp |
| `aliases` | string[] | Alternative names for wiki-link resolution |

### Custom Fields

Users can add arbitrary fields. Onyx preserves them:

```yaml
---
title: Project Notes
status: in-progress
priority: high
custom_field: any value
---
```

### Rust Types

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Frontmatter {
    /// Note title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Tags for categorization
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<DateTime<Utc>>,

    /// Last modified timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<DateTime<Utc>>,

    /// Alternative names for linking
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,

    /// Custom user-defined fields (preserved on save)
    #[serde(flatten)]
    pub custom: HashMap<String, serde_yaml::Value>,
}
```

### Parsing

```rust
pub fn parse_document(content: &str) -> (Frontmatter, &str) {
    if content.starts_with("---\n") {
        if let Some(end) = content[4..].find("\n---\n") {
            let yaml = &content[4..4 + end];
            let body = &content[4 + end + 5..];
            let frontmatter: Frontmatter = serde_yaml::from_str(yaml)
                .unwrap_or_default();
            return (frontmatter, body);
        }
    }
    (Frontmatter::default(), content)
}

pub fn serialize_document(frontmatter: &Frontmatter, body: &str) -> String {
    let yaml = serde_yaml::to_string(frontmatter).unwrap();
    format!("---\n{}---\n\n{}", yaml, body)
}
```

## Wiki-Links

Onyx supports Obsidian-style wiki-links for connecting notes.

### Syntax

```markdown
Link to [[another note]]
Link with [[another note|display text]]
Link to heading [[note#heading]]
Link to block [[note#^block-id]]
```

### Resolution Algorithm

1. Exact filename match (case-insensitive)
2. Match against `aliases` in frontmatter
3. Fuzzy match (for suggestions)

```rust
pub fn resolve_link(link: &str, vault: &Vault) -> Option<PathBuf> {
    let (note_name, _section) = parse_link_parts(link);

    // 1. Exact filename match
    for file in vault.files() {
        let stem = file.file_stem()?.to_str()?;
        if stem.eq_ignore_ascii_case(note_name) {
            return Some(file);
        }
    }

    // 2. Alias match
    for file in vault.files() {
        let doc = Document::load(&file).ok()?;
        if doc.frontmatter.aliases.iter()
            .any(|a| a.eq_ignore_ascii_case(note_name))
        {
            return Some(file);
        }
    }

    None
}

fn parse_link_parts(link: &str) -> (&str, Option<&str>) {
    if let Some(hash_pos) = link.find('#') {
        (&link[..hash_pos], Some(&link[hash_pos + 1..]))
    } else {
        (link, None)
    }
}
```

### Link Extraction

```rust
use regex::Regex;

pub fn extract_links(content: &str) -> Vec<WikiLink> {
    let re = Regex::new(r"\[\[([^\]|]+)(?:\|([^\]]+))?\]\]").unwrap();
    let mut links = Vec::new();

    for cap in re.captures_iter(content) {
        links.push(WikiLink {
            target: cap[1].to_string(),
            display: cap.get(2).map(|m| m.as_str().to_string()),
            start: cap.get(0).unwrap().start(),
            end: cap.get(0).unwrap().end(),
        });
    }

    links
}

#[derive(Debug, Clone)]
pub struct WikiLink {
    /// Target note name
    pub target: String,
    /// Optional display text
    pub display: Option<String>,
    /// Start offset in source
    pub start: usize,
    /// End offset in source
    pub end: usize,
}
```

## Index Format

SQLite database for fast searching and backlink computation.

### Location

`~/.onyx/index.db` or `<vault>/.onyx/index.db`

### Schema

```sql
-- Notes table
CREATE TABLE notes (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    title TEXT,
    content TEXT,
    frontmatter_json TEXT,
    created_at TEXT,
    modified_at TEXT,
    indexed_at TEXT NOT NULL
);

-- Tags table (normalized)
CREATE TABLE tags (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL
);

-- Note-tag relationships
CREATE TABLE note_tags (
    note_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
    tag_id INTEGER REFERENCES tags(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, tag_id)
);

-- Links table
CREATE TABLE links (
    id INTEGER PRIMARY KEY,
    source_id INTEGER REFERENCES notes(id) ON DELETE CASCADE,
    target_name TEXT NOT NULL,  -- Raw link target (may not resolve)
    target_id INTEGER REFERENCES notes(id) ON DELETE SET NULL,
    display_text TEXT
);

-- Full-text search
CREATE VIRTUAL TABLE notes_fts USING fts5(
    title,
    content,
    content='notes',
    content_rowid='id'
);

-- Triggers to keep FTS in sync
CREATE TRIGGER notes_ai AFTER INSERT ON notes BEGIN
    INSERT INTO notes_fts(rowid, title, content)
    VALUES (new.id, new.title, new.content);
END;

CREATE TRIGGER notes_ad AFTER DELETE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
    VALUES ('delete', old.id, old.title, old.content);
END;

CREATE TRIGGER notes_au AFTER UPDATE ON notes BEGIN
    INSERT INTO notes_fts(notes_fts, rowid, title, content)
    VALUES ('delete', old.id, old.title, old.content);
    INSERT INTO notes_fts(rowid, title, content)
    VALUES (new.id, new.title, new.content);
END;

-- Indexes
CREATE INDEX idx_notes_modified ON notes(modified_at);
CREATE INDEX idx_links_source ON links(source_id);
CREATE INDEX idx_links_target ON links(target_id);
CREATE INDEX idx_links_target_name ON links(target_name);
```

### Rust Types

```rust
use rusqlite::{Connection, params};
use std::path::PathBuf;

pub struct Index {
    conn: Connection,
}

impl Index {
    pub fn open(path: &Path) -> Result<Self, Error> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// Index a note (insert or update)
    pub fn index_note(&self, note: &IndexedNote) -> Result<(), Error> {
        self.conn.execute(
            "INSERT OR REPLACE INTO notes (path, title, content, frontmatter_json, created_at, modified_at, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, datetime('now'))",
            params![
                note.path.to_str(),
                note.title,
                note.content,
                serde_json::to_string(&note.frontmatter)?,
                note.created_at.map(|d| d.to_rfc3339()),
                note.modified_at.map(|d| d.to_rfc3339()),
            ],
        )?;
        Ok(())
    }

    /// Search notes by query
    pub fn search(&self, query: &str) -> Result<Vec<SearchResult>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT n.path, n.title, snippet(notes_fts, 1, '<mark>', '</mark>', '...', 32)
             FROM notes_fts
             JOIN notes n ON notes_fts.rowid = n.id
             WHERE notes_fts MATCH ?1
             ORDER BY rank
             LIMIT 50"
        )?;

        let results = stmt.query_map([query], |row| {
            Ok(SearchResult {
                path: PathBuf::from(row.get::<_, String>(0)?),
                title: row.get(1)?,
                snippet: row.get(2)?,
            })
        })?;

        results.collect()
    }

    /// Get backlinks to a note
    pub fn backlinks(&self, path: &Path) -> Result<Vec<Backlink>, Error> {
        let mut stmt = self.conn.prepare(
            "SELECT n.path, n.title, l.display_text
             FROM links l
             JOIN notes n ON l.source_id = n.id
             JOIN notes target ON l.target_id = target.id
             WHERE target.path = ?1"
        )?;

        let results = stmt.query_map([path.to_str()], |row| {
            Ok(Backlink {
                source_path: PathBuf::from(row.get::<_, String>(0)?),
                source_title: row.get(1)?,
                context: row.get(2)?,
            })
        })?;

        results.collect()
    }
}

#[derive(Debug)]
pub struct IndexedNote {
    pub path: PathBuf,
    pub title: String,
    pub content: String,
    pub frontmatter: Frontmatter,
    pub created_at: Option<DateTime<Utc>>,
    pub modified_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct SearchResult {
    pub path: PathBuf,
    pub title: String,
    pub snippet: String,
}

#[derive(Debug)]
pub struct Backlink {
    pub source_path: PathBuf,
    pub source_title: String,
    pub context: Option<String>,
}
```

## Vault Structure

Onyx uses a flat folder structure (no nested directories).

### Layout

```
my-vault/
├── .onyx/
│   └── index.db        # Search index
├── note-one.md
├── note-two.md
├── daily-2024-01-15.md
└── project-ideas.md
```

### Hidden Directory

`.onyx/` contains:
- `index.db` - SQLite search index
- `config.toml` - Vault-specific settings (optional)

### File Naming

- Lowercase with hyphens preferred: `my-note-title.md`
- Spaces allowed but discouraged: `My Note Title.md`
- Special characters should be avoided
- `.md` extension required

### Configuration

`<vault>/.onyx/config.toml`:

```toml
# Vault-specific configuration (optional)

[index]
# Re-index on startup
reindex_on_open = false

[autosave]
enabled = true
interval_seconds = 30

[daily_notes]
enabled = true
template = "templates/daily.md"
folder = ""  # Root of vault (flat structure)
```

## Import/Export

### Import from Obsidian

Obsidian vaults are compatible. Onyx ignores:
- `.obsidian/` directory
- Plugin data
- Nested folders (flattens on import)

```rust
pub fn import_obsidian_vault(source: &Path, dest: &Path) -> Result<(), Error> {
    for entry in walkdir::WalkDir::new(source) {
        let entry = entry?;
        if entry.path().extension() == Some("md".as_ref()) {
            // Flatten path: source/folder/note.md -> dest/folder-note.md
            let flat_name = flatten_path(entry.path(), source);
            let dest_path = dest.join(&flat_name);
            std::fs::copy(entry.path(), dest_path)?;
        }
    }
    Ok(())
}
```

### Export to HTML

```rust
pub fn export_to_html(vault: &Vault, output: &Path) -> Result<(), Error> {
    for note in vault.notes() {
        let html = markdown_to_html(&note.content);
        let html_path = output.join(note.filename()).with_extension("html");
        std::fs::write(html_path, html)?;
    }
    Ok(())
}
```
