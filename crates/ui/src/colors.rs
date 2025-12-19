//! Semantic color tokens for documentation and reference.
//!
//! Design policy: Components use Tailwind classes with semantic color names.
//! Never use hardcoded colors like `bg-zinc-900` - use `bg-background` instead.
//! The pre-push hook enforces this.
//!
//! # Available Color Tokens
//!
//! ## Base Colors
//! - `background` / `foreground` - Page background and text
//! - `card` / `card-foreground` - Card surfaces
//! - `popover` / `popover-foreground` - Popover surfaces
//!
//! ## Interactive Colors
//! - `primary` / `primary-foreground` - Primary actions (buttons, links)
//! - `secondary` / `secondary-foreground` - Secondary actions
//! - `accent` / `accent-foreground` - Accent highlights
//! - `muted` / `muted-foreground` - Muted/disabled states
//! - `destructive` - Destructive actions (delete, error)
//!
//! ## Utility Colors
//! - `border` - Borders and dividers
//! - `input` - Form input backgrounds
//! - `ring` - Focus rings
//!
//! ## Accent Colors (from platform)
//! - `green` - Success, positive
//! - `red` - Error, destructive
//! - `orange` - Warning
//! - `cyan` - Info
//! - `blue` - Links (use sparingly)
//! - `magenta` - Special highlights
//! - `yellow` - Attention
//!
//! # Usage Examples
//!
//! ```html
//! <!-- Background and text -->
//! <div class="bg-background text-foreground">...</div>
//!
//! <!-- Card -->
//! <div class="bg-card text-card-foreground border border-border">...</div>
//!
//! <!-- Muted text -->
//! <p class="text-muted-foreground">Secondary information</p>
//!
//! <!-- Primary button -->
//! <button class="bg-primary text-primary-foreground">Submit</button>
//!
//! <!-- Accent color -->
//! <span class="text-green">Success!</span>
//! ```

/// Semantic color token names (for reference).
pub const COLOR_TOKENS: &[&str] = &[
    // Base
    "background",
    "foreground",
    "card",
    "card-foreground",
    "popover",
    "popover-foreground",
    // Interactive
    "primary",
    "primary-foreground",
    "secondary",
    "secondary-foreground",
    "accent",
    "accent-foreground",
    "muted",
    "muted-foreground",
    "destructive",
    // Utility
    "border",
    "input",
    "ring",
    // Accents
    "green",
    "red",
    "orange",
    "cyan",
    "blue",
    "magenta",
    "yellow",
];
