<x-layout>
    <x-slot name="title">
        Chat: {{ $thread->title }}
    </x-slot>

    <div class="py-12">
        <div class="max-w-7xl mx-auto sm:px-6 lg:px-8">
            <div class="bg-background overflow-hidden shadow-sm sm:rounded-lg">
                <div class="p-6 bg-background border-b border-gray-200">
                    <h1 class="text-2xl font-bold mb-4">{{ $thread->title }}</h1>
                    
                    <div class="space-y-4">
                        @foreach ($messages as $message)
                            <div class="p-4 @if($message->user_id == auth()->id()) bg-blue-900 @else bg-gray-800 @endif rounded-lg">
                                <p class="text-sm text-gray-400">
                                    {{ optional($message->user)->name ?? 'Unknown User' }} - {{ $message->created_at->format('M d, Y H:i') }}
                                </p>
                                <p class="mt-1">{{ $message->content }}</p>
                            </div>
                        @endforeach
                    </div>

                    <form action="{{ route('messages.store', $thread) }}" method="POST" class="mt-6">
                        @csrf
                        <textarea name="content" rows="3" class="w-full bg-gray-700 border-gray-600 rounded-md shadow-sm focus:border-indigo-300 focus:ring focus:ring-indigo-200 focus:ring-opacity-50" required></textarea>
                        <button type="submit" class="mt-2 px-4 py-2 bg-blue-500 text-white rounded-md hover:bg-blue-600">Send</button>
                    </form>
                </div>
            </div>
        </div>
    </div>
</x-layout>