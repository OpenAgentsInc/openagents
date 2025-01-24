tests/repomap.rs:
│fn test_repomap_endpoint
│fn handle_repomap

tests/deepseek.rs:
│fn test_chat_basic
│fn test_chat_with_reasoning
│fn test_chat_with_tools

tests/health_check.rs:
│fn health_check_works

styles/tailwind.css:

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

assets/main.css:
│::after
│::backdrop
│::after
│::after
│:host
│body
│hr
│abbr:where([title])
│h6
│a
│strong
│pre
│small
│sup
│sub
│sup
│table
│textarea
│select
│input:where([type="submit"])
│:-moz-focusring
│:-moz-ui-invalid
│progress
│::-webkit-outer-spin-button
│[type="search"]
│::-webkit-search-decoration
│::-webkit-file-upload-button
│summary
│pre
│fieldset
│legend
│menu
│dialog
│textarea
│textarea::-moz-placeholder
│textarea::placeholder
│[role="button"]
│:disabled
│object
│video
│[hidden]:where(:not([hidden="until-found"]))
│.container
│@media (min-width: 640px)
│.container
│@media (min-width: 768px)
│.container
│@media (min-width: 1024px)
│.container
│@media (min-width: 1280px)
│.container
│@media (min-width: 1536px)
│.container
│.fixed
│.absolute
│.relative
│.left-1
│.top-1
│.bottom-4
│.right-4
│.mx-2
│.mx-auto
│.my-2
│.my-6
│.mb-2
│.mb-3
│.mb-4
│.mb-6
│.mb-8
│.ml-2
│.ml-6
│.mt-1
│.mt-10
│.mt-2
│.mt-4
│.mt-6
│.block
│.inline
│.flex
│.inline-flex
│.grid
│.hidden
│.h-1\.5
│.h-5
│.h-8
│.h-full
│.h-screen
│.h-\[57px\]
│.h-\[calc\(100vh-57px\)\]
│.h-4
│.h-6
│.min-h-screen
│.w-1\.5
│.w-5
│.w-8
│.w-\[48rem\]
│.w-\[600px\]
│.w-\[60rem\]
│.w-screen
│.w-64
│.w-full
│.w-48
│.w-\[80px\]
│.w-4
│.w-6
│.max-w-2xl
│.max-w-4xl
│.max-w-7xl
│.max-w-full
│.max-w-none
│.max-w-xl
│.flex-1
│.flex-shrink-0
│.flex-grow
│@keyframes spin
│to
│.animate-spin
│.touch-manipulation
│.select-none
│.list-disc
│.grid-cols-1
│.grid-cols-3
│.flex-col
│.items-center
│.justify-center
│.gap-2
│.gap-4
│.gap-8
│.gap-x-6
│.gap-x-8
│.gap-y-16
│.space-y-1 > :not([hidden]) ~ :not([hidden])
│.space-y-2 > :not([hidden]) ~ :not([hidden])
│.space-y-3 > :not([hidden]) ~ :not([hidden])
│.space-y-4 > :not([hidden]) ~ :not([hidden])
│.space-y-6 > :not([hidden]) ~ :not([hidden])
│.space-y-8 > :not([hidden]) ~ :not([hidden])
│.overflow-hidden
│.overflow-x-auto
│.overflow-y-auto
│.overflow-x-hidden
│.whitespace-nowrap
│.whitespace-pre-wrap
│.break-words
│.break-all
│.rounded
│.rounded-full
│.rounded-md
│.rounded-xl
│.border
│.border-b
│.border-b-2
│.border-l-4
│.border-r
│.border-t
│.border-l
│.border-red-500\/20
│.border-white
│.border-white\/10
│.border-white\/50
│.border-white\/90
│.border-red-500
│.bg-black
│.bg-black\/30
│.bg-black\/50
│.bg-gray-800
│.bg-indigo-600
│.bg-red-900\/20
│.bg-white
│.p-2
│.p-4
│.p-6
│.px-3
│.px-3\.5
│.px-4
│.px-6
│.py-1
│.py-2
│.py-2\.5
│.py-24
│.py-3
│.py-4
│.py-6
│.pb-4
│.pl-4
│.pl-9
│.text-center
│.align-middle
│.font-mono
│.text-3xl
│.text-base
│.text-lg
│.text-sm
│.text-xs
│.text-xl
│.font-bold
│.font-medium
│.font-semibold
│.italic
│.leading-6
│.leading-7
│.leading-8
│.tracking-tight
│.text-gray-300
│.text-gray-400
│.text-gray-600
│.text-gray-900
│.text-green-400
│.text-indigo-600
│.text-red-300
│.text-red-400
│.text-white
│.text-white\/50
│.text-white\/70
│.text-white\/80
│.text-yellow-400
│.text-red-500
│.text-gray-500
│.underline
│.no-underline
│.placeholder-white\/50::-moz-placeholder
│.placeholder-white\/50::placeholder
│.opacity-75
│.shadow-nav
│.shadow-sm
│.shadow-xl
│.outline-none
│.ring-1
│.ring-gray-400\/10
│.transition
│.transition-all
│.transition-transform
│.transition-colors
│.duration-300
│.duration-nav
│.ease-in-out
│.ease-nav
│.hover\:bg-indigo-500:hover
│.hover\:bg-white:hover
│.hover\:bg-zinc-900:hover
│.hover\:text-black:hover
│.hover\:text-white:hover
│.hover\:text-white\/80:hover
│.hover\:text-gray-300:hover
│.hover\:shadow-nav-hover:hover
│.focus\:border-white:focus
│.focus\:border-gray-400:focus
│.focus\:outline-none:focus
│.focus\:ring-1:focus
│.focus\:ring-2:focus
│.focus\:ring-white:focus
│.focus\:ring-offset-2:focus
│.focus-visible\:outline:focus-visible
│.focus-visible\:outline-2:focus-visible
│.focus-visible\:outline-offset-2:focus-visible
│.focus-visible\:outline-indigo-600:focus-visible
│.active\:shadow-nav-active:active
│.disabled\:cursor-not-allowed:disabled
│.disabled\:opacity-50:disabled
│@media (min-width: 640px)
│.sm\:w-\[57rem\]
│.sm\:gap-y-20
│.sm\:px-6
│.sm\:py-32
│.sm\:text-4xl
│@media (min-width: 768px)
│.md\:mx-6
│.md\:-ml-4
│.md\:grid-cols-2
│.md\:px-4
│.md\:py-1
│.md\:py-2
│.md\:py-3
│@media (min-width: 1024px)
│.lg\:mx-0
│.lg\:-ml-0
│.lg\:max-w-lg
│.lg\:max-w-none
│.lg\:grid-cols-2
│.lg\:grid-cols-6
│.lg\:px-8
│.lg\:pr-8
│.lg\:pt-4

assets/fonts.css:
│@font-face
│@font-face
│@font-face
│@font-face

src/bin/deepseek-cli.rs:
│fn print_colored
│fn main

src/bin/repo.rs:
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
│fn configure_app

src/server/mod.rs:

src/server/services/github_issue.rs:
│fn new
│fn get_issue
│fn post_comment
│fn post_github_comment

src/server/services/repomap.rs:
│fn new
│fn with_base_url
│fn generate_repomap

src/server/services/mod.rs:

src/server/services/deepseek.rs:
│fn from
│fn new
│fn with_base_url
│fn create_tool
│fn chat
│fn chat_with_tools
│fn chat_with_tool_response
│fn chat_stream
│fn chat_internal

src/server/services/github_types.rs:

src/server/ws/types.rs:

src/server/ws/transport.rs:
│fn new
│fn create_handlers
│fn handle_socket
│fn broadcast
│fn send_to

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

src/filters.rs:
│fn markdown
│fn safe

src/main.rs:
│fn main
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
