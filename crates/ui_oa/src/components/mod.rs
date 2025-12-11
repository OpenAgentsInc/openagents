//! Shadcn-style UI components for GPUI
//!
//! Components follow these patterns:
//! - Builder API: `Button::new("label").variant(v).size(s)`
//! - RenderOnce for stateless, Render for stateful
//! - Theme colors from `theme_oa::ui::*`

// Phase 1: Primitives
mod button;
mod label;
mod separator;
mod kbd;
mod skeleton;
mod spinner;
mod progress;
mod checkbox;
mod switch;

// Phase 2: Simple components
mod badge;
mod avatar;
mod alert;
mod toggle;
mod collapsible;
mod aspect_ratio;
mod radio_group;
mod slider;

// Phase 3: Medium components
mod card;
mod tabs;
mod accordion;
mod tooltip;
mod scroll_area;
mod table;
mod breadcrumb;
mod pagination;

// Phase 4: Complex components
mod select;
mod popover;
mod dialog;
mod dropdown_menu;
mod sheet;
mod command;

// Phase 1 exports
pub use button::{Button, ButtonVariant, ButtonSize};
pub use label::Label;
pub use separator::Separator;
pub use kbd::Kbd;
pub use skeleton::Skeleton;
pub use spinner::Spinner;
pub use progress::Progress;
pub use checkbox::Checkbox;
pub use switch::Switch;

// Phase 2 exports
pub use badge::{Badge, BadgeVariant};
pub use avatar::{Avatar, AvatarSize};
pub use alert::{Alert, AlertVariant};
pub use toggle::{Toggle, ToggleVariant, ToggleSize};
pub use collapsible::Collapsible;
pub use aspect_ratio::{AspectRatio, AspectRatioPreset};
pub use radio_group::{Radio, RadioGroup};
pub use slider::Slider;

// Phase 3 exports
pub use card::{Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter};
pub use tabs::{Tabs, TabItem, TabsContent};
pub use accordion::{Accordion, AccordionItem};
pub use tooltip::{Tooltip, TooltipSide};
pub use scroll_area::{ScrollArea, ScrollDirection};
pub use table::{Table, TableHeader, TableBody, TableRow, TableHead, TableCell};
pub use breadcrumb::{Breadcrumb, BreadcrumbItem};
pub use pagination::Pagination;

// Phase 4 exports
pub use select::{Select, SelectOption};
pub use popover::{Popover, PopoverSide};
pub use dialog::{Dialog, DialogHeader, DialogTitle, DialogDescription, DialogContent, DialogFooter};
pub use dropdown_menu::{DropdownMenu, DropdownMenuItem};
pub use sheet::{Sheet, SheetSide, SheetHeader, SheetTitle, SheetDescription, SheetContent, SheetFooter};
pub use command::{Command, CommandItem};
