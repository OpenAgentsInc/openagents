Native sidebar fixture screenshots with synthetic sessions and account data.

Organize, Sort, and Show open nested menus using the same popup component
as the model selector. Organize and Sort summarize their current choices; the
ungrouped choice is named None. Show has its own section without a count, and
Compact has a separate section with a switch. Radio choices close the child;
Show and Compact toggles stay open for repeated changes.

Validation: native fixture build, changed-file formatting, and diff checks passed.
Native X11 interaction checks covered hover traversal, repeated Show toggles,
arrow-key navigation and sort selection, Compact click/Space toggling, Escape,
and outside-click dismissal.
A narrow-window check confirmed the child switches to the left when needed.

Hover intent is shared with the model selector through `popover::HoverIntent`.
It protects diagonal travel through the trigger-to-child corridor, renews the
300ms grace period while the pointer advances, and cancels pending switches on
exit, clicks, keyboard input, and dismissal. The helper's module documents how
to wire it into future menus.

Regression coverage includes the existing model-selector keyboard and mouse
interaction tests (both popup sides), three shared intent tests, and native
sidebar checks for diagonal travel, renewed grace, child entry, paused sibling
switching, click cancellation, and corridor exit.
