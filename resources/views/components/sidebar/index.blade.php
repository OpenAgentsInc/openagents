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
                        <x-sidebar.chats-section />
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