backend/assets/main.css:
│class directly

backend/build.rs:
│fn main

backend/src/bin/analyze-issue.rs:
│fn main

backend/src/bin/chat.rs:
│fn print_colored
│fn main

backend/src/bin/deepseek-cli.rs:
│fn print_colored
│fn main

backend/src/bin/generate-repomap.rs:
│fn get_current_branch
│fn main

backend/src/bin/repo.rs:
│fn main

backend/src/bin/solver_impl/changes.rs:
│fn generate_changes
│fn apply_file_changes

backend/src/bin/solver_impl/changes_analysis.rs:
│fn analyze_changes_with_deepseek

backend/src/bin/solver_impl/context.rs:
│fn extract_paths_from_repomap
│fn collect_context

backend/src/bin/solver_impl/files.rs:
│fn identify_files

backend/src/bin/solver_impl/issue.rs:
│fn handle_issue

backend/src/bin/solver_impl/pre_analysis.rs:
│fn analyze_with_deepseek

backend/src/bin/transcribe.rs:
│fn main

backend/src/configuration.rs:
│fn default
│fn connect_options
│fn default_admin_token
│fn default_password
│fn default_port
│fn default_true
│fn get_configuration
│fn as_str
│fn try_from

backend/src/database.rs:
│fn get_connection_pool
│fn migrate_database

backend/src/filters.rs:
│fn render_markdown

backend/src/main.rs:
│fn main

backend/src/repo/analysis.rs:
│fn analyze_repository
│fn post_analysis

backend/src/repo/git.rs:
│fn cleanup_temp_dir
│fn clone_repository
│fn commit_changes
│fn push_changes_with_token
│fn checkout_branch

backend/src/repo/test.rs:
│fn run_cargo_tests

backend/src/repo/types.rs:
│fn new

backend/src/repomap.rs:
│#id: test
│fn generate_repo_map
│fn 
│fn extract_id
│fn extract_function_name
│fn 
│fn 
│fn extract_class_name
│fn extract_const_name
│fn init_logging
│fn setup_test_repo
│fn main
│fn test_repo_map_generation
│fn main
│fn helper
│fn test_extractors
│fn test_func
│class in
│class 
│class 
│class 
│class 
│class TestClass
│const DEFAULT_BLACKLIST
│const 
│const 
│const 
│const TEST_CONST

backend/src/routes.rs:
│fn health_check
│fn home
│fn login
│fn signup
│fn mobile_app
│fn business
│fn video_series
│fn company
│fn coming_soon
│fn cota
│fn get_user_info

backend/src/server/config.rs:
│fn default
│fn configure_app
│fn configure_app_with_config
│fn log_request
│fn app_router

backend/src/server/handlers/chat.rs:
│fn start_repo_chat
│fn send_message
│fn get_conversation_messages

backend/src/server/handlers/oauth/github.rs:
│fn github_login
│fn github_callback

backend/src/server/handlers/oauth/scramble.rs:
│fn scramble_login
│fn scramble_signup
│fn scramble_callback

backend/src/server/handlers/oauth/session.rs:
│fn create_session_and_redirect
│fn clear_session_and_redirect
│fn create_session_cookie
│fn clear_session_cookie
│const SESSION_COOKIE_NAME
│const SESSION_DURATION_DAYS
│const MOBILE_APP_SCHEME

backend/src/server/handlers/user.rs:
│fn check_email
│fn create_user
│fn get_user

backend/src/server/models/chat.rs:
│fn new
│fn new

backend/src/server/models/timestamp.rs:
│fn to_timestamp
│fn now
│fn into_inner
│fn from
│fn from
│fn to_timestamp
│fn type_info
│fn encode_by_ref
│fn decode
│fn from
│fn to_timestamp

backend/src/server/models/user.rs:
│fn new
│fn scramble_id
│fn github_id
│fn github_token
│fn metadata
│fn created_at
│fn last_login_at
│fn pseudonym
│fn email
│fn build
│fn builder

