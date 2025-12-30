# ToolCallCard elapsed time support

ToolCallCard can display elapsed time for running tools.

## API additions
- ToolCallCard::elapsed_secs(f64) sets elapsed time on construction.
- ToolCallCard::set_elapsed_secs(f64) updates elapsed time after creation.
- ChildTool now includes elapsed_secs for nested Task tool progress.

## Rendering behavior
- Running tool cards display the elapsed time (for example: 3.2s) in the status column.
- Child tools show elapsed time when available; otherwise they retain the placeholder running indicator.
