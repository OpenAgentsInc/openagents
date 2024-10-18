<style>
    .sidebar-init #sidebar,
    .sidebar-init #sidebarContent {
        transition: none !important;
    }

    #sidebarContent {
        opacity: var(--sidebar-content-opacity, 1);
        visibility: var(--sidebar-content-visibility, visible);
        transition: opacity 0.2s ease-in-out, visibility 0.2s ease-in-out;
    }
</style>
<div id="sidebar" class="h-full overflow-hidden flex flex-col transition-all duration-300 ease-in-out"
    style="width: var(--sidebar-width, 270px);">
    <div class="bg-background flex-grow border-r border-border flex flex-col">
        <div class="p-4">
            <button
                id="sidebarToggle"
                class="btn btn-square btn-sm btn-ghost rounded ml-1.5"
                aria-label="Toggle sidebar">
                <x-icons.sidebar class="h-6 w-6" />
            </button>
        </div>
        <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col">
            <div class="w-[270px] flex-grow overflow-y-auto p-4">
                <!-- sidebar content -->
            </div>
            <div class="p-4 border-t border-border">
                <a href="{{ route('logout') }}" class="btn btn-outline btn-block">
                    Logout
                </a>
            </div>
        </div>
    </div>
</div>