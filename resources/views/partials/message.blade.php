<li class="bg-zinc-800 p-4 rounded-lg">
    <p class="text-sm text-zinc-400">{{ $message->created_at->format('M d, Y H:i') }}</p>
    <p class="mt-2 message-content">{{ $message->content }}</p>
</li>