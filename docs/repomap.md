build.rs:
│fn main

docs/adding-tools.md:

docs/chat_template.md:

docs/configuration.md:

docs/hierarchy.md:

docs/hyperview.md:

docs/hyperview_logout.md:

docs/local-repos.md:

docs/repomap.md:

docs/repomap_generation.md:

docs/solver.md:

docs/templates.md:

docs/test-failures-analysis.md:

docs/transcribe.md:

src/bin/analyze-issue.rs:
│fn main

src/bin/chat.rs:
│fn print_colored
│fn main

src/bin/deepseek-cli.rs:
│fn print_colored
│fn main

src/bin/generate-repomap.rs:
│fn get_current_branch
│fn run_git_command
│fn main

src/bin/repo.rs:
│fn main

src/bin/solver.rs:
│fn new
│fn get_logs
│fn write
│fn flush
│fn make_writer
│fn main
│const OLLAMA_URL

src/bin/solver_impl/changes.rs:
│fn generate_changes
│fn apply_file_changes

src/bin/solver_impl/changes_analysis.rs:
│fn analyze_changes_with_deepseek

src/bin/solver_impl/context.rs:
│fn extract_paths_from_repomap
│fn collect_context

src/bin/solver_impl/files.rs:
│fn identify_files

src/bin/solver_impl/issue.rs:
│fn handle_issue

src/bin/solver_impl/pre_analysis.rs:
│fn analyze_with_deepseek

src/bin/solver_orig.rs:
│fn main

src/bin/transcribe.rs:
│fn main

src/configuration.rs:
│fn default
│fn connect_options
│fn default_admin_token
│fn default_password
│fn default_port
│fn default_true
│fn get_configuration
│fn as_str
│fn try_from

src/database.rs:
│fn get_connection_pool
│fn migrate_database

src/filters.rs:
│fn render_markdown

src/main.rs:
│fn main

src/repo/analysis.rs:
│fn analyze_repository
│fn post_analysis

src/repo/git.rs:
│fn cleanup_temp_dir
│fn clone_repository
│fn commit_changes
│fn push_changes_with_token
│fn checkout_branch

src/repo/test.rs:
│fn run_cargo_tests

src/repo/types.rs:
│fn new

src/repomap.rs:
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
│fn cota

src/server/config.rs:
│fn default
│fn configure_app

src/server/handlers/auth/forms.rs:
│fn deserialize_checkbox
│fn validate

src/server/handlers/auth/github.rs:
│fn github_login_page
│fn handle_github_login
│fn handle_github_callback

src/server/handlers/auth/login.rs:
│fn login_page
│fn handle_login
│fn handle_login_callback

src/server/handlers/auth/mod.rs:
│fn new
│fn handle_auth_error
│fn callback
│const SESSION_COOKIE_NAME
│const SESSION_DURATION_DAYS

src/server/handlers/auth/session.rs:
│fn create_session_and_redirect
│fn clear_session_and_redirect
│fn render_login_template
│fn render_signup_template
│fn clear_session_cookie
│const MOBILE_APP_SCHEME

src/server/handlers/auth/signup.rs:
│fn signup_page
│fn handle_signup
│fn handle_signup_callback

src/server/handlers/user.rs:
│fn create_user

src/server/hyperview/handlers/auth.rs:
│fn mobile_logout

src/server/hyperview/handlers/content.rs:
│#id: content
│fn content

src/server/hyperview/handlers/issue_analysis.rs:
│#id: issue_analysis
│#id: issue_analysis
│#id: issue_analysis
│fn analyze_issue
│fn analyze_issue_internal

src/server/hyperview/handlers/mod.rs:
│fn append_content

src/server/hyperview/handlers/pages.rs:
│#id: container
│#id: title
│#id: button
│#id: buttonText
│#id: loading
│#id: loading-text
│#id: login-button
│fn main_page
│fn login_page
│fn auth_error_response

src/server/hyperview/handlers/repomap.rs:
│#id: repos_list
│#id: container
│#id: error
│fn generate_repomap
│fn error_response

