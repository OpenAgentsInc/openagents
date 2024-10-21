<x-layouts.empty>
    <div class="h-screen w-screen flex bg-background">
        <!-- Sidebar -->
        <div class="w-1/4 bg-zinc-800 text-white p-4">
            <h2 class="pt-12 text-xl font-bold mb-4">Chats</h2>
            <ul>
                @foreach($threads as $thread)
                <li class="mb-2">
                    <a href="#" class="hover:text-zinc-300" hx-get="{{ route('chat.messages', $thread) }}" hx-target="#message-list" hx-trigger="click">
                        {{ $thread->title }}
                    </a>
                </li>
                @endforeach
            </ul>
        </div>

        <!-- Main Content -->
        <div class="w-3/4 flex flex-col h-screen justify-center items-center">
            <div id="main-content" class="flex-grow overflow-y-auto p-4">
                <div id="message-list" class="space-y-4">
                    <x-empty-message-list />
                </div>
            </div>
            <div class="p-4 w-[650px]">
                @include('dashboard.message-form', ['thread' => $threads->first()])
            </div>
        </div>
    </div>
</x-layouts.empty>
