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
                class="btn btn-square btn-sm btn-ghost rounded ml-0.5"
                aria-label="Toggle sidebar">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
                </svg>
            </button>
        </div>
        <div id="sidebarContent" class="flex-grow overflow-hidden">
            <div class="w-[270px] h-full overflow-y-auto p-4">
                <h2 class="text-lg font-semibold mb-4">Sidebar Content</h2>
                <ul>
                    <li class="mb-2"><a href="#" class="text-blue-500 hover:underline">Home</a></li>
                    <li class="mb-2"><a href="#" class="text-blue-500 hover:underline">About</a></li>
                    <li class="mb-2"><a href="#" class="text-blue-500 hover:underline">Contact</a></li>
                </ul>
            </div>
        </div>
    </div>
</div>