src/server/hyperview/handlers/repos.rs:
│#id: repos-list
│#id: repos-list
│#id: issues_list
│fn error_response
│fn github_repos
│fn github_issues
│fn github_issues_internal

src/server/hyperview/handlers/solver.rs:
│#id: solver-status
│#id: solver_status
│#id: solver_status
│#id: solver_status
│#id: file-changes
│#id: files
│#id: code-diffs
│#id: diffs
│#id: lines
│fn solver_status
│fn solver_status_internal
│fn error_xml
│fn approve_change
│fn reject_change
│fn approve_change_internal
│fn reject_change_internal
│fn solver_files
│fn solver_files_internal
│fn solver_diffs
│fn solver_diffs_internal

src/server/hyperview/handlers/status.rs:
│fn connected_status
│fn disconnected_status

src/server/hyperview/handlers/user.rs:
│#id: user-info
│fn user_info
│fn get_user_from_github_id
│fn auth_error_fragment_response

src/server/hyperview/routes.rs:
│#id: container
│#id: userInfoText
│#id: Description
│#id: Basic
│#id: Bold
│#id: Color
│#id: container
│#id: Basic
│#id: Color
│#id: button
│#id: buttonText
│#id: container
│#id: Basic
│#id: Color
│#id: modalHeader
│#id: modalTitle
│#id: closeButton
│#id: modalBody
│#id: content
│fn hyperview_routes
│fn demo_home
│fn demo_screen2
│fn demo_screen3
│fn solve_demo_modal
│fn screen2_redirect
│fn modal_redirect

src/server/hyperview/services/github_repos.rs:
│fn new
│fn get_user_repos

src/server/hyperview/ws.rs:
│fn hyperview_ws_handler
│fn handle_socket

src/server/models/chat.rs:
│fn new
│fn new

src/server/services/auth.rs:
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
│fn is_valid_jwt_format
│fn extract_pseudonym

src/server/services/chat_database.rs:
│fn new
│fn create_conversation
│fn add_message
│fn get_conversation
│fn get_conversation_messages
│fn list_user_conversations
│fn delete_conversation

src/server/services/deepseek/methods/chat.rs:
│fn chat
│fn chat_internal

src/server/services/deepseek/methods/chat_stream.rs:
│fn chat_stream

src/server/services/deepseek/methods/chat_with_tool_response.rs:
│fn chat_with_tool_response

src/server/services/deepseek/methods/chat_with_tools.rs:
│fn chat_with_tools
│fn chat_with_tools_messages

src/server/services/deepseek/service.rs:
│fn new
│fn with_base_url
│fn create_tool

src/server/services/deepseek/types.rs:
│fn from

src/server/services/gateway/mod.rs:
│fn metadata
│fn chat
│fn chat_stream

src/server/services/github_auth.rs:
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

src/server/services/github_issue/analyzer.rs:
│fn new
│fn analyze_issue
│fn test_analyze_issue

src/server/services/github_issue/conversions.rs:
│fn try_from
│fn try_from
│fn try_from

src/server/services/github_issue/mod.rs:
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

src/server/services/github_repos.rs:
│fn new
│fn get_user_repos

src/server/services/model_router.rs:
│fn new
│fn route_message
│fn execute_tool_call
│fn chat
│fn chat_stream
│fn handle_tool_response

src/server/services/ollama/config.rs:
│fn default
│fn global

src/server/services/ollama/service.rs:
│fn default
│fn new
│fn with_config
│fn chat_structured
│fn metadata
│fn chat
│fn chat_stream

src/server/services/ollama/types.rs:
│fn default

src/server/services/openrouter/service.rs:
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

src/server/services/openrouter/types.rs:
│fn from
│fn default
│const FREE_MODELS

src/server/services/repomap/cache.rs:
│fn new
│fn save
│fn get
│fn delete

src/server/services/repomap/mod.rs:
│fn new
│fn with_pool
│fn generate_repomap
│fn get_repository_map
│fn generate_repository_map
│fn invalidate_cache
│fn cleanup

src/server/services/solver/mod.rs:
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

