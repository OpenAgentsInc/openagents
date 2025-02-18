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
│fn get_user_by_scramble_id
│fn update_user_token
│fn get_or_create_user

backend/src/server/services/oauth/verifier_store.rs:
│fn clone
│fn new
│fn store_verifier
│fn get_verifier
│fn cleanup_old_verifiers
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

docs/adding-tools.md:

docs/chat-routing.md:

docs/chat_template.md:

docs/configuration.md:

docs/consolidation.md:

docs/deep-research/oauth.md:

docs/hierarchy.md:

docs/hyperview.md:

docs/hyperview_logout.md:

docs/local-repos.md:

docs/oauth-fixes.md:

docs/repomap.md:

docs/repomap_generation.md:

docs/solver.md:

docs/templates.md:

docs/test-failures-analysis.md:

docs/timestamp.md:

docs/transcribe.md:

frontend/app/+types/home.ts:
│const BLOG_POSTS

frontend/app/+types/onyx.ts:
│const CHANGELOG

frontend/app/+types/video-series.ts:
│const VIDEOS

frontend/app/root.tsx:
│const links

frontend/app/routes/_layout.tsx:
│const navItems

frontend/app/routes/company.tsx:
│class of

frontend/app/routes/repomap.tsx:
│#id: repo_url
│const 
│const 
│const handleSubmit
│const formData
│const repoUrl
│const response
│const data

frontend/app/welcome/logo-dark.svg:
│#id: clip0_202_2131

frontend/app/welcome/logo-light.svg:
│#id: clip0_171_1761

frontend/app/welcome/welcome.tsx:
│const resources

frontend_old/index.html:
│#id: root

frontend_old/public/vite.svg:
│#id: IconifyId1813088fe1fbc01fb466

frontend_old/src/components/HeaderBar.tsx:
│const 
│const navigateTo

frontend_old/src/components/ui/accordion.tsx:
│const Accordion
│const AccordionItem
│const AccordionTrigger
│const AccordionContent

frontend_old/src/components/ui/alert-dialog.tsx:
│const AlertDialog
│const AlertDialogTrigger
│const AlertDialogPortal
│const AlertDialogOverlay
│const AlertDialogContent
│const AlertDialogHeader
│const AlertDialogFooter
│const AlertDialogTitle
│const AlertDialogDescription
│const AlertDialogAction
│const AlertDialogCancel

frontend_old/src/components/ui/alert.tsx:
│const alertVariants
│const Alert
│const AlertTitle
│const AlertDescription

frontend_old/src/components/ui/aspect-ratio.tsx:
│const AspectRatio

frontend_old/src/components/ui/avatar.tsx:
│const Avatar
│const AvatarImage
│const AvatarFallback

frontend_old/src/components/ui/badge.tsx:
│const badgeVariants

frontend_old/src/components/ui/breadcrumb.tsx:
│const Breadcrumb
│const BreadcrumbList
│const BreadcrumbItem
│const BreadcrumbLink
│const Comp
│const BreadcrumbPage
│const BreadcrumbSeparator
│const BreadcrumbEllipsis

frontend_old/src/components/ui/button.tsx:
│const buttonVariants
│const Button
│const Comp

frontend_old/src/components/ui/card.tsx:
│const Card
│const CardHeader
│const CardTitle
│const CardDescription
│const CardContent
│const CardFooter

frontend_old/src/components/ui/carousel.tsx:
│const CarouselContext
│const context
│const Carousel
│const 
│const 
│const 
│const onSelect
│const scrollPrev
│const scrollNext
│const handleKeyDown
│const CarouselContent
│const 
│const CarouselItem
│const 
│const CarouselPrevious
│const 
│const CarouselNext
│const 

frontend_old/src/components/ui/chart.tsx:
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

frontend_old/src/components/ui/chat-message.tsx:
│const chatBubbleVariants
│const ChatMessage
│const files
│const dataArray
│const file
│const isUser
│const formattedTime
│const base64
│const buf

frontend_old/src/components/ui/chat.tsx:
│const lastMessage
│const isEmpty
│const isTyping
│const messageOptions
│const 
│const ChatContainer
│const ChatForm
│const 
│const onSubmit
│const fileList
│const dataTransfer
│const file

frontend_old/src/components/ui/checkbox.tsx:
│const Checkbox

frontend_old/src/components/ui/collapsible.tsx:
│const Collapsible
│const CollapsibleTrigger
│const CollapsibleContent

frontend_old/src/components/ui/command.tsx:
│const Command
│const CommandDialog
│const CommandInput
│const CommandList
│const CommandEmpty
│const CommandGroup
│const CommandSeparator
│const CommandItem
│const CommandShortcut

