<div id="thread-list" class="space-y-2">
    @forelse($threads as $thread)
        <a href="{{ route('chat.show', $thread->id) }}" 
           class="block p-2 hover:bg-sidebar-accent rounded-md transition-colors duration-200"
           hx-get="{{ route('chat.show', $thread->id) }}"
           hx-target="#main-content-inner"
           hx-push-url="true"
           hx-include="this"
           hx-params="*">
            <div class="text-sm font-medium text-sidebar-foreground">{{ $thread->title }}</div>
            <div class="text-xs text-sidebar-muted">{{ $thread->created_at->diffForHumans() }}</div>
        </a>
    @empty
        <p class="text-sidebar-muted text-sm">No chats yet. Start a new conversation!</p>
    @endforelse
</div>