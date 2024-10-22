@props(['isCollapsed' => false])

<div class="px-3 flex items-center justify-between">
    <button
        id="sidebarToggle"
        @click="isCollapsed = !isCollapsed"
        class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0"
        aria-label="Toggle sidebar">
        <x-icons.sidebar class="h-6 w-6" />
    </button>
</div>