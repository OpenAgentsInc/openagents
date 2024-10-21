@props(['isCollapsed' => false])

<div class="px-3 flex items-center justify-between">
    <button
        id="sidebarToggle"
        @click="isCollapsed = !isCollapsed"
        class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0"
        aria-label="Toggle sidebar">
        <x-icons.sidebar class="h-6 w-6" />
    </button>
    <button 
        id="newChatButton" 
        class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0" 
        aria-label="Create new chat"
        x-show="!isCollapsed"
        x-transition:enter="transition ease-out duration-300"
        x-transition:enter-start="opacity-0 scale-90"
        x-transition:enter-end="opacity-100 scale-100"
        x-transition:leave="transition ease-in duration-300"
        x-transition:leave-start="opacity-100 scale-100"
        x-transition:leave-end="opacity-0 scale-90"
        hx-post="{{ route('threads.create') }}"
        hx-swap="none"
        hx-trigger="click"
        @htmx:after-request="if(event.detail.successful) { window.location.href = event.detail.xhr.response.redirect; }">
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
        </svg>
    </button>
</div>