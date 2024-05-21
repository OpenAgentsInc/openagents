@props(['message'])

<li class="p-2 bg-gray-700 text-white rounded-md">
    <div>
        <strong>from!</strong>
        <span class="text-sm text-gray-400">- {{ $message->created_at->format('d M Y, h:i A') }}</span>
    </div>
    <div class="message-body">{{ $message->body }}</div>
</li>
