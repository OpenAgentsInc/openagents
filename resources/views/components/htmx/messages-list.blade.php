<div class="p-4 space-y-4">
    @if(isset($thread))
        <h2 class="text-lg font-bold">Messages for Thread: {{ $thread->title }}</h2>
        <ul class="space-y-2">
            @foreach($messages as $message)
                <li class="p-2 bg-gray-700 text-white rounded-md">
                    <div><strong>from</strong> <span
                                class="text-sm text-gray-400">- {{ $message->created_at->format('d M Y, h:i A') }}</span>
                    </div>
                    <div>{{ $message->body }}</div>
                </li>
            @endforeach
        </ul>
    @else
        <p>Select a thread to view messages.</p>
    @endif
</div>
