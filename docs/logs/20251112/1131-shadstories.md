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
