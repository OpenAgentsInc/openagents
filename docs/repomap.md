tests/tool_selection.rs:
│fn test_tool_selection

tests/oidc_client.rs:
│fn setup_test_db
│fn test_full_auth_flow
│fn test_invalid_callback
│fn test_duplicate_login

tests/model_router_service/mod.rs:
│fn init_logging
│fn create_test_tools
│fn create_mock_router

tests/model_router_service/tool_execution.rs:
│fn test_model_router_tool_execution

tests/model_router_service/routing.rs:
│fn test_model_router_service

tests/model_router_service/chat.rs:
│fn test_model_router_chat

tests/repomap.rs:
│fn test_repomap_endpoint
│fn handle_repomap

tests/user.rs:
│fn test_user_creation

tests/chat_router_integration.rs:
│fn init_logging
│fn create_test_tools
│fn test_chat_router_integration
│fn test_chat_router_streaming

tests/model_router.rs:
│fn test_routing_decision

tests/deepseek.rs:
│fn test_chat_basic
│fn test_chat_with_reasoning
│fn test_chat_with_tools

tests/health_check.rs:
│fn health_check_works

tests/chat_database.rs:
│fn test_chat_persistence

build.rs:
│fn main

styles/tailwind.css:
│@layer base
│pre
│pre code
│code:not(pre code)
│textarea
│textarea:focus
│@layer components
│.prose-custom-dark

templates/blog_post.html:

templates/admin/login.html:
│<body>
│<head>
│#id: bg
│#id: error-message

templates/admin/dashboard.html:
│<body>
│<head>
│#id: bg
│#id: solver-progress
│#id: solver-status
│#id: solver-result

templates/macros/blog.html:

templates/macros/video.html:

templates/macros/blog_post.html:

templates/macros/ui.html:

templates/macros/nav.html:

templates/components/hero.html:

templates/components/chat/error_section.html:
│#id: error-section
│#id: error-message

templates/components/chat/head_scripts.html:

templates/components/chat/login_overlay.html:
│#id: login-overlay

templates/components/chat/sidebar_right.html:

templates/components/chat/sidebar_left.html:

templates/components/chat/templates.html:
│#id: message-template
│#id: ai-icon-template
│#id: user-icon-template

templates/components/chat/main_chat.html:
│#id: chat-messages
│#id: input
│#id: submit-button

templates/components/chat/websocket_scripts.html:

templates/components/chat/header.html:

templates/components/features.html:

templates/layouts/chat_base.html:
│<body>
│<head>

templates/layouts/base.html:
│<body>
│<head>

templates/layouts/chat_content.html:
│#id: chat-messages

templates/layouts/content.html:
│#id: content

templates/pages/onyx.html:

templates/pages/home.html:

templates/pages/repomap.html:
│#id: repo_url
│#id: submit-button
│#id: loading
│#id: repomap-result

templates/pages/video-series.html:

templates/pages/404.html:

templates/pages/solver.html:
│#id: issue_url
│#id: submit-button
│#id: loading
│#id: solver-container
│#id: progress-section
│#id: progress-bar
│#id: solver-status
│#id: files-section
│#id: files-list
│#id: files-reasoning
│#id: solution-section
│#id: solution-reasoning
│#id: solution-code
│#id: error-section
│#id: error-message

templates/pages/coming-soon.html:

templates/pages/company.html:

templates/pages/services.html:

templates/pages/chat.html:

templates/header.html:

postcss.config.js:

src/bin/deepseek-cli.rs:
│fn print_colored
│fn main

src/bin/repo.rs:
│fn print_colored
│fn main

src/bin/chat.rs:
│fn print_colored
│fn main

src/database.rs:
│fn get_connection_pool
│fn migrate_database

src/lib.rs:

src/configuration.rs:
│impl Default for Default
│impl DatabaseSettings for DatabaseSettings
│fn default
│fn connect_options
│fn default_admin_token
│fn default_password
│fn default_port
│fn default_true
│fn get_configuration
│fn as_str
│fn try_from

src/server/config.rs:
│fn create_tools
│fn configure_app

src/server/models/user.rs:

src/server/models/mod.rs:

src/server/models/chat.rs:
│fn new
│fn new

src/server/mod.rs:

src/server/handlers/auth.rs:
│fn new
│fn login
│fn callback
│fn logout
│fn setup_logging
│fn test_auth_flow
│fn test_invalid_callback
│fn test_duplicate_login

src/server/handlers/user.rs:
│fn create_user

src/server/handlers/mod.rs:

src/server/services/github_issue.rs:
│fn new
│fn get_issue
│fn post_comment
│fn post_github_comment

src/server/services/auth.rs:
│fn fmt
│fn from
│fn new
│fn authorization_url
│fn exchange_code
│fn authenticate
│fn extract_pseudonym
│fn test_oidc_config_validation
│fn test_authorization_url_generation
│fn test_token_exchange

src/server/services/repomap.rs:
│fn new
│fn with_base_url
│fn generate_repomap

src/server/services/deepseek/types.rs:
│fn from

src/server/services/deepseek/methods/chat_with_tool_response.rs:
│fn chat_with_tool_response

src/server/services/deepseek/methods/chat_stream.rs:
│fn chat_stream
│fn process_chunk

src/server/services/deepseek/methods/mod.rs:

src/server/services/deepseek/methods/chat_with_tools.rs:
│fn chat_with_tools
│fn chat_with_tools_messages

src/server/services/deepseek/methods/chat.rs:
│fn chat
│fn chat_internal

src/server/services/deepseek/service.rs:
│fn new
│fn with_base_url
│fn create_tool

src/server/services/deepseek/mod.rs:

src/server/services/deepseek/streaming.rs:

src/server/services/mod.rs:

src/server/services/model_router.rs:
│fn new
│fn route_message
│fn execute_tool_call
│fn chat
│fn chat_stream
│fn handle_tool_response

src/server/services/chat_database.rs:
│fn new
│fn create_conversation
│fn add_message
│fn get_conversation
│fn get_conversation_messages
│fn list_user_conversations
│fn delete_conversation

src/server/services/github_types.rs:

src/server/ws/types.rs:
│fn fmt

src/server/ws/transport.rs:
│fn new
│fn create_handlers
│fn validate_session
│fn handle_socket
│fn broadcast
│fn send_to
│fn get_user_id
│fn add_test_connection

src/server/ws/mod.rs:
│fn ws_handler
│fn handle_socket

src/server/ws/handlers/mod.rs:
│trait MessageHandler

src/server/ws/handlers/chat.rs:
│fn new
│fn process_message
│impl MessageHandler for MessageHandler
│impl ChatHandler for ChatHandler
│fn handle_message
│fn broadcast

src/repomap.rs:
│fn generate_repo_map
│fn walk_dir
│fn extract_id
│fn extract_function_name
│fn extract_class_name
│fn extract_const_name

src/routes.rs:
│fn health_check
│fn home
│fn chat
│fn mobile_app
│fn business
│fn video_series
│fn company
│fn coming_soon
│fn repomap
│fn generate_repomap

src/filters.rs:
│fn render_markdown

src/main.rs:
│fn main
│fn create_tools

src/repo/test.rs:
│fn run_cargo_tests

src/repo/types.rs:
│fn new

src/repo/analysis.rs:
│fn analyze_repository
│fn post_analysis

src/repo/git.rs:
│fn cleanup_temp_dir
│fn clone_repository

src/repo/mod.rs:
