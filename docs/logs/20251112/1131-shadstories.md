## 2025-11-12 11:31 — Shadcn UI Stories (batch 1)

- Scope: Add Storybook stories for 5 UI components under `tauri/src/stories`.
- Notes: Follow existing story patterns, keep titles under `UI/*`, and provide basic controls/variants.

Components covered

1) Alert — `UI/Alert`
   - Variants: default, destructive
   - Title + description slots demonstrated

2) Badge — `UI/Badge`
   - Variants: default, secondary, destructive, outline
   - `asChild` is supported but not used in default examples

3) Card — `UI/Card`
   - Demonstrates header/title/description/action/content/footer layout

4) Checkbox — `UI/Checkbox`
   - Controlled example with label
   - `checked`/`disabled` controls

5) Input — `UI/Input`
   - Text/password/file variants
   - Placeholder and width controls

Next batch candidates

- Accordion, Tabs, Switch, Progress, Textarea

## 2025-11-12 11:38 — Shadcn UI Stories (batch 2)

Components covered

6) Accordion — `UI/Accordion`
   - Modes: `single` (collapsible) and `multiple`
   - Three example items with default open state

7) Tabs — `UI/Tabs`
   - Triggers: Account, Password, Billing
   - Content area wrapped with bordered container

8) Switch — `UI/Switch`
   - Controlled example with label
   - `checked`/`disabled` controls

9) Progress — `UI/Progress`
   - Value slider control 0–100
   - Width control container

10) Textarea — `UI/Textarea`
   - Placeholder, rows, width controls
   - `aria-invalid` and `disabled` toggles

Next batch candidates

- HoverCard, Popover, DropdownMenu, RadioGroup, Slider

## 2025-11-12 11:46 — Shadcn UI Stories (batch 3)

Components covered

11) HoverCard — `UI/HoverCard`
    - Controls: side, align, sideOffset
    - Button trigger with descriptive content

12) Popover — `UI/Popover`
    - Controls: side, align, sideOffset, width
    - Example includes input inside content

13) DropdownMenu — `UI/DropdownMenu`
    - Account group, checkboxes, submenu, and radio group
    - Includes keyboard shortcut indicator

14) RadioGroup — `UI/RadioGroup`
    - Controlled example with three options
    - `disabled` toggle

15) Slider — `UI/Slider`
    - Basic (single value) and Range (two thumbs)
    - Orientation, min/max, width/height controls

Next batch candidates

- Popover-like components: ContextMenu, Menubar; plus Select already added; remaining: Pagination, Breadcrumb, Separator

## 2025-11-12 11:54 — Shadcn UI Stories (batch 4)

Components covered

16) ContextMenu — `UI/ContextMenu`
    - Right-click trigger area
    - Items, checkboxes, submenu, and radio group

17) Menubar — `UI/Menubar`
    - File/Edit/View menus
    - Includes submenu, checkbox items, and radio group

18) Pagination — `UI/Pagination`
    - Simple paginator with ellipses logic
    - Controls: page, total, width

19) Breadcrumb — `UI/Breadcrumb`
    - Docs → Components → (ellipsis) → UI → Breadcrumb
    - Toggleable ellipsis

20) Separator — `UI/Separator`
    - Horizontal and vertical demos via orientation control

Next batch candidates

- Remaining: Calendar, Date Picker (if separate), Drawer/Sheet already done?, Command, Carousel, Chart, Resizable, Sidebar, Field/Item/InputGroup

## 2025-11-12 12:02 — Shadcn UI Stories (batch 5)

Components covered

21) Calendar — `UI/Calendar`
    - Single mode and Range mode stories
    - Controls: months, caption layout, nav button variant

22) Drawer — `UI/Drawer`
    - Controlled open state, direction control
    - Header, description, footer actions

23) Sheet — `UI/Sheet`
    - Side control (right/left/top/bottom)
    - Close button + footer actions

24) Command — `UI/Command`
    - CommandDialog with input, groups, items, and shortcuts

25) Carousel — `UI/Carousel`
    - Horizontal/vertical with Prev/Next controls
    - Slide count, width/height controls