src/server/services/solver/types.rs:
│fn new
│fn add_file
│fn set_repo_path
│fn add_change

src/server/tools.rs:
│fn create_tools

src/server/ws/handlers/chat.rs:
│fn new
│fn process_message
│fn handle_message
│fn broadcast

src/server/ws/handlers/mod.rs:
│fn handle_message
│fn broadcast

src/server/ws/handlers/solver.rs:
│fn new
│fn handle_solver_event
│fn handle_message
│fn broadcast

src/server/ws/handlers/solver_json.rs:
│fn new
│fn handle_message
│fn emit_state_update
│fn emit_file_analysis
│fn emit_change_generated
│fn emit_change_applied
│fn emit_error
│fn handle_message
│fn broadcast

src/server/ws/mod.rs:
│fn ws_handler
│fn handle_socket

src/server/ws/transport.rs:
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

src/server/ws/types.rs:
│fn fmt

src/solver/changes/apply.rs:
│fn apply_change_to_file
│fn apply_changes
│fn test_apply_change_to_file
│fn test_apply_changes

src/solver/changes/generation.rs:
│fn extract_json_from_markdown
│fn parse_llm_response
│fn validate_changes_relevance
│fn extract_keywords
│fn is_common_word
│fn generate_changes
│fn add
│fn add
│fn test_extract_json_from_markdown
│fn test_validate_changes_relevance
│fn test_extract_keywords
│const MAX_RETRIES

src/solver/changes/parsing.rs:
│fn parse_search_replace

src/solver/changes/tests.rs:
│fn test_generate_changes
│fn test_generate_changes_no_changes
│fn test_parse_search_replace
│fn test_parse_search_replace_multiple
│fn test_parse_search_replace_invalid

src/solver/changes/types.rs:
│fn validate
│fn test_change_block_validation

src/solver/config.rs:
│fn load

src/solver/context.rs:
│fn new
│fn new_with_dir
│fn with_github
│fn create_branch
│fn create_pull_request
│fn generate_file_list
│fn generate_changes
│fn parse_changes
│fn apply_changes
│fn cleanup
│fn test_apply_changes
│fn test
│fn test
│fn test
│fn test_cleanup

src/solver/display.rs:
│fn print_colored
│fn flush_stdout

src/solver/file_list.rs:
│fn extract_json
│fn generate_file_list
│fn test_extract_json
│fn setup_test_repo
│fn main
│fn add
│fn test_generate_file_list
│fn test_invalid_files_filtered
│fn test_empty_repo

src/solver/github.rs:
│fn new
│fn create_branch
│fn generate_pr_title
│fn create_pull_request
│fn post_comment
│fn get_issue
│fn get_issue_comments
│fn test_generate_pr_title
│fn test_new_with_invalid_repo

src/solver/json.rs:
│fn escape_json_string
│fn is_valid_json_string
│fn fix_common_json_issues
│fn test_escape_json_string
│fn test_is_valid_json_string
│fn test_fix_common_json_issues

src/solver/planning.rs:
│fn new
│fn validate_llm_response
│fn generate_prompt
│fn retry_with_feedback
│fn generate_plan
│fn generate_plan_sync
│fn test_validate_llm_response
│fn generate_pr_title
│fn generate_pr_title
│fn test_something
│fn test_something_new
│fn test_generate_prompt
│const MAX_RETRIES

src/solver/solution.rs:
│fn handle_solution

src/solver/state.rs:
│fn new
│fn add_file
│fn update_status
│fn add_change
│fn test_solver_state_creation
│fn test_add_file
│fn test_add_change
│fn test_update_status

src/solver/streaming.rs:
│fn handle_plan_stream

src/solver/types.rs:
│fn new
│fn with_reason
│fn validate
│fn eq
│fn validate_pr_title
│fn test_change_with_reason
│fn test_validate_pr_title
│fn test_change_validation
│fn test_change_error_equality

tailwind.config.cjs:
│const

templates/admin/dashboard.html:
│#id: bg
│#id: solver-progress
│#id: solver-status
│#id: solver-result