frontend_old/src/components/ui/context-menu.tsx:
│const ContextMenu
│const ContextMenuTrigger
│const ContextMenuGroup
│const ContextMenuPortal
│const ContextMenuSub
│const ContextMenuRadioGroup
│const ContextMenuSubTrigger
│const ContextMenuSubContent
│const ContextMenuContent
│const ContextMenuItem
│const ContextMenuCheckboxItem
│const ContextMenuRadioItem
│const ContextMenuLabel
│const ContextMenuSeparator
│const ContextMenuShortcut

frontend_old/src/components/ui/copy-button.tsx:
│const 

frontend_old/src/components/ui/dialog.tsx:
│const Dialog
│const DialogTrigger
│const DialogPortal
│const DialogClose
│const DialogOverlay
│const DialogContent
│const DialogHeader
│const DialogFooter
│const DialogTitle
│const DialogDescription

frontend_old/src/components/ui/drawer.tsx:
│const Drawer
│const DrawerTrigger
│const DrawerPortal
│const DrawerClose
│const DrawerOverlay
│const DrawerContent
│const DrawerHeader
│const DrawerFooter
│const DrawerTitle
│const DrawerDescription

frontend_old/src/components/ui/dropdown-menu.tsx:
│const DropdownMenu
│const DropdownMenuTrigger
│const DropdownMenuGroup
│const DropdownMenuPortal
│const DropdownMenuSub
│const DropdownMenuRadioGroup
│const DropdownMenuSubTrigger
│const DropdownMenuSubContent
│const DropdownMenuContent
│const DropdownMenuItem
│const DropdownMenuCheckboxItem
│const DropdownMenuRadioItem
│const DropdownMenuLabel
│const DropdownMenuSeparator
│const DropdownMenuShortcut

frontend_old/src/components/ui/file-preview.tsx:
│const FilePreview
│const ImageFilePreview
│const TextFilePreview
│const 
│const reader
│const text
│const GenericFilePreview

frontend_old/src/components/ui/form.tsx:
│const Form
│const FormFieldContext
│const FormField
│const useFormField
│const fieldContext
│const itemContext
│const 
│const fieldState
│const 
│const FormItemContext
│const FormItem
│const id
│const FormLabel
│const 
│const FormControl
│const 
│const FormDescription
│const 
│const FormMessage
│const 
│const body

frontend_old/src/components/ui/hover-card.tsx:
│const HoverCard
│const HoverCardTrigger
│const HoverCardContent

frontend_old/src/components/ui/input-otp.tsx:
│const InputOTP
│const InputOTPGroup
│const InputOTPSlot
│const inputOTPContext
│const 
│const InputOTPSeparator

frontend_old/src/components/ui/input.tsx:
│const Input

frontend_old/src/components/ui/label.tsx:
│const labelVariants
│const Label

frontend_old/src/components/ui/markdown-renderer.tsx:
│const HighlightedPre
│const 
│const 
│const style
│const CodeBlock
│const code
│const preClass
│const COMPONENTS
│const match
│const Component

frontend_old/src/components/ui/menubar.tsx:
│const MenubarMenu
│const MenubarGroup
│const MenubarPortal
│const MenubarSub
│const MenubarRadioGroup
│const Menubar
│const MenubarTrigger
│const MenubarSubTrigger
│const MenubarSubContent
│const MenubarContent
│const MenubarItem
│const MenubarCheckboxItem
│const MenubarRadioItem
│const MenubarLabel
│const MenubarSeparator
│const MenubarShortcut

frontend_old/src/components/ui/message-input.tsx:
│const 
│const 
│const addFiles
│const onDragOver
│const onDragLeave
│const onDrop
│const dataTransfer
│const onPaste
│const items
│const text
│const blob
│const file
│const files
│const onKeyDown
│const textAreaRef
│const showFileList
│const filtered
│const files
│const input
│const files

frontend_old/src/components/ui/message-list.tsx:
│const additionalOptions

frontend_old/src/components/ui/navigation-menu.tsx:
│const NavigationMenu
│const NavigationMenuList
│const NavigationMenuItem
│const navigationMenuTriggerStyle
│const NavigationMenuTrigger
│const NavigationMenuContent
│const NavigationMenuLink
│const NavigationMenuViewport
│const NavigationMenuIndicator

frontend_old/src/components/ui/pagination.tsx:
│const Pagination
│const PaginationContent
│const PaginationItem
│const PaginationLink
│const PaginationPrevious
│const PaginationNext
│const PaginationEllipsis

