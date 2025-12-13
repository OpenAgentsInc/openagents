use dioxus::prelude::*;
use lumen_blocks::components::avatar::{Avatar, AvatarFallback};
use lumen_blocks::components::button::{Button, ButtonVariant};

#[derive(Clone, PartialEq)]
struct Message {
    id: usize,
    role: String,
    content: String,
}

#[component]
pub fn MechaCoder() -> Element {
    let mut messages = use_signal(|| vec![
        Message {
            id: 0,
            role: "assistant".to_string(),
            content: "Hello! I'm MechaCoder, your AI coding assistant. How can I help you today?".to_string(),
        },
    ]);
    let mut input_value = use_signal(|| String::new());
    let mut next_id = use_signal(|| 1usize);

    let mut send_message = move |_| {
        let content = input_value();
        if content.trim().is_empty() {
            return;
        }

        // Add user message
        let user_msg = Message {
            id: next_id(),
            role: "user".to_string(),
            content: content.clone(),
        };
        messages.write().push(user_msg);
        next_id += 1;

        // Add assistant response (placeholder)
        let assistant_msg = Message {
            id: next_id(),
            role: "assistant".to_string(),
            content: format!("I received your message: \"{}\"", content),
        };
        messages.write().push(assistant_msg);
        next_id += 1;

        // Clear input
        input_value.set(String::new());
    };

    rsx! {
        div {
            class: "flex flex-col h-screen bg-background",

            // Header
            header {
                class: "flex items-center justify-between px-6 py-4 border-b border-border",
                div {
                    class: "flex items-center gap-3",
                    Avatar {
                        class: "h-8 w-8",
                        AvatarFallback { "MC" }
                    }
                    h1 {
                        class: "text-xl font-semibold text-foreground",
                        "MechaCoder"
                    }
                }
                span {
                    class: "text-sm text-muted-foreground",
                    "AI Coding Assistant"
                }
            }

            // Messages area
            main {
                class: "flex-1 overflow-y-auto p-6 space-y-4",
                for msg in messages() {
                    MessageBubble {
                        key: "{msg.id}",
                        role: msg.role.clone(),
                        content: msg.content.clone(),
                    }
                }
            }

            // Input area
            footer {
                class: "border-t border-border p-4",
                form {
                    class: "flex gap-3 max-w-4xl mx-auto",
                    onsubmit: move |e| {
                        e.prevent_default();
                        send_message(());
                    },
                    input {
                        class: "flex-1 rounded border border-input bg-background px-3 py-2 text-sm text-foreground placeholder:text-muted-foreground focus:outline-none focus:ring-2 focus:ring-ring",
                        placeholder: "Type your message...",
                        value: input_value(),
                        oninput: move |e| input_value.set(e.value()),
                    }
                    Button {
                        button_type: "submit",
                        variant: ButtonVariant::Primary,
                        "Send"
                    }
                }
            }
        }
    }
}

#[component]
fn MessageBubble(role: String, content: String) -> Element {
    let is_user = role == "user";

    rsx! {
        div {
            class: if is_user { "flex justify-end" } else { "flex justify-start" },
            div {
                class: "flex items-start gap-3 max-w-[80%]",
                class: if is_user { "flex-row-reverse" } else { "" },

                Avatar {
                    class: "h-8 w-8 shrink-0",
                    AvatarFallback {
                        if is_user { "You" } else { "MC" }
                    }
                }

                div {
                    class: "rounded-lg px-4 py-2",
                    class: if is_user {
                        "bg-primary text-primary-foreground"
                    } else {
                        "bg-muted text-foreground"
                    },
                    p { "{content}" }
                }
            }
        }
    }
}