templates/admin/login.html:
│#id: bg
│#id: error-message

templates/components/auth/auth_scripts.html:
│const errorDiv
│const errorMessage
│const errorDiv
│const errorMessages
│const message

templates/components/auth/error.html:
│#id: auth-error
│#id: auth-error-message

templates/components/auth/error_handler.js:
│const errorDiv
│const errorMessage
│const errorDiv
│const errorMessages
│const message

templates/components/chat/error_section.html:
│#id: error-section
│#id: error-message

templates/components/chat/head_scripts.html:
│const newTitle

templates/components/chat/login_overlay.html:
│#id: login-overlay

templates/components/chat/main_chat.html:
│#id: chat-messages
│#id: input
│#id: submit-button
│const textarea
│const form
│const newHeight

templates/components/chat/templates.html:
│#id: message-template
│#id: ai-icon-template
│#id: user-icon-template

templates/components/chat/websocket_scripts.html:
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

templates/components/code_diffs.xml:
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

templates/components/file_changes.xml:
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

templates/components/solver_status.xml:
│#id: solver-status-component
│#id: statusContainer
│#id: statusHeader
│#id: statusTitle
│#id: statusBadge
│#id: statusText
│#id: progressBar
│#id: progressFill

templates/layouts/base.html:
│const newTitle
│const currentPath
│const activeLink
│const dot

templates/layouts/chat_content.html:
│#id: chat-messages

templates/layouts/content.html:
│#id: content

templates/macros/ui.html:
│class

templates/pages/auth/callback.xml:
│#id: container
│#id: loading
│#id: error
│#id: loading-text
│#id: error-message
│#id: callback-response

templates/pages/auth/error.xml:
│#id: container
│#id: title
│#id: error
│#id: button
│#id: buttonText

templates/pages/auth/loading.xml:
│#id: container
│#id: title
│#id: message
│#id: spinner

templates/pages/auth/login.xml:
│#id: container
│#id: title
│#id: button
│#id: buttonText
│#id: error
│#id: loading
│#id: loading-text
│#id: login-button
│#id: error-message

templates/pages/company.html:
│class of

templates/pages/login.html:
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

templates/pages/main copy 2.xml:
│#id: screen
│#id: body
│#id: device
│#id: inputBox
│#id: voiceContainer
│#id: askAnything
│#id: sendContainer
│#id: menuItem
│#id: menuText
│#id: menuIconContainer
│#id: whiteText
│#id: conversationText
│#id: menuContainerTop
│#id: menuContainerBottom
│#id: conversationContainer
│#id: logo
│#id: logoContainer
│#id: topContainer
│#id: demoButton
│#id: wsOutput
│#id: wsOutputContent
│#id: message
│#id: messageText
│#id: deepseekOutput
│#id: deepseekText
│#id: deepseekContent
│#id: deepseekChunk
│#id: buttonText
│#id: backButton
│#id: backArrow
│#id: mainContent
│#id: solverContent
│#id: loadingSpinner
│#id: solver-ui
│#id: solver-status
│#id: file-changes
│#id: files
│#id: code-diffs
│#id: diffs
│#id: solve-demo-output
│#id: solverModal
│#id: modalHeader
│#id: modalTitle
│#id: closeButton
│#id: modalBody
│#id: outputScroll
│#id: deepseekOutput
│#id: deepseekText
│#id: deepseekChunk
│#id: deepseek-output
│#id: stream-content

templates/pages/main copy.xml:
│#id: container
│#id: safeArea
│#id: header
│#id: MyHeader
│#id: MyHeaderText
│#id: button
│#id: buttonText
│#id: reposList
│#id: reposScroll
│#id: reposScrollContent
│#id: repoItem
│#id: repoName
│#id: repoDescription
│#id: repoUpdated
│#id: repoActions
│#id: repoButton
│#id: repoButtonText
│#id: error
│#id: welcomeText
│#id: user-info
│#id: redirectContainer
│#id: repos-list