backend/src/server/services/auth.rs:
│fn new
│fn fmt
│fn from
│fn from
│fn new
│fn authorization_url_for_login
│fn authorization_url_for_signup
│fn login
│fn signup
│fn exchange_code
│fn get_user_by_id
│fn get_user_by_github_id
│fn get_user_by_scramble_id
│fn get_user_by_pseudonym
│fn create_user
│fn update_user_token
│fn update_user_metadata
│fn update_user_last_login
│fn is_valid_jwt_format
│fn extract_pseudonym
│fn get_user_by_id
│fn get_user_by_scramble_id
│fn update_user_by_id
│fn create_user

backend/src/server/services/chat_database.rs:
│fn new
│fn create_conversation
│fn create_message
│fn get_conversation
│fn get_conversation_messages
│fn list_user_conversations
│fn delete_conversation

backend/src/server/services/deepseek/methods/chat.rs:
│fn chat
│fn chat_internal

backend/src/server/services/deepseek/methods/chat_stream.rs:
│fn chat_stream

backend/src/server/services/deepseek/methods/chat_with_tool_response.rs:
│fn chat_with_tool_response

backend/src/server/services/deepseek/methods/chat_with_tools.rs:
│fn chat_with_tools
│fn chat_with_tools_messages

backend/src/server/services/deepseek/service.rs:
│fn new
│fn with_base_url
│fn create_tool

backend/src/server/services/deepseek/types.rs:
│fn from

backend/src/server/services/gateway/mod.rs:
│fn metadata
│fn chat
│fn chat_stream

backend/src/server/services/github_auth.rs:
│fn fmt
│fn from
│fn new
│fn authorization_url
│fn authenticate
│fn exchange_code
│fn get_github_user
│fn get_or_create_user
│fn exchange_code_for_token
│fn process_auth_code

backend/src/server/services/github_issue/analyzer.rs:
│fn new
│fn analyze_issue
│fn test_analyze_issue

backend/src/server/services/github_issue/conversions.rs:
│fn try_from
│fn try_from
│fn try_from

backend/src/server/services/github_issue/mod.rs:
│fn new
│fn get_issue
│fn get_issue_comments
│fn post_comment
│fn check_branch_exists
│fn create_branch
│fn check_branch_has_commits
│fn create_pull_request
│fn list_issues
│fn post_github_comment
│fn test_analyze_issue

backend/src/server/services/github_repos.rs:
│fn new
│fn get_user_repos

backend/src/server/services/groq/config.rs:
│fn default

backend/src/server/services/groq/service.rs:
│fn new
│fn with_base_url
│fn set_model
│fn chat_with_history
│fn chat_with_history_stream
│fn metadata
│fn chat
│fn chat_stream

backend/src/server/services/model_router.rs:
│fn new
│fn route_message
│fn execute_tool_call
│fn chat
│fn chat_stream
│fn handle_tool_response

backend/src/server/services/oauth/github.rs:
│fn new
│fn authorization_url_for_login
│fn authorization_url_for_signup
│fn authenticate
│fn get_github_user
│fn get_or_create_user

backend/src/server/services/oauth/mod.rs:
│fn new
│fn id_token
│fn set_id_token
│fn new
│fn authorization_url
│fn exchange_code

backend/src/server/services/oauth/scramble.rs:
│fn access_token
│fn token_type
│fn expires_in
│fn refresh_token
│fn scopes
│fn from
│fn new
│fn authorization_url_for_login
│fn authorization_url_for_signup
│fn authorization_url
│fn exchange_code
│fn authenticate
│fn handle_signup
│fn handle_login
│fn extract_pseudonym
│fn extract_email
│fn get_user_by_scramble_id
│fn update_user_token
│fn get_or_create_user

backend/src/server/services/oauth/verifier_store.rs:
│fn clone
│fn new
│fn store_verifier
│fn get_verifier
│fn default

backend/src/server/services/ollama/config.rs:
│fn default
│fn global

backend/src/server/services/ollama/service.rs:
│fn default
│fn new
│fn with_config
│fn chat_structured
│fn metadata
│fn chat
│fn chat_stream

backend/src/server/services/ollama/types.rs:
│fn default

