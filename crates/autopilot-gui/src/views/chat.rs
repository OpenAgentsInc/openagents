//! Chat interface view

use maud::{html, Markup};

/// Chat interface with WebSocket streaming
pub fn chat_interface() -> Markup {
    html! {
        div style="display: flex; flex-direction: column; height: calc(100vh - 60px);" {
            // Messages container
            div id="messages" style="flex: 1; overflow-y: auto; padding: 2rem; background: #1a1a1a;" {
                // Messages will be added here via JavaScript
            }

            // Input area
            div style="background: #2a2a2a; border-top: 1px solid #3a3a3a; padding: 1rem;" {
                div style="max-width: 1200px; margin: 0 auto; display: flex; gap: 1rem;" {
                    input
                        type="text"
                        id="prompt-input"
                        placeholder="Type your message..."
                        style="flex: 1; background: #1a1a1a; color: #e0e0e0; border: 1px solid #3a3a3a; padding: 0.75rem; font-family: inherit; font-size: 1rem;"
                        autofocus;

                    button
                        id="send-button"
                        style="background: #4a9eff; color: white; border: none; padding: 0.75rem 1.5rem; cursor: pointer; font-weight: bold;"
                        { "Send" }

                    button
                        id="abort-button"
                        style="background: #c44; color: white; border: none; padding: 0.75rem 1.5rem; cursor: pointer; font-weight: bold; display: none;"
                        { "Abort" }
                }
            }
        }

        // WebSocket client JavaScript
        script {
            r#"
            (function() {
                const messages = document.getElementById('messages');
                const input = document.getElementById('prompt-input');
                const sendBtn = document.getElementById('send-button');
                const abortBtn = document.getElementById('abort-button');

                let ws = null;
                let isProcessing = false;

                function connect() {
                    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                    ws = new WebSocket(protocol + '//' + window.location.host + '/ws');

                    ws.onopen = () => {
                        console.log('WebSocket connected');
                        addSystemMessage('Connected to autopilot');
                    };

                    ws.onclose = () => {
                        console.log('WebSocket disconnected');
                        addSystemMessage('Disconnected - attempting reconnect...');
                        setTimeout(connect, 2000);
                    };

                    ws.onerror = (error) => {
                        console.error('WebSocket error:', error);
                    };

                    ws.onmessage = (event) => {
                        const msg = JSON.parse(event.data);
                        handleServerMessage(msg);
                    };
                }

                function handleServerMessage(msg) {
                    switch (msg.type) {
                        case 'message':
                            addMessage(msg.role, msg.content);
                            isProcessing = false;
                            updateUI();
                            break;
                        case 'status':
                            addSystemMessage('Status: ' + msg.status);
                            isProcessing = false;
                            updateUI();
                            break;
                        case 'error':
                            addSystemMessage('Error: ' + msg.message);
                            isProcessing = false;
                            updateUI();
                            break;
                    }
                }

                function addMessage(role, content) {
                    const bubble = createMessageBubble(role, content);
                    messages.appendChild(bubble);
                    messages.scrollTop = messages.scrollHeight;
                }

                function addSystemMessage(text) {
                    addMessage('system', text);
                }

                function createMessageBubble(role, content) {
                    const colors = {
                        user: { bg: '#2a4a7c', text: '#e0e0e0', align: 'flex-end' },
                        assistant: { bg: '#2a2a2a', text: '#e0e0e0', align: 'flex-start' },
                        system: { bg: '#3a3a1a', text: '#d0d0a0', align: 'center' }
                    };
                    const color = colors[role] || colors.assistant;

                    const wrapper = document.createElement('div');
                    wrapper.style.cssText = `display: flex; justify-content: ${color.align}; margin-bottom: 1rem;`;

                    const bubble = document.createElement('div');
                    bubble.style.cssText = `background: ${color.bg}; color: ${color.text}; padding: 0.75rem 1rem; max-width: 70%; border: 1px solid #3a3a3a;`;

                    const roleLabel = document.createElement('div');
                    roleLabel.style.cssText = 'font-size: 0.75rem; opacity: 0.7; margin-bottom: 0.25rem;';
                    roleLabel.textContent = role;

                    const contentDiv = document.createElement('div');
                    contentDiv.style.cssText = 'white-space: pre-wrap; word-break: break-word;';
                    contentDiv.textContent = content;

                    bubble.appendChild(roleLabel);
                    bubble.appendChild(contentDiv);
                    wrapper.appendChild(bubble);

                    return wrapper;
                }

                function sendPrompt() {
                    const text = input.value.trim();
                    if (!text || !ws || ws.readyState !== WebSocket.OPEN) return;

                    addMessage('user', text);
                    ws.send(JSON.stringify({ type: 'prompt', text }));

                    input.value = '';
                    isProcessing = true;
                    updateUI();
                }

                function abort() {
                    if (ws && ws.readyState === WebSocket.OPEN) {
                        ws.send(JSON.stringify({ type: 'abort' }));
                    }
                    isProcessing = false;
                    updateUI();
                }

                function updateUI() {
                    sendBtn.style.display = isProcessing ? 'none' : 'block';
                    abortBtn.style.display = isProcessing ? 'block' : 'none';
                    input.disabled = isProcessing;
                }

                sendBtn.addEventListener('click', sendPrompt);
                abortBtn.addEventListener('click', abort);

                input.addEventListener('keypress', (e) => {
                    if (e.key === 'Enter' && !e.shiftKey) {
                        e.preventDefault();
                        sendPrompt();
                    }
                });

                // Connect on load
                connect();
            })();
            "#
        }
    }
}
