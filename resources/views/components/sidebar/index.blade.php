<div
    x-data="{ isCollapsed: false, chatsExpanded: true }"
    x-init="
        isCollapsed = localStorage.getItem('sidebarCollapsed') === 'true';
        chatsExpanded = localStorage.getItem('chatsExpanded') !== 'false';
        $nextTick(() => {
            document.getElementById('sidebar').style.visibility = 'visible';
        });
        $watch('isCollapsed', value => localStorage.setItem('sidebarCollapsed', value));
        $watch('chatsExpanded', value => localStorage.setItem('chatsExpanded', value));
    "
    id="sidebar"
    class="h-full overflow-hidden transition-all duration-300 ease-in-out border-r border-sidebar-border invisible"
    :class="{ 'w-[60px]': isCollapsed, 'w-[270px]': !isCollapsed }">
    <div class="sidebar-inner bg-sidebar-background h-full">
        <div class="sidebar-content flex flex-col">
            <div class="flex flex-col space-y-4 py-3">
                <x-sidebar.team-switcher :isCollapsed="true" x-bind:is-collapsed="isCollapsed" />
            </div>
            <div id="sidebarDivider" class="my-4 mx-4 h-[1px] bg-sidebar-border opacity-0 transition-opacity duration-300" :class="{ 'opacity-50': !isCollapsed }"></div>
            <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col opacity-0 transition-opacity duration-300" :class="{ 'opacity-100': !isCollapsed }">
                <div class="flex-grow overflow-y-auto">
                    <div class="w-[270px] p-4">
                        <!-- Chats Section -->
                        <div id="chatsSection" class="space-y-2 mb-4">
                            <button class="flex items-center justify-between w-full text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent rounded-md p-2" @click="chatsExpanded = !chatsExpanded">
                                <div class="flex items-center space-x-2">
                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                        <path fill-rule="evenodd" d="M18 10c0 3.866-3.582 7-8 7a8.841 8.841 0 01-4.083-.98L2 17l1.338-3.123C2.493 12.767 2 11.434 2 10c0-3.866 3.582-7 8-7s8 3.134 8 7zM7 9H5v2h2V9zm8 0h-2v2h2V9zM9 9h2v2H9V9z" clip-rule="evenodd" />
                                    </svg>
                                    <span>Chats</span>
                                </div>
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 transform transition-transform duration-200" :class="{ 'rotate-180': chatsExpanded }" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                                </svg>
                            </button>
                            <div id="chatsContent" x-show="chatsExpanded" x-transition:enter="transition ease-out duration-100" x-transition:enter-start="opacity-0 transform scale-95" x-transition:enter-end="opacity-100 transform scale-100" x-transition:leave="transition ease-in duration-75" x-transition:leave-start="opacity-100 transform scale-100" x-transition:leave-end="opacity-0 transform scale-95">
                                <ul class="space-y-2">
                                    @foreach(range(1, 5) as $index)
                                    <li class="bg-sidebar-accent bg-opacity-10 p-2 rounded">
                                        <p class="text-sm font-medium text-sidebar-foreground">Chat #{{ $index }}</p>
                                        <p class="text-xs text-sidebar-foreground opacity-70">Last message...</p>
                                    </li>
                                    @endforeach
                                </ul>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="flex-shrink-0">
                    <x-sidebar.sidebar-footer />
                </div>
            </div>
        </div>
    </div>
</div>

<script>
    function toggleSection(sectionId) {
        const content = document.getElementById(sectionId);
        const isHidden = content.classList.contains('hidden');
        content.classList.toggle('hidden', !isHidden);
        const button = content.previousElementSibling;
        const svg = button.querySelector('svg:last-child');
        svg.style.transform = isHidden ? 'rotate(180deg)' : '';
    }
</script>