backend/src/server/services/openrouter/service.rs:
│fn new
│fn with_config
│fn is_test_mode
│fn get_model
│fn get_next_available_model
│fn mark_model_rate_limited
│fn prepare_messages
│fn make_request
│fn make_structured_request
│fn process_stream_chunk
│fn make_structured_request_with_retry
│fn analyze_issue
│fn analyze_issue_with_schema
│fn metadata
│fn chat
│fn chat_stream
│const REQUEST_TIMEOUT
│const MAX_RETRIES
│const RETRY_DELAY

backend/src/server/services/openrouter/types.rs:
│fn from
│fn default
│const FREE_MODELS

backend/src/server/services/repomap/cache.rs:
│fn new
│fn new
│fn get
│fn set
│fn delete

backend/src/server/services/repomap/mod.rs:
│fn new
│fn generate_repomap
│fn get_map
│fn set_map
│fn delete_map
│fn generate_repository_map
│fn cleanup

backend/src/server/services/solver/mod.rs:
│fn new
│fn create_solver
│fn get_solver
│fn update_solver
│fn start_generating_changes
│fn approve_change
│fn reject_change
│fn check_all_changes_reviewed
│fn analyze_issue
│fn solve_demo_repo
│fn solve_repo

backend/src/server/services/solver/types.rs:
│fn new
│fn add_file
│fn set_repo_path
│fn add_change

backend/src/server/tools.rs:
│fn create_tools

backend/src/server/ws/handlers/chat.rs:
│fn new
│fn process_message
│fn handle_message
│fn broadcast

backend/src/server/ws/handlers/mod.rs:
│fn handle_message
│fn broadcast

backend/src/server/ws/handlers/solver.rs:
│fn new
│fn handle_solver_event
│fn handle_message
│fn broadcast

backend/src/server/ws/handlers/solver_json.rs:
│fn new
│fn handle_message
│fn emit_state_update
│fn emit_file_analysis
│fn emit_change_generated
│fn emit_change_applied
│fn emit_error
│fn handle_message
│fn broadcast

backend/src/server/ws/mod.rs:
│fn ws_handler
│fn handle_socket

backend/src/server/ws/transport.rs:
│fn new
│fn create_handlers
│fn validate_session
│fn handle_socket
│fn broadcast
│fn send_to
│fn get_user_id
│fn add_connection
│fn remove_connection
│fn add_test_connection
│fn get_tx
│fn clone

backend/src/server/ws/types.rs:
│fn fmt

backend/tailwind.config.cjs:
│const 

backend/templates/admin/dashboard.html:
│#id: bg
│#id: solver-progress
│#id: solver-status
│#id: solver-result

backend/templates/admin/login.html:
│#id: bg
│#id: error-message

backend/templates/components/auth/auth_scripts.html:
│const errorDiv
│const errorMessage
│const errorDiv
│const errorMessages
│const message

backend/templates/components/auth/error.html:
│#id: auth-error
│#id: auth-error-message

backend/templates/components/auth/error_handler.js:
│const errorDiv
│const errorMessage
│const errorDiv
│const errorMessages
│const message

backend/templates/components/chat/error_section.html:
│#id: error-section
│#id: error-message

backend/templates/components/chat/head_scripts.html:
│const newTitle

backend/templates/components/chat/login_overlay.html:
│#id: login-overlay

backend/templates/components/chat/main_chat.html:
│#id: chat-messages
│#id: input
│#id: submit-button
│const textarea
│const form
│const newHeight

backend/templates/components/chat/templates.html:
│#id: message-template
│#id: ai-icon-template
│#id: user-icon-template

backend/templates/components/chat/websocket_scripts.html:
│class if
│const button
│const codeBlock
│const originalHTML
│const errorSection
│const form
│const formData
│const content
│const message
│const data
│const button
│const tempDiv
│const codeBlocks
│const pre
│const wrapper
│const langClass
│const lang
│const copyButton
│const messagesDiv
│const template
│const aiIconTemplate
│const userIconTemplate
│const messageEl
│const contentEl
│const statusEl
│const avatarContainer
│const messageEl
│const contentEl
│const statusEl
│const avatarContainer
│const errorSection
│const errorMessage

