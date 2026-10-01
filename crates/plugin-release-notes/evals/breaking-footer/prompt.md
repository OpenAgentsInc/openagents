+++
v = "openagents.eval-case.v1"
description = "A pasted full log whose only breaking change is a BREAKING CHANGE footer in a commit body."
kind = "should-fire"
tags = ["pasted", "footer"]

[run]
allowed_operations = ["read", "write"]
+++

Write the release notes for these commits:

```
commit 0a1b2c3d4e5f60718293a4b5c6d7e8f901234567
Author: Ada Example <ada@example.com>
Date:   Wed Sep 30 11:00:00 2026 +0000

    feat(config): read settings from config.toml

    BREAKING CHANGE: settings.ini is no longer read; move it to config.toml.

commit 1b2c3d4e5f60718293a4b5c6d7e8f9012345678a
Author: Lin Example <lin@example.com>
Date:   Tue Sep 29 09:00:00 2026 +0000

    fix: show the right time zone in reminders
```
