# Hyperview Logout Flow

## Current Implementation

We're trying to implement a logout flow in our Hyperview mobile app that should:
1. Clear the session cookie on the server
2. Navigate back to the login screen
3. Clear any client-side auth state

## What We've Tried

### Attempt 1: Direct Navigation
```xml
<text style="logoutText logoutButton" href="/auth/logout" href-action="push">
  Logout!
</text>
```
Issue: Direct navigation didn't trigger any server requests.

### Attempt 2: Behavior with Replace-Inner
```xml
<view id="redirectContainer" />
<text style="logoutText logoutButton">
  <behavior
    trigger="press"
    action="replace-inner"
    target="redirectContainer"
    href="/hyperview/auth/logout"
  />
  Logout!
</text>
```
Issue: Server response was received but navigation didn't trigger.

### Attempt 3: Full Document Response
```rust
.body(r###"<?xml version="1.0" encoding="UTF-8" ?>
<doc xmlns="https://hyperview.org/hyperview">
  <screen>
    <body>
      <behavior
        trigger="load"
        action="navigate"
        href="/templates/pages/auth/login.xml"
        new-stack="true"
        force-reset="true"
      />
    </body>
  </screen>
</doc>"###.into())
```
Issue: Got XMLRestrictedElementFound error because fragments can't contain `<doc>`.

### Attempt 4: Direct Navigate Action
```xml
<text style="logoutText logoutButton">
  <behavior
    trigger="press"
    action="navigate"
    href="/hyperview/auth/logout"
    new-stack="true"
  />
  Logout!
</text>
```
Issue: No server logs, seems the request isn't being made.

## What We Learned

1. **Fragment vs Document**
   - Fragments (using `replace-inner`) can't contain `<doc>` elements
   - Full documents can only be returned for initial page loads

2. **Navigation Types**
   - `push`: Adds new screen to navigation stack
   - `navigate`: Replaces current screen
   - `replace-inner`: Updates content within a container

3. **Cookie Clearing**
   - We have the server-side cookie clearing working
   - But we never reach it because navigation isn't working

## Current Issues

1. **Navigation Not Triggering**
   - The biggest issue is that our logout button isn't triggering any server requests
   - We see no logs when clicking the button
   - This suggests the Hyperview client isn't handling the click event

2. **Client State**
   - Even if we clear the server cookie, the client maintains its auth state
   - We need to trigger a client-side auth clear as well

3. **Proper Navigation Pattern**
   - We haven't found the right combination of:
     - Button/behavior setup
     - Server response format
     - Navigation action type

## Possible Solutions to Try

1. **Use Standard Link Pattern**
```xml
<text href="/hyperview/auth/logout" href-action="push">Logout</text>
```

2. **Two-Step Navigation**
```xml
<!-- First clear auth -->
<behavior trigger="press" action="custom" handler="clearAuth" />
<!-- Then navigate -->
<behavior trigger="press" action="navigate" href="/auth/logout" />
```

3. **Custom Protocol**
```xml
<behavior trigger="press" action="open-url" href="onyx://logout" />
```

## Questions to Answer

1. Is the click event being captured by Hyperview?
2. Are we using the correct action type for logout navigation?
3. Should we handle auth state clearing on the client first?
4. Are we following Hyperview's navigation patterns correctly?

## Next Steps

1. Add more detailed logging on both client and server
2. Try simpler navigation patterns first
3. Consider implementing a custom handler for logout
4. Look for examples of logout in other Hyperview apps

## References

- [Hyperview Navigation Docs](https://hyperview.org/docs/example_navigation)
- [Delayed Navigation Example](https://hyperview.org/docs/example_delayed_navigation)

## Relevant Files

### Server-Side Files
- `src/server/hyperview/handlers.rs` - Contains logout handlers and other Hyperview endpoints
- `src/server/hyperview/routes.rs` - Defines the Hyperview routes including logout
- `src/server/handlers/auth/session.rs` - Session management and cookie handling
- `src/server/handlers/auth/mod.rs` - Auth module organization
- `src/server/config.rs` - Server configuration

### Templates
- `templates/pages/main.xml` - Main page template with logout button
- `templates/pages/auth/login.xml` - Login page template

### Database
- `migrations/20250126023641_create_users_table.sql` - User table schema
- `migrations/20250110000000_initial.sql` - Initial database setup

### Tests
- `tests/oidc_client.rs` - Auth-related tests

### Documentation
- `docs/hyperview_logout.md` - This document explaining the logout flow