templates/pages/main.xml:
│#id: container
│#id: userInfoText
│#id: Description
│#id: Basic
│#id: Bold
│#id: Color
│#id: buttonContainer
│#id: button
│#id: buttonText
│#id: spinnerStyle
│#id: loadingSpinner
│#id: redirectContainer

templates/pages/repomap.html:
│#id: repo_url
│#id: submit-button
│#id: loading
│#id: repomap-result
│const submitButton
│const response
│const resultDiv

templates/pages/signup.html:
│#id: email
│#id: password
│#id: password-confirm
│#id: terms

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

tests/auth_error_handling.rs:
│fn test_error_component_accessibility
│fn test_error_component_included
│fn test_error_js_included
│const MAX_SIZE

tests/auth_pages.rs:
│fn setup_test_env
│fn test_login_page
│fn test_signup_page

tests/auth_signup_test.rs:
│fn new
│fn authorization_url
│fn new
│fn signup
│fn test_signup_flow
│fn test_signup_url_generation

tests/chat_database.rs:
│fn test_chat_persistence

tests/chat_router_integration.rs:
│fn init_logging
│fn test_chat_router_integration
│fn test_chat_router_streaming

tests/common/mod.rs:
│fn setup_test_db

tests/deepseek.rs:
│fn test_chat_basic
│fn test_chat_with_reasoning
│fn test_chat_with_tools

tests/gateway.rs:
│fn test_openrouter_metadata
│fn test_openrouter_chat
│fn test_openrouter_chat_stream
│fn test_openrouter_with_config
│fn test_openrouter_error_handling

tests/health_check.rs:
│fn health_check_works

tests/model_router.rs:
│fn test_routing_decision

tests/model_router_service/chat.rs:
│fn test_model_router_chat

tests/model_router_service/mod.rs:
│fn init_logging
│fn create_test_tools
│fn create_mock_router

tests/model_router_service/routing.rs:
│fn test_model_router_service

tests/model_router_service/tool_execution.rs:
│fn test_model_router_tool_execution

tests/oidc_client.rs:
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

tests/oidc_signup.rs:
│fn create_test_service
│fn create_test_token
│fn test_signup_authorization_url
│fn test_signup_flow
│fn test_duplicate_signup

tests/ollama.rs:
│fn test_ollama_metadata
│fn test_ollama_chat
│fn test_ollama_chat_stream
│fn test_ollama_with_config
│fn test_ollama_error_handling

tests/signup_flow.rs:
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

tests/solver_changes.rs:
│fn test_change_generation
│fn add
│fn test_change_generation_no_changes

tests/solver_context.rs:
│fn setup_test_context
│fn test_context_initialization
│fn test_apply_changes_new_file
│fn new_function
│fn new_function
│fn test_apply_changes_modify_file
│fn old_function
│fn old_function
│fn new_function
│fn new_function
│fn test_apply_changes_no_match
│fn existing_function
│fn non_existent
│fn new_function
│fn existing_function
│fn test_apply_changes_file_not_found
│fn old
│fn new
│fn test_cleanup

tests/solver_file_list.rs:
│fn setup_test_repo
│fn main
│fn add
│fn test_file_list_generation
│fn main
│fn add
│fn test_file_list_with_invalid_paths
│fn main
│fn add
│fn test_file_list_empty_repo

tests/solver_loop.rs:
│fn test_solver_loop_state_transitions
│fn test_solver_loop_error_handling
│fn test_solver_loop_file_management

tests/solver_ollama.rs:
│fn setup_test_repo
│fn main
│fn add
│fn load_env
│fn test_ollama_file_list
│fn test_ollama_planning
│fn test_ollama_changes
│fn add

tests/solver_state.rs:
│fn test_state_serialization
│fn old_code
│fn new_code
│fn test_state_transitions
│fn test_file_management
│fn test_json_schema_compatibility

tests/solver_types.rs:
│fn test_change_validation
│fn old
│fn new
│fn old
│fn new
│fn new
│fn old
│fn test_change_equality
│fn old
│fn new
│fn old
│fn new
│fn old
│fn new

tests/tool_selection.rs:
│fn test_tool_selection

tests/user.rs:
│fn test_user_creation
│fn create_test_user
