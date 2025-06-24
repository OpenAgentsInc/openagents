# ProseMirror Documentation

This directory contains comprehensive local documentation for ProseMirror, a toolkit for building rich-text editors on the web. The documentation has been organized to provide easy access to guides, API references, and practical examples.

## Table of Contents

### 📚 Guide Documentation

#### [01-introduction.md](./01-introduction.md)
**Overview of ProseMirror architecture and core concepts**
- Introduction to ProseMirror's modular design
- Four essential modules: model, state, view, and transform
- Basic setup and initialization
- Understanding transactions and plugins
- Creating a simple editor instance

#### [02-document-structure.md](./02-document-structure.md)
**How ProseMirror represents documents differently from the DOM**
- Tree structure with inline content
- Node properties and attributes
- Why ProseMirror doesn't use the DOM structure
- Document immutability and functional updates

#### [03-schemas.md](./03-schemas.md)
**Complete guide to defining document schemas**
- Node types and mark types
- Content expressions and constraints
- Attributes and their specifications
- DOM parsing and serialization rules
- Creating custom schemas from scratch

#### [04-document.md](./04-document.md)
**Working with ProseMirror documents**
- Document indexing and positions
- Retrieving content and nodes
- Working with slices and ranges
- Node methods and properties
- Document transformation patterns

#### [05-transformations.md](./05-transformations.md)
**The transform system for document updates**
- Understanding steps and their types
- Position mapping through changes
- Transform methods and operations
- Rebasing and operational transformation
- Best practices for applying changes

#### [06-state.md](./06-state.md)
**Editor state management and plugins**
- EditorState structure and updates
- Selection types and management
- Transaction creation and application
- Plugin system architecture
- State fields and meta information

#### [07-view.md](./07-view.md)
**The view component and DOM interaction**
- EditorView responsibilities
- Props and their effects
- Decorations for visual enhancements
- Custom node views
- Event handling and DOM coordination

#### [08-commands.md](./08-commands.md)
**Command interface for editor actions**
- Command function signature
- Built-in editing commands
- Command composition and chaining
- Creating custom commands
- Keymap integration

#### [09-collaborative-editing.md](./09-collaborative-editing.md)
**Implementing real-time collaboration**
- The collaborative editing algorithm
- Authority and version management
- Using the collab module
- Conflict resolution
- Advanced features and optimizations

### 📖 API Reference Documentation

#### [ref-model.md](./ref-model.md)
**Core document model (prosemirror-model)**
- Node class and methods
- Mark class and methods
- Schema definition and usage
- Fragment operations
- Slice manipulation
- DOMParser and DOMSerializer

#### [ref-transform.md](./ref-transform.md)
**Document transformations (prosemirror-transform)**
- Transform class and operations
- Step types (ReplaceStep, AddMarkStep, etc.)
- Mapping positions through changes
- StepMap and inversion
- Replace operations and structure changes

#### [ref-state.md](./ref-state.md)
**Editor state management (prosemirror-state)**
- EditorState creation and configuration
- Transaction building
- Selection types (TextSelection, NodeSelection, AllSelection)
- Plugin system and lifecycle
- State fields and facets

#### [ref-view.md](./ref-view.md)
**DOM rendering and interaction (prosemirror-view)**
- EditorView setup and props
- NodeView interface for custom rendering
- Decoration system (inline, widget, node)
- DOM event handling
- Coordinate systems and positioning

#### [ref-commands.md](./ref-commands.md)
**Editing commands (prosemirror-commands)**
- Text manipulation commands
- Mark toggling commands
- Block manipulation commands
- List commands
- Join and split operations
- Command helpers and utilities

#### [ref-keymap.md](./ref-keymap.md)
**Keyboard shortcut handling (prosemirror-keymap)**
- Keymap creation and binding syntax
- Platform-specific key handling
- Key sequence support
- Integration with command system

#### [ref-history.md](./ref-history.md)
**Undo/redo functionality (prosemirror-history)**
- History tracking configuration
- Undo and redo commands
- History event merging
- Custom history behavior

#### [ref-collab.md](./ref-collab.md)
**Collaborative editing support (prosemirror-collab)**
- Collaborative editing protocol
- Version tracking
- Rebasing steps
- Receiving remote changes
- Authority management

#### [ref-inputrules.md](./ref-inputrules.md)
**Auto-formatting as you type (prosemirror-inputrules)**
- InputRule creation
- Common patterns (quotes, ellipsis, arrows)
- Text pattern matching
- Integration with transactions

#### [ref-gapcursor.md](./ref-gapcursor.md)
**Cursor between block nodes (prosemirror-gapcursor)**
- Gap cursor behavior
- Navigation between blocks
- Visual representation
- Integration requirements