backend/templates/components/code_diffs.xml:
│#id: code-diffs-component
│#id: diffsContainer
│#id: sectionTitle
│#id: diffList
│#id: diffItem
│#id: diffHeader
│#id: diffPath
│#id: diffActions
│#id: actionButton
│#id: approveButton
│#id: rejectButton
│#id: buttonText
│#id: codeBlock
│#id: codeLine
│#id: addedLine
│#id: removedLine
│#id: emptyState
│#id: emptyText
│#id: diffs
│#id: lines

backend/templates/components/file_changes.xml:
│#id: file-changes-component
│#id: changesContainer
│#id: sectionTitle
│#id: fileList
│#id: fileItem
│#id: fileIcon
│#id: fileName
│#id: fileStatus
│#id: emptyState
│#id: emptyText
│#id: files

backend/templates/components/solver_status.xml:
│#id: solver-status-component
│#id: statusContainer
│#id: statusHeader
│#id: statusTitle
│#id: statusBadge
│#id: statusText
│#id: progressBar
│#id: progressFill

backend/templates/layouts/base.html:
│const newTitle
│const currentPath
│const activeLink
│const dot

backend/templates/layouts/chat_content.html:
│#id: chat-messages

backend/templates/layouts/content.html:
│#id: content

backend/templates/macros/ui.html:
│class 

backend/templates/pages/company.html:
│class of

backend/templates/pages/login.html:
│#id: auth-error
│#id: auth-error-message
│#id: loading-spinner
│#id: login-form
│#id: email
│#id: password
│#id: remember-me
│const errorDiv
│const errorMessage
│const errorDiv
│const spinner
│const spinner
│const errorMessages
│const message
│const formData
│const response
│const location
│const data

backend/templates/pages/repomap.html:
│#id: repo_url
│#id: submit-button
│#id: loading
│#id: repomap-result
│const submitButton
│const response
│const resultDiv

backend/templates/pages/signup.html:
│#id: email
│#id: password
│#id: password-confirm
│#id: terms

backend/templates/pages/solver.html:
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
│const errorSection
│const errorMessage
│const data
│const progress
│const reasoningDiv
│const solutionDiv
│const errorSection
│const errorMessage
│const submitButton
│const form
│const errorSection
│const formData
│const data

backend/tests/auth_pages.rs:
│fn setup_test_db
│fn setup_test_env
│fn test_login_page
│fn test_signup_page

backend/tests/auth_signup_test.rs:
│fn new
│fn authorization_url
│fn new
│fn signup
│fn test_signup_flow
│fn test_signup_url_generation

backend/tests/chat_database.rs:
│fn test_chat_persistence

backend/tests/chat_router_integration.rs:
│fn init_logging
│fn setup_test_db
│fn test_chat_router_integration
│fn test_chat_router_streaming

backend/tests/common/mod.rs:
│fn setup_test_db

backend/tests/deepseek.rs:
│fn test_chat_basic
│fn test_chat_with_reasoning
│fn test_chat_with_tools

backend/tests/gateway.rs:
│fn test_openrouter_metadata
│fn test_openrouter_chat
│fn test_openrouter_chat_stream
│fn test_openrouter_with_config
│fn test_openrouter_error_handling

backend/tests/groq.rs:
│fn test_groq_metadata
│fn test_groq_chat
│fn test_groq_chat_stream
│fn test_groq_error_handling
│fn test_groq_with_base_url

backend/tests/model_router.rs:
│fn test_routing_decision

backend/tests/model_router_service/chat.rs:
│fn test_model_router_chat

backend/tests/model_router_service/mod.rs:
│fn init_logging
│fn create_test_tools
│fn create_mock_router

backend/tests/model_router_service/routing.rs:
│fn test_model_router_service

backend/tests/model_router_service/tool_execution.rs:
│fn test_model_router_tool_execution

