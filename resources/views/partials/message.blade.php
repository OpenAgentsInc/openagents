<div class="message {{ $message->is_system_message ? 'system-message' : 'user-message' }}">
    <strong>{{ $message->user ? $message->user->name : 'System' }}:</strong>
    <p>{{ $message->content }}</p>
</div>