#### [ref-schema-basic.md](./ref-schema-basic.md)
**Pre-built basic schema (prosemirror-schema-basic)**
- Standard node types (doc, paragraph, heading, etc.)
- Basic mark types (strong, em, code, link)
- Ready-to-use schema
- Customization patterns

#### [ref-schema-list.md](./ref-schema-list.md)
**List node types and commands (prosemirror-schema-list)**
- List node specifications
- List manipulation commands
- Indentation handling
- List type toggling

#### [ref-dropcursor.md](./ref-dropcursor.md)
**Visual feedback during drag & drop (prosemirror-dropcursor)**
- Drop cursor plugin
- Visual customization
- Integration with drag events

#### [ref-menu.md](./ref-menu.md)
**Menu bar and UI components (prosemirror-menu)**
- Menu item types
- Menu bar setup
- Icons and rendering
- Dropdown menus
- Context-sensitive items

#### [ref-example-setup.md](./ref-example-setup.md)
**Quick setup with sensible defaults (prosemirror-example-setup)**
- Complete editor setup
- Default keybindings
- Basic menu configuration
- Input rules and history

### 💡 Example Implementations

#### [example-basic.md](./example-basic.md)
**Basic editor setup**
- Minimal ProseMirror initialization
- Essential plugins and commands
- Simple schema definition

#### [example-markdown.md](./example-markdown.md)
**Two-way Markdown editing**
- Markdown parsing and serialization
- View switching between rich text and markdown
- Preserving document state during conversion

#### [example-upload.md](./example-upload.md)
**Asynchronous file upload handling**
- Placeholder nodes during upload
- Progress tracking
- Error handling
- Replacing placeholders with uploaded content

#### [example-menu.md](./example-menu.md)
**Custom menu bar implementation**
- Building menu items from scratch
- Dynamic menu state
- Dropdown menus
- Keyboard shortcut display

#### [example-dino.md](./example-dino.md)
**Custom inline nodes**
- Creating special node types
- Custom node views
- Inline node behavior
- Interactive node elements

#### [example-tooltip.md](./example-tooltip.md)
**Selection-based tooltips**
- Tracking selection changes
- Positioning tooltips
- Dynamic content based on selection

#### [example-schema.md](./example-schema.md)
**Building schemas from scratch**
- Node type definitions
- Mark specifications
- Content expressions
- Parsing and serialization rules

#### [example-fold.md](./example-fold.md)
**Code folding with decorations**
- Using node decorations
- Hiding and showing content
- Maintaining document structure
- State persistence

#### [example-codemirror.md](./example-codemirror.md)
**CodeMirror integration**
- Embedding CodeMirror in nodes
- Syntax highlighting
- Two-way synchronization
- Focus management

#### [example-lint.md](./example-lint.md)
**Document linting**
- Problem detection
- Inline problem display
- Fix suggestions
- Lint rule implementation

#### [example-footnote.md](./example-footnote.md)
**Nested editor views**
- Footnote implementation
- Nested ProseMirror instances
- State synchronization
- Complex node interactions

#### [example-track.md](./example-track.md)
**Change tracking**
- Recording document changes
- Commit history
- Change visualization
- Accepting/rejecting changes

#### [example-collab.md](./example-collab.md)
**Collaborative editing setup**
- Basic collaboration architecture
- Authority server implementation
- Client synchronization
- Conflict resolution

### 📋 Additional Resources

#### [changelog.md](./changelog.md)
**Version history and migration guide**
- Recent version changes
- Breaking changes
- New features
- Migration considerations

## Quick Start Guide

For implementing a basic ProseMirror editor with just Enter to submit and Shift+Enter for new lines:

1. Start with [01-introduction.md](./01-introduction.md) to understand the architecture
2. Review [example-basic.md](./example-basic.md) for a minimal setup
3. Check [ref-keymap.md](./ref-keymap.md) for keyboard handling
4. See [08-commands.md](./08-commands.md) for implementing the submit command

## Key Concepts to Understand

1. **Document Model**: Immutable tree structure, not DOM-based
2. **Transactions**: All changes go through transactions
3. **Plugins**: Extend editor behavior in a composable way
4. **Schema**: Defines what content is valid
5. **Commands**: Functions that create transactions
6. **Decorations**: Visual additions without changing the document

## Implementation Order

1. Define your schema
2. Create the editor state
3. Set up the editor view
4. Add keybindings
5. Implement commands
6. Add any additional plugins

This documentation provides everything needed to implement a ProseMirror-based editor, from basic text input to advanced collaborative features.