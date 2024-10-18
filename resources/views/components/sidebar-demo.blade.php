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
        <div class="p-4 flex items-center justify-between">
            <button
                id="sidebarToggle"
                class="btn btn-square btn-sm btn-ghost rounded"
                aria-label="Toggle sidebar">
                <x-icons.sidebar class="h-6 w-6" />
            </button>
            <div class="flex items-center space-x-2">
                <select class="select select-bordered select-sm w-32" id="teamSwitcher">
                    <option>Personal</option>
                    <option>Team Alpha</option>
                    <option>Team Beta</option>
                </select>
                <button class="btn btn-sm btn-ghost" aria-label="Create new team">
                    <x-icons.plus class="h-4 w-4" />
                </button>
            </div>
        </div>
        <div class="px-4 py-2">
            <select class="select select-bordered w-full" id="projectSwitcher">
                <option>Active Project: Project X</option>
                <option>Project Y</option>
                <option>Project Z</option>
            </select>
        </div>
        <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col">
            <div class="w-[270px] flex-grow overflow-y-auto p-4">
                <h3 class="font-semibold mb-2">Recent Messages</h3>
                <ul class="space-y-2">
                    @foreach(range(1, 10) as $index)
                        <li class="bg-secondary/10 p-2 rounded">
                            <p class="text-sm font-medium">Message #{{ $index }}</p>
                            <p class="text-xs text-muted-foreground">Lorem ipsum dolor sit amet...</p>
                        </li>
                    @endforeach
                </ul>
            </div>
            <div class="p-4 border-t border-border">
                <div class="flex items-center space-x-3 cursor-pointer" id="userCard">
                    <div class="avatar">
                        <div class="w-10 rounded-full">
                            <img src="https://i.pravatar.cc/100" alt="User avatar" />
                        </div>
                    </div>
                    <div>
                        <p class="font-medium">John Doe</p>
                        <p class="text-xs text-muted-foreground">john@example.com</p>
                    </div>
                </div>
            </div>
        </div>
    </div>
</div>

<script>
    document.getElementById('userCard').addEventListener('click', function() {
        // TODO: Implement user menu functionality
        alert('User menu clicked. Add logout and manage subscription options here.');
    });
</script>