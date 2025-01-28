tests/auth_pages.rs:
│fn test_login_page
│fn test_signup_page

tests/tool_selection.rs:
│fn test_tool_selection

tests/oidc_signup.rs:
│fn create_test_service
│fn create_test_token
│fn test_signup_authorization_url
│fn test_signup_flow
│fn test_duplicate_signup

tests/solver.rs:
│fn test_branch_creation
│fn test_pr_creation
│fn test_issue_comments
│fn test_comment_context_generation

tests/oidc_client.rs:
│fn create_test_token
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

tests/gateway.rs:
│fn setup
│fn test_openrouter_metadata
│fn test_openrouter_chat
│fn test_openrouter_stream
│fn test_openrouter_with_config
│fn test_openrouter_conversation

tests/user.rs:
│fn test_user_creation

tests/mod.rs:

tests/common/mod.rs:
│fn setup_test_db

tests/auth_error_handling.rs:
│fn test_error_component_included
│fn test_error_js_included
│fn test_error_component_accessibility

tests/chat_router_integration.rs:
│fn init_logging
│fn create_test_tools
│fn test_chat_router_integration
│fn test_chat_router_streaming

tests/model_router.rs:
│fn test_routing_decision

tests/signup_flow.rs:
│fn init_test_logging
│fn create_test_service
│fn create_test_token
│fn test_signup_authorization_url
│fn test_signup_flow
│fn test_duplicate_signup
│fn test_signup_error_handling

tests/deepseek.rs:
│fn test_chat_basic
│fn test_chat_with_reasoning
│fn test_chat_with_tools

tests/health_check.rs:
│fn health_check_works

tests/auth_signup_test.rs:
│fn new
│fn authorization_url
│fn new
│fn signup
│fn test_signup_flow
│fn test_signup_url_generation

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

templates/components/auth/error_handler.js:
│function showAuthError
│const errorDiv
│const errorMessage
│function clearAuthError
│const errorDiv
│function handleAuthError
│const errorMessages
│const message

templates/components/auth/auth_scripts.html:

templates/components/auth/error.html:
│#id: auth-error
│#id: auth-error-message

templates/components/features.html:

templates/layouts/chat_base.html:
│<body>
│<head>

templates/layouts/base.html:
│<body>
│<head>

templates/layouts/auth_base.html:
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

templates/pages/login.html:
│#id: auth-error
│#id: auth-error-message
│#id: email
│#id: password
│#id: remember-me

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

templates/pages/signup.html:
│#id: email
│#id: password
│#id: password-confirm
│#id: terms

templates/pages/chat.html:

templates/header.html:

postcss.config.js:

assets/fonts.css:
│@font-face
│@font-face
│@font-face
│@font-face

src/bin/solver.rs:
│fn main

src/bin/deepseek-cli.rs:
│fn print_colored
│fn main

src/bin/repo.rs:
│fn print_colored
│fn main

src/bin/generate-repomap.rs:
│fn get_current_branch
│fn run_git_command
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

src/server/tools.rs:
│fn create_tools

src/server/config.rs:
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
│fn signup
│fn callback
│fn logout

src/server/handlers/user.rs:
│fn create_user

src/server/handlers/mod.rs:

src/server/services/openrouter/types.rs:
│fn from
│impl Default for Default
│impl OpenRouterConfig for OpenRouterConfig
│fn default

src/server/services/openrouter/service.rs:
│fn new
│fn with_config
│fn is_test_mode
│fn get_model
│fn prepare_messages
│fn update_history
│fn make_request
│fn process_stream_chunk
│impl Gateway for Gateway
│impl OpenRouterService for OpenRouterService
│fn metadata
│fn chat
│fn chat_stream

src/server/services/openrouter/mod.rs:

src/server/services/github_issue.rs:
│fn new
│fn get_issue
│fn get_issue_comments
│fn post_comment
│fn create_branch
│fn create_pull_request
│fn post_github_comment

src/server/services/auth.rs:
│fn new
│fn fmt
│fn from
│fn new
│fn authorization_url_for_login
│fn authorization_url_for_signup
│fn login
│fn signup
│fn exchange_code
│fn is_valid_jwt_format
│fn extract_pseudonym

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

src/server/services/gateway/types.rs:

src/server/services/gateway/mod.rs:
│trait Gateway

src/server/services/gateway/streaming.rs:

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
│fn generate_repo_map_with_blacklist
│fn walk_dir
│fn extract_id
│fn extract_function_name
│fn extract_class_name
│fn extract_const_name

src/solver/display.rs:
│fn print_colored
│fn flush_stdout

src/solver/solution.rs:
│fn new
│fn clone_repository
│fn generate_repo_map
│fn cleanup

src/solver/config.rs:
│fn load

src/solver/mod.rs:

src/solver/github.rs:
│fn new
│fn create_branch
│fn create_pull_request
│fn post_comment
│fn get_issue
│fn get_issue_comments

src/solver/planning.rs:
│fn new
│fn generate_plan

src/routes.rs:
│fn health_check
│fn home
│fn login
│fn signup
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

