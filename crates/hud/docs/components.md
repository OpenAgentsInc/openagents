# Components Reference

Complete reference for all HUD components.

## Frames

### FrameCorners

Bracket-style corner decorations for containers.

```rust
use hud::FrameCorners;

let frame = FrameCorners::new()
    .corner_length(20.0)  // Length of corner brackets
    .line_width(2.0)      // Border thickness
    .color(colors::FRAME_BRIGHT);

frame.animator_mut().enter();
frame.tick();
frame.paint(bounds, &mut scene);
```

**Properties:**
- `corner_length(f32)` - Length of corner bracket segments
- `line_width(f32)` - Border line thickness
- `color(Hsla)` - Border color

---

### FrameLines

Edge lines with configurable gaps and sides.

```rust
use hud::{FrameLines, FrameSides};

let frame = FrameLines::new()
    .sides(FrameSides::horizontal())  // Top and bottom only
    .gap(100.0)                       // Gap in center of each line
    .line_width(1.0)
    .color(colors::FRAME_DIM);
```

**Properties:**
- `sides(FrameSides)` - Which sides to draw (`.all()`, `.horizontal()`, `.vertical()`, or custom)
- `gap(f32)` - Gap width in center of lines
- `line_width(f32)` - Border thickness

---

### FrameOctagon

8-sided frame with clipped corners.

```rust
use hud::FrameOctagon;

let frame = FrameOctagon::new()
    .corner_size(15.0)  // Size of corner clips
    .line_width(1.5)
    .color(colors::FRAME_NORMAL);
```

---

### FrameCircle

Circular border using line segments.

```rust
use hud::FrameCircle;

let frame = FrameCircle::new()
    .segments(48)      // Smoothness of circle
    .line_width(1.5)
    .color(colors::FRAME_NORMAL);
```

---

### FrameHeader

Header section with top line and corner accents.

```rust
use hud::FrameHeader;

let frame = FrameHeader::new()
    .accent_size(12.0)   // Size of corner accents
    .line_width(1.5)
    .show_bottom(true)   // Include bottom border
    .color(colors::FRAME_NORMAL);
```

---

### FrameUnderline

Simple animated bottom line.

```rust
use hud::FrameUnderline;

let frame = FrameUnderline::new()
    .line_width(2.0)
    .color(colors::FRAME_NORMAL);
```

---

## Backgrounds

### DotGridBackground

Animated dot grid pattern.

```rust
use hud::DotGridBackground;

let grid = DotGridBackground::new()
    .spacing(25.0)      // Distance between dots
    .dot_radius(1.0)    // Size of each dot
    .color(colors::DOT_GRID);

grid.animator_mut().enter();
grid.tick();
grid.paint(screen_bounds, &mut scene);
```

---

### GridLinesBackground

Static grid line pattern.

```rust
use hud::GridLinesBackground;

let grid = GridLinesBackground::new()
    .spacing(100.0)     // Grid cell size
    .line_width(1.0)
    .color(Hsla::new(0.0, 0.0, 1.0, 0.03));
```

---

### MovingLinesBackground

Animated moving lines.

```rust
use hud::{MovingLinesBackground, LineDirection};

let lines = MovingLinesBackground::new()
    .spacing(60.0)
    .line_width(1.0)
    .speed(0.5)
    .direction(LineDirection::Down)  // Up, Down, Left, Right
    .color(Hsla::new(0.0, 0.0, 1.0, 0.02));
```

---

## Text Animation

### TextSequence

Character-by-character typewriter reveal.

```rust
use hud::TextSequence;

let text = TextSequence::new("SYSTEM INITIALIZED")
    .font_size(16.0)
    .color(colors::TEXT)
    .show_cursor(true)  // Blinking cursor at end
    .reveal_speed(2);   // Characters per tick

text.animator_mut().enter();
text.tick();
text.paint(position, &mut scene, &mut text_system);
```

**Properties:**
- `font_size(f32)` - Text size
- `color(Hsla)` - Text color
- `show_cursor(bool)` - Show blinking cursor
- `reveal_speed(usize)` - Characters revealed per frame

---

### TextDecipher

Scramble/decipher "hacking" effect.

```rust
use hud::TextDecipher;

let text = TextDecipher::new("STATUS: ONLINE")
    .font_size(12.0)
    .color(colors::TEXT_MUTED)
    .scramble_speed(2);  // Iterations before settling

text.animator_mut().enter();
text.tick();
text.paint(position, &mut scene, &mut text_system);
```

---

## Effects

### Illuminator

Mouse-following radial glow.

```rust
use hud::Illuminator;

let mut illuminator = Illuminator::new()
    .size(200.0)        // Radius of glow
    .color(Hsla::new(0.0, 0.0, 1.0, 0.08))
    .smoothing(0.1);    // Position smoothing factor

// Update position on mouse move
illuminator.set_position(mouse_x, mouse_y);

illuminator.animator_mut().enter();
illuminator.tick();
illuminator.paint(screen_bounds, &mut scene);
```

---

