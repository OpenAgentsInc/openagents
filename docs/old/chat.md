# Chat System Design

## Overview

This document outlines the design and implementation of the chat system for OpenAgents, focusing on the flow where a user sends a message from the home dashboard, gets redirected to a thread, and receives a streaming response.

## Current Implementation

Based on the existing code and tests, we have the following structure:

1. Users can send messages from the homepage.
2. A new thread is created for each new conversation.
3. Messages are associated with threads.
4. The system redirects users to the chat page after sending a message.

## Desired Flow

1. User submits a message on the home dashboard.
2. System creates a new thread (if not existing) and associates the message with it.
3. User is redirected to the chat page for the specific thread.
4. The system starts streaming the response in real-time.

## Implementation Details

### 1. Message Submission

- Use HTMX for handling form submissions without full page reloads.
- Update the `sendMessage` method in `MessageController` to return an HTMX-compatible response.

### 2. Thread Creation and Redirection

- In `MessageController::sendMessage`:
  - Create a new thread if not provided.
  - Associate the message with the thread.
  - Return an HTMX response that triggers a redirect to the chat page.

### 3. Streaming Response

- Implement Server-Sent Events (SSE) for real-time streaming.
- Use the HTMX SSE extension to handle the client-side streaming.

#### Server-side Implementation

1. Create a new route for SSE connections (e.g., `/sse/{thread_id}`).
2. Implement an SSE controller that:
   - Establishes the SSE connection.
   - Streams the AI-generated response.
   - Sends events for each chunk of the response.

#### Client-side Implementation

1. Include the HTMX SSE extension:
   ```html
   <script src="https://unpkg.com/htmx-ext-sse@2.2.2/sse.js"></script>
   ```

2. Set up the SSE connection in the chat view:
   ```html
   <div hx-ext="sse" sse-connect="/sse/{thread_id}" sse-swap="message">
     <!-- Chat messages will be updated here -->
   </div>
   ```

3. Handle the streaming updates:
   ```html
   <div id="chat-messages" hx-ext="sse" sse-connect="/sse/{thread_id}">
     <div sse-swap="message">
       <!-- Individual message will be swapped here -->
     </div>
   </div>
   ```

## Considerations

1. **Performance**: Ensure that the SSE implementation can handle multiple concurrent connections efficiently.

2. **Error Handling**: Implement robust error handling for SSE connections, including automatic reconnection.

3. **Security**: Validate user permissions for accessing specific threads and receiving SSE updates.

4. **Scalability**: Consider using a message queue system for handling AI response generation to ensure the system can scale with increased load.

5. **User Experience**: Provide visual feedback during the streaming process, such as a typing indicator or progressive loading of the message.

6. **Testing**: Update existing tests and add new ones to cover the SSE functionality and streaming behavior.

7. **Fallback**: Implement a fallback mechanism for browsers that don't support SSE or in case of connection issues.

## Next Steps

1. Update the `MessageController` to handle HTMX requests and SSE setup.
2. Create a new SSE controller for managing streaming connections.
3. Modify the chat view to incorporate HTMX and SSE attributes.
4. Implement the AI response generation and streaming logic.
5. Update and expand the test suite to cover new functionality.
6. Perform thorough testing and optimize for performance.

By implementing these changes, we'll create a seamless chat experience where users can send messages from the home dashboard, get redirected to the appropriate chat thread, and receive real-time streaming responses using HTMX and Server-Sent Events.