backend/tests/oidc_client.rs:
│fn init_logging
│fn new
│fn mock_token_success
│fn mock_token_error
│fn create_test_jwt
│fn setup_test_app
│fn test_full_auth_flow
│fn test_invalid_callback
│fn test_duplicate_login
│const MAX_SIZE

backend/tests/oidc_signup.rs:
│fn create_test_service
│fn create_test_token
│fn test_signup_authorization_url
│fn test_signup_flow
│fn test_duplicate_signup

backend/tests/ollama.rs:
│fn test_ollama_metadata
│fn test_ollama_chat
│fn test_ollama_chat_stream
│fn test_ollama_with_config
│fn test_ollama_error_handling

backend/tests/signup_flow.rs:
│fn create_test_service
│fn create_test_token
│fn test_signup_authorization_url
│fn test_signup_flow
│fn test_duplicate_signup

backend/tests/solver.rs:
│fn test_branch_creation
│fn test_pr_creation
│fn test_issue_comments
│fn test_comment_context_generation

backend/tests/tool_selection.rs:
│fn test_tool_selection

backend/tests/user.rs:
│fn test_user_creation
│fn create_test_user
│fn retry_db_operation
│fn test_user_serialization
│fn test_user_debug
│const MAX_RETRIES
│const RETRY_DELAY

docs/configuration.md:

docs/deep-research/oauth.md:

docs/deep-research/rr.md:

docs/deep-research/syncengine.md:

docs/groq3/oa-syncengine.md:

docs/hierarchy.md:

docs/reasoning.md:

docs/repomap.md:

docs/repomap_generation.md:

docs/scramble.md:

docs/timestamp.md:

frontend/app/+types/home.ts:
│const BLOG_POSTS

frontend/app/+types/onyx.ts:
│const CHANGELOG

frontend/app/+types/video-series.ts:
│const VIDEOS

frontend/app/components/chat/chat-input.tsx:
│const 
│const 
│const 
│const 
│const textareaRef
│const handleSubmitMessage
│const repos
│const handleSubmit
│const handleKeyDown
│const handleAddRepo
│const form
│const owner
│const name
│const branch

frontend/app/components/chat/repo-selector.tsx:
│const RepoForm
│const 
│const 
│const 
│const handleRepoInputChange
│const handleRepoSubmit
│const handleRemoveRepo
│const handleEditClick
│const key

frontend/app/components/chat/thinking.tsx:
│const scrollRef
│const contentRef
│const 
│const shouldScroll
│const getIcon
│const getLabel
│const hasContent

frontend/app/components/header-bar.tsx:
│#id: login-button
│#id: signup-button
│const 
│const navigateTo

frontend/app/components/library/chat.tsx:
│const EXAMPLE_CONTENT
│const 
│const 
│const messagesEndRef
│const scrollToBottom
│const handleSubmit

frontend/app/components/library/shad.tsx:
│#id: email
│#id: airplane-mode
│#id: terms
│#id: option-one
│#id: option-two
│#id: name
│#id: bio
│#id: message
│const 

frontend/app/components/login-form.tsx:
│#id: email
│#id: password
│const 
│const 
│const 
│const 
│const 
│const 
│const checkEmail
│const url
│const response
│const data
│const handleSubmit
│const endpoint
│const response
│const data
│const errorText

frontend/app/components/ui/alert.tsx:
│const alertVariants

frontend/app/components/ui/badge.tsx:
│const badgeVariants
│const Comp

frontend/app/components/ui/breadcrumb.tsx:
│const Comp

frontend/app/components/ui/button.tsx:
│const buttonVariants
│const Comp

frontend/app/components/ui/carousel.tsx:
│const CarouselContext
│const context
│const 
│const 
│const 
│const onSelect
│const scrollPrev
│const scrollNext
│const handleKeyDown
│const 
│const 
│const 
│const 

frontend/app/components/ui/chart.tsx:
│const THEMES
│const ChartContext
│const context
│const ChartContainer
│const uniqueId
│const chartId
│const ChartStyle
│const colorConfig
│const color
│const ChartTooltip
│const ChartTooltipContent
│const 
│const tooltipLabel
│const 
│const key
│const itemConfig
│const value
│const nestLabel
│const key
│const itemConfig
│const indicatorColor
│const ChartLegend
│const ChartLegendContent
│const 
│const key
│const itemConfig
│const payloadPayload