## Interactive

### HudButton

Animated button with frame and hover/press states.

```rust
use hud::HudButton;

let mut button = HudButton::new("CONNECT")
    .font_size(14.0)
    .padding(20.0, 10.0)
    .corner_length(12.0)
    .on_click(|| println!("Clicked!"));

button.animator_mut().enter();

// Update loop
button.tick();

// Handle events
if button.event(&input_event, button_bounds) {
    request_redraw();
}

// Render
button.paint(button_bounds, &mut scene, &mut text_system);
```

**State Methods:**
- `is_hovered()` - Mouse is over button
- `is_pressed()` - Button is being pressed
- `label()` / `set_label(&str)` - Get/set button text

---

## Form Controls

### TextInput

Text field with animated underline.

```rust
use hud::TextInput;

let mut input = TextInput::new()
    .placeholder("Enter command...")
    .value("initial text")
    .font_size(14.0)
    .padding(12.0, 8.0)
    .on_change(|text| println!("Value: {}", text));

input.animator_mut().enter();
input.tick();

// Handle keyboard events
input.event(&input_event, input_bounds);

input.paint(input_bounds, &mut scene, &mut text_system);
```

**Methods:**
- `get_value()` - Current text value
- `set_value(&str)` - Set text programmatically
- `is_focused()` - Input has keyboard focus
- `set_focused(bool)` - Set focus state

---

### Checkbox

Animated checkbox with optional label.

```rust
use hud::Checkbox;

let mut checkbox = Checkbox::new()
    .label("Enable feature")
    .checked(true)
    .font_size(12.0)
    .on_change(|checked| println!("Checked: {}", checked));

checkbox.animator_mut().enter();
checkbox.tick();
checkbox.event(&input_event, checkbox_bounds);
checkbox.paint(checkbox_bounds, &mut scene, &mut text_system);
```

**Methods:**
- `is_checked()` - Current checked state
- `set_checked(bool)` - Set state programmatically

---

### Toggle

On/off switch with sliding knob animation.

```rust
use hud::Toggle;

let mut toggle = Toggle::new()
    .label("Dark mode")
    .enabled(false)
    .font_size(12.0)
    .on_change(|enabled| println!("Enabled: {}", enabled));
```

**Methods:**
- `is_enabled()` - Current enabled state
- `set_enabled(bool)` - Set state programmatically

---

### Select

Dropdown menu with animated expansion.

```rust
use hud::{Select, SelectOption};

let mut select = Select::new()
    .options(vec![
        SelectOption::new("Option A"),
        SelectOption::new("Option B"),
        SelectOption::with_value("Display", "actual_value"),
    ])
    .placeholder("Choose...")
    .selected(0)  // Initial selection
    .on_change(|index, value| println!("Selected: {} = {}", index, value));

select.animator_mut().enter();
select.tick();
select.event(&input_event, select_bounds);
select.paint(select_bounds, &mut scene, &mut text_system);
```

**Methods:**
- `selected_index()` - Current selection index
- `selected_option()` - Current SelectOption reference
- `set_selected(usize)` - Set selection programmatically
- `is_open()` - Dropdown is expanded

---

## Data Display

### List

Animated list with staggered item reveal.

```rust
use hud::{List, ListItem};

let mut list = List::new()
    .items(vec![
        ListItem::new("First item"),
        ListItem::new("Second item").secondary("With detail"),
        ListItem::new("Third item"),
    ])
    .font_size(14.0)
    .item_height(28.0);

list.animator_mut().enter();
list.tick();
list.paint(list_bounds, &mut scene, &mut text_system);
```

---

### Table

Data table with headers and staggered row animation.

```rust
use hud::{Table, TableColumn};

let mut table = Table::new()
    .columns(vec![
        TableColumn::new("Name").width(2.0),
        TableColumn::new("Status").width(1.0),
        TableColumn::new("Value").width(1.0),
    ])
    .rows(vec![
        vec!["Alpha".into(), "Active".into(), "100".into()],
        vec!["Beta".into(), "Idle".into(), "50".into()],
    ])
    .font_size(12.0)
    .row_height(28.0);

table.animator_mut().enter();
table.tick();
table.paint(table_bounds, &mut scene, &mut text_system);
```

---

### CodeBlock

Code display with line numbers and frame.

```rust
use hud::CodeBlock;

let mut code = CodeBlock::new()
    .content("fn main() {\n    println!(\"Hello\");\n}")
    .language("rust")
    .font_size(12.0)
    .show_line_numbers(true);

code.animator_mut().enter();
code.tick();
code.paint(code_bounds, &mut scene, &mut text_system);
```

---

### Card

Content container with optional title.

```rust
use hud::Card;

let mut card = Card::new()
    .title("System Status")
    .padding(15.0)
    .corner_length(15.0)
    .bg_opacity(0.03);

card.animator_mut().enter();
card.tick();
card.paint(card_bounds, &mut scene, &mut text_system);

// Get content area for child components
let content = card.content_bounds(card_bounds);
// Paint children within content...
```
