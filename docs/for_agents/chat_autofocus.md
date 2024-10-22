# Chat Autofocus Feature

This document explains how the chat autofocus feature works in the OpenAgents application. The feature automatically focuses the message input field when a user loads a new chat by clicking on a chat link in the sidebar or on initial page load.

## Components Involved

1. `resources/views/components/sidebar/section-threads.blade.php`
2. `resources/views/components/dashboard/dashboard.blade.php`
3. `resources/views/components/dashboard/message-form.blade.php`
4. `public/js/dashboard.js`

## How It Works

### 1. Triggering the Event

In the `section-threads.blade.php` file, we've added a custom event dispatch to each chat link. This event is triggered after the HTMX request to load the chat content is completed.

```html
<a href="{{ route('chat.show', $thread) }}"
   ...
   hx-get="{{ route('chat.show', $thread) }}"
   hx-target="#main-content"
   hx-push-url="true"
   hx-on::after-request="console.log('Chat loaded event dispatched'); document.body.dispatchEvent(new CustomEvent('chatLoaded')); console.log('Event dispatched, listeners:', document.body._events ? Object.keys(document.body._events).length : 'No listeners');"
   ...>
   <!-- Link content -->
</a>
```

The `hx-on::after-request` attribute is an HTMX extension that allows us to execute JavaScript after the HTMX request is completed. In this case, we're dispatching a custom event named 'chatLoaded' on the document.body.

### 2. Listening for the Event and Focusing the Textarea

We've moved the event listening and focusing logic to a separate JavaScript file, `public/js/dashboard.js`. This file handles both the initial load and subsequent chat loads:

```javascript
function focusTextarea() {
    console.log('Attempting to focus textarea');
    const textarea = document.getElementById('message-textarea');
    if (textarea) {
        console.log('Textarea found, focusing');
        setTimeout(() => {
            textarea.focus();
        }, 100);
    } else {
        console.log('Textarea not found');
    }
}

function setupChatLoadedListener() {
    console.log('Setting up chatLoaded listener');
    document.body.addEventListener('chatLoaded', function() {
        console.log('chatLoaded event received');
        focusTextarea();
    });
}

document.addEventListener('DOMContentLoaded', function() {
    console.log('DOM fully loaded');
    setupChatLoadedListener();
    focusTextarea();

    // MutationObserver setup
    const observer = new MutationObserver(function(mutations) {
        mutations.forEach(function(mutation) {
            if (mutation.type === 'childList') {
                const addedNodes = mutation.addedNodes;
                for (let i = 0; i < addedNodes.length; i++) {
                    if (addedNodes[i].id === 'message-textarea') {
                        console.log('Textarea added to DOM');
                        focusTextarea();
                        observer.disconnect(); // Stop observing once we've found the textarea
                        break;
                    }
                }
            }
        });
    });

    // Start observing the document with the configured parameters
    observer.observe(document.body, { childList: true, subtree: true });
});
```

This script does the following:

1. Defines a `focusTextarea` function that attempts to focus the textarea.
2. Sets up a listener for the 'chatLoaded' event.
3. Attempts to focus the textarea on initial page load.
4. Uses a MutationObserver to detect when the textarea is added to the DOM, which helps with the initial load case.

### 3. The Message Form

The `message-form.blade.php` component contains the textarea that we want to focus. It has an id of 'message-textarea':

```html
<textarea
    name="content"
    id="message-textarea"
    ...
>
</textarea>
```

This id is crucial as it's used in the script to find and focus the textarea.

## Flow of Execution

1. On initial page load:
   - The DOMContentLoaded event fires, triggering the setup of event listeners and the initial focus attempt.
   - If the textarea isn't immediately available, the MutationObserver will detect when it's added to the DOM and focus it.

2. When a user clicks on a chat link in the sidebar:
   - HTMX loads the new chat content into the main content area.
   - After the content is loaded, the 'chatLoaded' event is dispatched.
   - The event listener in dashboard.js catches this event and focuses the textarea.

## Benefits

- Improves user experience by automatically placing the cursor in the message input field.
- Allows users to start typing immediately after selecting a chat or on initial page load.
- Works seamlessly with the HTMX-powered dynamic content loading.
- Handles both initial page load and subsequent chat loads.

## Considerations

- Make sure the id 'message-textarea' is unique and consistently used across the application.
- The MutationObserver is used to handle cases where the textarea might not be immediately available in the DOM.
- A small delay (100ms) is added before focusing to ensure the textarea is fully rendered.

## Potential Enhancements

- Add a debounce function to prevent multiple rapid focus events.
- Implement smooth scrolling to ensure the textarea is visible when focused.
- Add keyboard shortcuts for navigating between chats while maintaining focus on the textarea.

By implementing this feature, we've enhanced the user experience in the OpenAgents application, making chat interactions more fluid and intuitive, both on initial load and when switching between chats.