frontend/app/components/ui/form.tsx:
│const Form
│const FormFieldContext
│const FormField
│const useFormField
│const fieldContext
│const itemContext
│const 
│const formState
│const fieldState
│const 
│const FormItemContext
│const id
│const 
│const 
│const 
│const 
│const body

frontend/app/components/ui/input-otp.tsx:
│const inputOTPContext
│const 

frontend/app/components/ui/navigation-menu.tsx:
│const navigationMenuTriggerStyle

frontend/app/components/ui/sidebar.tsx:
│const SIDEBAR_COOKIE_NAME
│const SIDEBAR_COOKIE_MAX_AGE
│const SIDEBAR_WIDTH
│const SIDEBAR_WIDTH_MOBILE
│const SIDEBAR_WIDTH_ICON
│const SIDEBAR_KEYBOARD_SHORTCUT
│const SidebarContext
│const context
│const SidebarProvider
│const isMobile
│const 
│const 
│const open
│const setOpen
│const openState
│const toggleSidebar
│const handleKeyDown
│const state
│const contextValue
│const 
│const 
│const 
│const Comp
│const Comp
│const sidebarMenuButtonVariants
│const Comp
│const 
│const button
│const Comp
│const width
│const Comp

frontend/app/components/ui/slider.tsx:
│const _values

frontend/app/components/ui/sonner.tsx:
│const Toaster
│const 

frontend/app/components/ui/toggle-group.tsx:
│const ToggleGroupContext
│const context

frontend/app/components/ui/toggle.tsx:
│const toggleVariants

frontend/app/hooks/use-mobile.ts:
│const MOBILE_BREAKPOINT
│const 
│const mql
│const onChange

frontend/app/lib/agentsync/hooks/useAgentSync.ts:
│const INITIAL_STATE
│const 
│const 
│const streamingStateRef
│const handleOnline
│const handleOffline
│const processStreamChunk
│const sendMessage
│const userMessageId
│const response
│const errorText
│const reader
│const decoder
│const assistantMessageId
│const 
│const chunk
│const lines
│const line
│const data
│const parsed
│const content
│const reasoning
│const chatId
│const response
│const errorText
│const reader
│const decoder
│const userMessageId
│const 
│const chunk
│const lines
│const line
│const data
│const parsed
│const content
│const reasoning
│const targetId

frontend/app/root.tsx:
│const links

frontend/app/routes/_layout.tsx:
│const navItems
│const location

frontend/app/routes/chat/$id.tsx:
│const EMPTY_MESSAGES
│const 
│const 
│const messageContainerRef
│const messagesSelector
│const messages
│const 
│const timeout
│const response
│const data
│const handleSubmit

frontend/app/routes/chat/index.tsx:
│const navigate
│const 
│const 
│const handleSubmit
│const response

frontend/app/routes/company.tsx:
│class of

frontend/app/routes/components/thinking.tsx:
│const DEMO_TEXT
│const 
│const 
│const 
│const 
│const 
│const allLines
│const timer
│const elapsed
│const startThinking

frontend/app/routes/login-full.tsx:
│const handleGitHubLogin

frontend/app/routes/login.tsx:
│const handleGitHubLogin

frontend/app/routes/repomap.tsx:
│#id: repo_url
│const 
│const 
│const handleSubmit
│const formData
│const repoUrl
│const response
│const data

frontend/app/routes/thinking.tsx:
│const DEMO_TEXT
│const 
│const 
│const 
│const 
│const 
│const allLines
│const timer
│const elapsed
│const startThinking

frontend/app/stores/messages.ts:
│const useMessagesStore
│const 

frontend/app/welcome/logo-dark.svg:
│#id: clip0_202_2131

frontend/app/welcome/logo-light.svg:
│#id: clip0_171_1761

frontend/app/welcome/welcome.tsx:
│const resources