Next batch candidates

- Chart, Resizable, Sidebar, Field, Item, InputGroup

## 2025-11-12 12:10 — Shadcn UI Stories (batch 6)

Components covered

26) Chart — `UI/Chart`
    - Area chart with two series, tooltip and legend

27) Resizable — `UI/Resizable`
    - Horizontal/vertical with draggable handle and default sizes

28) Sidebar — `UI/Sidebar`
    - Provider + Sidebar + Inset layout
    - Groups, menu items, trigger, footer action

29) Field — `UI/Field`
    - FieldSet/FieldGroup with inputs, description and error
    - Orientation control (vertical/horizontal/responsive)

30) Item — `UI/Item`
    - List-style items with icon/image media and actions

Next batch candidates

- InputGroup, Kbd, Badge variants done, Breadcrumb done; remaining: NavigationMenu, Menubar done, Tabs done; also: Toggle/ToggleGroup, RadioGroup done, Progress done; others: Popover done, HoverCard done; remaining bigger: Command done, Calendar done, Carousel done, Chart done; still to add: Empty, Pagination done, Table, Collapsible, AlertDialog, Toast (sonner), Spinner, Kbd (if not yet), InputOTP, ButtonGroup

## 2025-11-12 12:18 — Shadcn UI Stories (batch 7)

Components covered

31) InputGroup — `UI/InputGroup`
    - WithButtons (search UI, addons on both sides)
    - WithTextarea (block-start/end addons)

32) Kbd — `UI/Kbd`
    - Single key and grouped key combos

33) Collapsible — `UI/Collapsible`
    - Controlled open state with button trigger

34) Table — `UI/Table`
    - Basic table with caption and footer

35) Empty — `UI/Empty`
    - Icon header, title/description, and actions

Next batch candidates

- Remaining: Toggle, ToggleGroup, RadioGroup done; InputOTP, ButtonGroup, Pagination done; NavigationMenu, Breadcrumb done, Menubar done; Progress done; Spinner, AlertDialog, Sonner (toast), Form

## 2025-11-12 12:26 — Shadcn UI Stories (batch 8)

Components covered

36) AlertDialog — `UI/AlertDialog`
    - Confirm/Cancel with controlled open state

37) Toggle — `UI/Toggle`
    - Variant and size controls, pressed state

38) ToggleGroup — `UI/ToggleGroup`
    - Single and Multiple selection variants

39) InputOTP — `UI/InputOTP`
    - Configurable length with separator and validation state

40) ButtonGroup — `UI/ButtonGroup`
    - Horizontal/vertical groups with separators and text blocks

Next batch candidates

- Spinner, Sonner (toast), Form, NavigationMenu, Pagination done, Breadcrumb done; also: AspectRatio, Label, Progress done, RadioGroup done

## 2025-11-12 12:34 — Shadcn UI Stories (batch 9, final)

Components covered

41) AspectRatio — `UI/AspectRatio`
    - Ratio control with image/placeholder content

42) Label — `UI/Label`
    - With input and checkbox associations

43) NavigationMenu — `UI/NavigationMenu`
    - Two menus (Products, Docs) with viewport

44) ScrollArea — `UI/ScrollArea`
    - Vertical list with border and custom size controls

45) Spinner — `UI/Spinner`
    - Size variants (sm/md/lg)

46) Sonner Toaster — `UI/Sonner`
    - Buttons to trigger success/info/warning/error/promise toasts

47) Form — `UI/Form`
    - React Hook Form + zod validation (name/email)

Status

- All components under `src/components/ui/` now have Storybook coverage with at least one basic story.
- Storybook builds cleanly after each batch.

Fixes

- UI Overview docs (UI.Overview.mdx): fixed runtime error "No CSF file attached" by:
  - Importing blocks from `@storybook/addon-docs/blocks` (local env lacks `@storybook/blocks`).
  - Attaching a CSF via `<Meta of={ButtonStories} />`.
  - Replacing `<Description>` block with plain markdown to avoid context resolution.
