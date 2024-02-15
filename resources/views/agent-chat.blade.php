<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <title>{{ config('app.name', 'OpenAgents') }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('analytics')
    <script src="{{ asset('vendor/htmx.min.js') }}"></script> <!-- Include HTMX -->
</head>

<body class="bg-black text-white font-mono h-screen">
    <div class="flex flex-col h-full">
        @include('layouts.navigation')
        <main class="flex-1 overflow-hidden">
            <div class="flex flex-row w-screen h-full">
                <div class="w-1/5 border-r border-offblack shadow-xl nice-scrollbar overflow-y-auto">
                    <!-- Left side content goes here -->
                </div>
                <div class="w-4/5 flex flex-col px-2">
                    <div id="chatbox-container" class="grow nice-scrollbar weird-height">
                        <!-- Chatbox container content will be appended here -->
                    </div>
                    <div>
                        <form id="sendMessageForm"
                              hx-post="/agent/{{$task->id}}/run"
                              hx-headers='{"X-CSRF-TOKEN": "{{ csrf_token() }}", "Accept": "text/html"}'
                              hx-trigger="submit"
                              hx-target="#chatbox-container"
                              hx-swap="beforeend">
                            @csrf
                            <div class="flex flex-row items-center h-auto rounded-xl w-full px-4">
                                <div class="flex-grow ml-4">
                                    <div class="relative w-full">
                                        <x-textarea id="message-input" name="input"
                                                  autofocus
                                                  onkeydown="if(event.keyCode == 13 && !event.shiftKey) { event.preventDefault(); document.getElementById('send-message').click(); }"
                                                  class="text-md w-full rounded-lg border focus:outline-none disabled:opacity-50 dark:text-white focus:ring-0"
                                                  rows="3" placeholder="Type a message..."></x-textarea>
                                    </div>
                                </div>
                                <div class="ml-4">
                                    <button id="send-message" type="submit"
                                            class="flex items-center justify-center bg-teal-500 hover:bg-teal-600 rounded-xl text-white px-4 py-1 flex-shrink-0">
                                        <span>Send</span>
                                        <span class="ml-2">
                                            <svg class="w-4 h-4 transform rotate-45 -mt-px" fill="none" stroke="currentColor"
                                                viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2"
                                                      d="M12 19l9 2-9-18-9 18 9-2zm0 0v-8"></path>
                                            </svg>
                                        </span>
                                    </button>
                                </div>
                            </div>
                        </form>
                    </div>
                </div>
            </div>
        </main>
    </div>
    <script src="{{ asset('vendor/htmx.min.js') }}"></script>
</body>

</html>
