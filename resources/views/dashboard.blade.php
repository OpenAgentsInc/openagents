<x-layout>
    <div class="relative min-h-screen overflow-hidden bg-black">
        <!-- Animated background -->
        <div class="absolute inset-0 z-0">
            <div class="absolute inset-0 bg-grid opacity-20"></div>
            <div class="absolute inset-0 overflow-hidden">
                <div class="absolute inset-0 neural-network"></div>
            </div>
        </div>

        <!-- Top right buttons -->
        <div class="absolute top-4 right-4 z-20 flex space-x-2">
            <a href="{{ route('components') }}" class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80 h-9 px-4 py-2">
                View Components
            </a>
            <form method="POST" action="{{ route('logout') }}">
                @csrf
                <button type="submit" class="inline-flex items-center justify-center whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90 h-9 px-4 py-2">
                    Logout
                </button>
            </form>
        </div>

        <!-- Main content -->
        <main class="relative z-10 flex flex-col items-center justify-center min-h-screen p-4">
            <div class="flex h-full flex-col items-center justify-center text-zinc-200">
                <div class="h-full w-full lg:py-[18px]">
                    <div class="m-auto text-base px-3 md:px-4 w-full md:px-5 lg:px-4 xl:px-5 h-full">
                        <div class="mx-auto flex h-full w-full flex-col text-base justify-center md:max-w-3xl">
                            <div class="mb-7 text-center block">
                                <div class="select-none pointer-events-none inline-flex justify-center text-2xl font-semibold leading-9">
                                    <h1>How can we help?</h1>
                                </div>
                            </div>
                            <div class="w-full">
                                <form class="w-full" method="POST" action="{{ route('send-message') }}">
                                    @csrf
                                    <input type="hidden" name="project_id" value="{{ auth()->user()->currentProject->id }}">
                                    <div class="flex w-full flex-col gap-1.5 rounded-[30px] p-1 transition-colors bg-zinc-900">
                                        <div class="flex items-end gap-1.5 pl-4 py-0.5 md:gap-2">
                                            <div class="flex min-w-0 flex-1 flex-col">
                                                <textarea
                                                    name="message"
                                                    class="min-h-[46px] max-h-[300px] overflow-y-auto resize-none flex w-full rounded-md bg-transparent px-1 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 border-none"
                                                    placeholder="Message OpenAgents"
                                                    rows="1"
                                                    autofocus
                                                    oninput="this.style.height = 'auto'; this.style.height = this.scrollHeight + 'px'; this.closest('.flex').style.height = 'auto'; this.closest('.flex').style.height = this.scrollHeight + 'px';"
                                                ></textarea>
                                            </div>
                                            <div class="mb-1 me-1">
                                                <button type="submit" aria-label="Send prompt" data-testid="send-button" class="flex h-8 w-8 items-center justify-center rounded-full bg-zinc-700 text-zinc-200 transition-colors hover:bg-zinc-600 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-zinc-500 disabled:bg-zinc-800 disabled:text-zinc-500 disabled:hover:bg-zinc-800" id="send-message" dusk="send-message">
                                                    <svg width="32" height="32" viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg" class="icon-2xl">
                                                        <path fill-rule="evenodd" clip-rule="evenodd" d="M15.1918 8.90615C15.6381 8.45983 16.3618 8.45983 16.8081 8.90615L21.9509 14.049C22.3972 14.4953 22.3972 15.2189 21.9509 15.6652C21.5046 16.1116 20.781 16.1116 20.3347 15.6652L17.1428 12.4734V22.2857C17.1428 22.9169 16.6311 23.4286 15.9999 23.4286C15.3688 23.4286 14.8571 22.9169 14.8571 22.2857V12.4734L11.6652 15.6652C11.2189 16.1116 10.4953 16.1116 10.049 15.6652C9.60265 15.2189 9.60265 14.4953 10.049 14.049L15.1918 8.90615Z" fill="currentColor"></path>
                                                    </svg>
                                                </button>
                                            </div>
                                        </div>
                                    </div>
                                </form>
                            </div>
                            <div class="mt-5 leading-relaxed text-center relative w-full px-2 py-2 text-foreground/30 text-xs empty:hidden md:px-[30px]">
                                <div class="min-h-4"><span class="text-xs select-none leading-none">By messaging OpenAgents, you agree to our <a href="https://openagents.com/terms" target="_blank" class="text-token-text-primary underline decoration-token-text-primary" rel="noreferrer">Terms</a> and <a href="https://openagents.com/privacy" target="_blank" class="text-token-text-primary underline decoration-token-text-primary" rel="noreferrer">Privacy Policy</a>.</span></div>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </main>
    </div>

    <style>
        .bg-grid {
            background-image:
                linear-gradient(to right, rgba(255, 255, 255, 0.2) 1px, transparent 1px),
                linear-gradient(to bottom, rgba(255, 255, 255, 0.2) 1px, transparent 1px);
            background-size: 40px 40px;
        }

        .neural-network {
            background:
                radial-gradient(circle at 50% 50%, rgba(255, 255, 255, 0.1) 0%, transparent 25%),
                radial-gradient(circle at 20% 80%, rgba(255, 255, 255, 0.1) 0%, transparent 35%),
                radial-gradient(circle at 80% 20%, rgba(255, 255, 255, 0.1) 0%, transparent 35%);
            animation: move 30s ease-in-out infinite alternate;
        }

        @keyframes move {
            0% {
                transform: translate(0, 0);
            }

            100% {
                transform: translate(40px, 40px);
            }
        }
    </style>
</x-layout>