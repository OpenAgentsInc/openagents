<x-layout>
    <div class="h-full w-full text-foreground flex flex-col">
        <div class="flex-1 overflow-hidden flex flex-col">
            <main class="flex-1 overflow-y-auto">
                <div class="max-w-4xl mx-auto p-4 pt-16">
                    <h1 class="text-2xl font-bold mb-4">Chat Thread: {{ $thread->title }}</h1>
                    @if (count($thread->messages) > 0)
                        @foreach ($thread->messages as $message)
                            <x-chat.message :message="$message" />
                        @endforeach
                    @endif
                </div>
            </main>
        </div>

        <div class="flex-shrink-0 w-full">
            <div class="max-w-4xl mx-auto px-4 mb-2">
                <form action="{{ route('messages.store', ['thread' => $thread->id]) }}" method="POST">
                    @csrf
                    <div class="flex items-center space-x-2">
                        <textarea name="content" class="flex-grow rounded-md border-gray-300 shadow-sm focus:border-indigo-300 focus:ring focus:ring-indigo-200 focus:ring-opacity-50" rows="3" required></textarea>
                        <button type="submit" class="inline-flex items-center px-4 py-2 bg-blue-600 border border-transparent rounded-md font-semibold text-xs text-white uppercase tracking-widest hover:bg-blue-700 active:bg-blue-800 focus:outline-none focus:border-blue-800 focus:ring focus:ring-blue-200 disabled:opacity-25 transition">Send</button>
                    </div>
                </form>
            </div>
            <div class="pb-2 text-center text-xs text-zinc-500">
                @if (auth()->check() && auth()->user()->currentTeam)
                    Messages visible to all members of {{ auth()->user()->currentTeam->name }}
                @else
                    Messages are visible only to you
                @endif
            </div>
        </div>
    </div>
</x-layout>