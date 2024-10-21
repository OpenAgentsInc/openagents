<h2 class="text-xl font-semibold mb-4">{{ $thread->title }}</h2>
<ul class="space-y-4" id="message-list">
    @foreach($messages as $message)
        <li class="bg-zinc-800 p-4 rounded-lg">
            <p class="text-sm text-zinc-400">{{ $message->created_at->format('M d, Y H:i') }}</p>
            <p class="mt-2">{{ $message->content }}</p>
        </li>
    @endforeach
</ul>

@include('dashboard.message-form', ['thread' => $thread])