frontend_old/src/components/ui/popover.tsx:
│const Popover
│const PopoverTrigger
│const PopoverAnchor
│const PopoverContent

frontend_old/src/components/ui/progress.tsx:
│const Progress

frontend_old/src/components/ui/radio-group.tsx:
│const RadioGroup
│const RadioGroupItem

frontend_old/src/components/ui/resizable.tsx:
│const ResizablePanelGroup
│const ResizablePanel
│const ResizableHandle

frontend_old/src/components/ui/scroll-area.tsx:
│const ScrollArea
│const ScrollBar

frontend_old/src/components/ui/select.tsx:
│const Select
│const SelectGroup
│const SelectValue
│const SelectTrigger
│const SelectScrollUpButton
│const SelectScrollDownButton
│const SelectContent
│const SelectLabel
│const SelectItem
│const SelectSeparator

frontend_old/src/components/ui/separator.tsx:
│const Separator

frontend_old/src/components/ui/sheet.tsx:
│const Sheet
│const SheetTrigger
│const SheetClose
│const SheetPortal
│const SheetOverlay
│const sheetVariants
│const SheetContent
│const SheetHeader
│const SheetFooter
│const SheetTitle
│const SheetDescription

frontend_old/src/components/ui/sidebar.tsx:
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
│const Sidebar
│const 
│const SidebarTrigger
│const 
│const SidebarRail
│const 
│const SidebarInset
│const SidebarInput
│const SidebarHeader
│const SidebarFooter
│const SidebarSeparator
│const SidebarContent
│const SidebarGroup
│const SidebarGroupLabel
│const Comp
│const SidebarGroupAction
│const Comp
│const SidebarGroupContent
│const SidebarMenu
│const SidebarMenuItem
│const sidebarMenuButtonVariants
│const SidebarMenuButton
│const Comp
│const 
│const button
│const SidebarMenuAction
│const Comp
│const SidebarMenuBadge
│const SidebarMenuSkeleton
│const width
│const SidebarMenuSub
│const SidebarMenuSubItem
│const SidebarMenuSubButton
│const Comp

frontend_old/src/components/ui/slider.tsx:
│const Slider

frontend_old/src/components/ui/sonner.tsx:
│const Toaster
│const 

frontend_old/src/components/ui/switch.tsx:
│const Switch

frontend_old/src/components/ui/table.tsx:
│const Table
│const TableHeader
│const TableBody
│const TableFooter
│const TableRow
│const TableHead
│const TableCell
│const TableCaption

frontend_old/src/components/ui/tabs.tsx:
│const Tabs
│const TabsList
│const TabsTrigger
│const TabsContent

frontend_old/src/components/ui/textarea.tsx:
│const Textarea

frontend_old/src/components/ui/toast.tsx:
│const ToastProvider
│const ToastViewport
│const toastVariants
│const Toast
│const ToastAction
│const ToastClose
│const ToastTitle
│const ToastDescription

frontend_old/src/components/ui/toaster.tsx:
│const 

frontend_old/src/components/ui/toggle-group.tsx:
│const ToggleGroupContext
│const ToggleGroup
│const ToggleGroupItem
│const context

frontend_old/src/components/ui/toggle.tsx:
│const toggleVariants
│const Toggle

frontend_old/src/components/ui/tooltip.tsx:
│const TooltipProvider
│const Tooltip
│const TooltipTrigger
│const TooltipContent

frontend_old/src/hooks/use-auto-scroll.ts:
│const ACTIVATION_THRESHOLD
│const containerRef
│const previousScrollTop
│const 
│const scrollToBottom
│const handleScroll
│const 
│const isScrollingUp
│const isScrolledToBottom
│const handleTouchStart

frontend_old/src/hooks/use-autosize-textarea.ts:
│const originalHeight
│const currentRef
│const borderAdjustment
│const scrollHeight
│const clampedToMax
│const clampedToMin

frontend_old/src/hooks/use-copy-to-clipboard.ts:
│const 
│const timeoutRef
│const handleCopy

frontend_old/src/hooks/use-mobile.tsx:
│const MOBILE_BREAKPOINT
│const 
│const mql
│const onChange

frontend_old/src/hooks/use-toast.ts:
│const TOAST_LIMIT
│const TOAST_REMOVE_DELAY
│const actionTypes
│const toastTimeouts
│const addToRemoveQueue
│const timeout
│const reducer
│const 
│const listeners
│const id
│const update
│const dismiss
│const 
│const index

frontend_old/src/pages/ChatScreen.tsx:
│const 

