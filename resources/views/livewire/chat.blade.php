<div class="flex h-screen w-full overflow-hidden bg-gray-900">
    <div class="fixed top-0 left-0 h-screen w-[300px] bg-offblack z-10">
        <div class="flex flex-col h-full mt-4">
            <a wire:navigate href="{{ route('chat') }}">
                <x-button variant="ghost" size="lg" icon="create">
                    New session
                </x-button>
            </a>
            <div class="flex flex-col flex-grow overflow-y-auto px-3 pb-3.5">
                <div class="mt-2 text-md">
                    <p class="text-gray px-3 tracking-wider">Recent</p>
                    <ul class="mt-2 cursor-pointer">
                        @foreach($conversations as $conversation)
                            <a wire:navigate
                                href="{{ route('chat.show', $conversation['id']) }}">
                                <li class="text-white px-3 py-1 hover:bg-darkgray rounded-[6px]">
                                    {{ $conversation['title'] ?? 'New chat' }}
                                </li>
                            </a>
                        @endforeach
                    </ul>
                </div>
                <div class="mt-auto">
                    <div class="flex items-center justify-between px-3 py-2 rounded-lg">
                        <div class="flex items-center">
                            <div class="h-8 w-8 bg-gray rounded-full mr-2"></div>
                            <span class="text-white text-sm">Chris</span>
                        </div>
                        <button class="text-white text-xs">
                            <svg class="h-4 w-4 fill-white" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
                                <path d="M10 12a2 2 0 110-4 2 2 0 010 4z" />
                            </svg>
                        </button>
                    </div>
                </div>
            </div>
        </div>
    </div>

    <div class="pl-[300px] w-full h-screen flex flex-col">
        <div class="fixed top-0 left-[300px] right-0 h-[60px] z-10">
            <div class="text-white flex items-center justify-between p-2">
                <div class="mt-1 cursor-pointer flex items-center">
                    <div class="ml-2 p-2 border border-darkgray rounded">
                        <x-icon name="code" class="w-10 h-10" />
                    </div>
                    <div class="ml-4 flex flex-col">
                        <span class="text-lg font-bold">Junior Developer</span>
                        <span class="text-xs text-gray">Created by OpenAgents</span>
                    </div>
                </div>
                <div class="-mt-2 mr-4">
                    <x-button variant="outline" icon="share">
                        Share
                    </x-button>
                </div>
            </div>
        </div>


        <div class="pt-[60px] pb-[60px] flex-1 overflow-auto bg-gray-900 text-white">
            @foreach($messages as $message)
                @php
                    $message['sender'] = $message['sender'] === 'user' ? 'You' : $agent->name ?? 'Agent';
                @endphp

                <x-message :author="$message['sender']" :message="$message['body']" />
            @endforeach
        </div>

        <div class="fixed bottom-0 left-[300px] right-0 h-[80px] bg-black px-4 py-3 flex items-center z-10">
            <div
                class="w-full pt-2 md:pt-0 dark:border-white/20 md:border-transparent md:dark:border-transparent md:w-[calc(100%-.5rem)]">
                <form
                    class="stretch mx-2 flex flex-row gap-3 last:mb-2 md:mx-4 md:last:mb-6 lg:mx-auto lg:max-w-2xl xl:max-w-3xl">
                    <div class="relative flex h-full flex-1 items-stretch md:flex-col">
                        <div class="flex w-full items-center text-white">
                            <div
                                class="overflow-hidden [&amp;:has(textarea:focus)]:border-gray [&amp;:has(textarea:focus)]:shadow-[0_2px_6px_rgba(0,0,0,.05)] flex flex-col w-full dark:border-gray flex-grow relative border border-gray dark:text-white rounded-[6px]">
                                <textarea id="prompt-textarea" tabindex="0" rows="1"
                                    placeholder="Message Junior Developer…"
                                    class="outline-none m-0 w-full resize-none border-0 bg-transparent focus:ring-0 focus-visible:ring-0 dark:bg-transparent max-h-25 py-[10px] pr-10 md:py-3.5 md:pr-12 placeholder-white/50 pl-10 md:pl-[55px]"
                                    style="height: 52px; overflow-y: hidden;"></textarea>
                                <div class="absolute bottom-2 md:bottom-3 left-2 md:left-4">
                                    <div class="flex">
                                        <button class="btn relative p-0 text-gray" aria-label="Attach files">
                                            <div class="flex w-full gap-2 items-center justify-center">
                                                <svg width="24" height="24" viewBox="0 0 24 24" fill="none"
                                                    xmlns="http://www.w3.org/2000/svg">
                                                    <path fill-rule="evenodd" clip-rule="evenodd"
                                                        d="M9 7C9 4.23858 11.2386 2 14 2C16.7614 2 19 4.23858 19 7V15C19 18.866 15.866 22 12 22C8.13401 22 5 18.866 5 15V9C5 8.44772 5.44772 8 6 8C6.55228 8 7 8.44772 7 9V15C7 17.7614 9.23858 20 12 20C14.7614 20 17 17.7614 17 15V7C17 5.34315 15.6569 4 14 4C12.3431 4 11 5.34315 11 7V15C11 15.5523 11.4477 16 12 16C12.5523 16 13 15.5523 13 15V9C13 8.44772 13.4477 8 14 8C14.5523 8 15 8.44772 15 9V15C15 16.6569 13.6569 18 12 18C10.3431 18 9 16.6569 9 15V7Z"
                                                        fill="currentColor"></path>
                                                </svg>
                                            </div>
                                        </button>
                                        <input multiple="" type="file" tabindex="-1" class="hidden"
                                            style="display: none;">
                                    </div>
                                </div>
                                <button disabled=""
                                    class="absolute bottom-1.5 right-2 rounded-lg border border-black bg-black p-0.5 text-white transition-colors enabled:bg-black disabled:text-gray-400 disabled:opacity-25 dark:border-white dark:bg-white dark:hover:bg-white md:bottom-3 md:right-3"
                                    data-testid="send-button">
                                    <span class="" data-state="closed">
                                        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" class="text-gray">
                                            <path d="M7 11L12 6L17 11M12 18V7" stroke="currentColor" stroke-width="2"
                                                stroke-linecap="round" stroke-linejoin="round"></path>
                                        </svg>
                                    </span>
                                </button>
                            </div>
                        </div>
                    </div>
                </form>
            </div>
        </div>
    </div>
</div>
