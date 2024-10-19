@php
$isCollapsed = true; // Set this based on user preference or session state
@endphp

<style>
    .sidebar-init #sidebar,
    .sidebar-init #sidebarContent {
        transition: none !important;
    }

    #sidebarContent,
    #sidebarHeader {
        transition: opacity 0.2s ease-in-out, visibility 0.2s ease-in-out;
    }

    .sidebar-inner {
        width: 270px;
        position: relative;
    }

    .sidebar-content {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        bottom: 0;
    }
</style>

<div id="sidebar" class="h-full overflow-hidden transition-all duration-300 ease-in-out border-r border-sidebar-border"
    style="width: {{ $isCollapsed ? '60px' : '270px' }};">
    <div class="sidebar-inner bg-sidebar-background h-full">
        <div class="sidebar-content flex flex-col">
            <div class="flex flex-col space-y-4 py-3">
                <div class="px-3 flex items-center justify-between">
                    <button
                        id="sidebarToggle"
                        class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0"
                        aria-label="Toggle sidebar">
                        <x-icons.sidebar class="h-6 w-6" />
                    </button>
                    <button id="newTeamButton" class="inline-flex items-center justify-center rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-sidebar-ring disabled:pointer-events-none disabled:opacity-50 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-10 w-10 flex-shrink-0" aria-label="Create new team" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="w-6 h-6">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                        </svg>
                    </button>
                </div>
                <div id="sidebarHeader" class="px-3 space-y-4" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}">
                    <!-- Team Section -->
                    <div id="teamSection" class="space-y-2">
                        <button class="flex items-center justify-between w-full text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent rounded-md p-2" onclick="toggleSection('teamContent')">
                            <div class="flex items-center space-x-2">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                    <path d="M13 6a3 3 0 11-6 0 3 3 0 016 0zM18 8a2 2 0 11-4 0 2 2 0 014 0zM14 15a4 4 0 00-8 0v3h8v-3zM6 8a2 2 0 11-4 0 2 2 0 014 0zM16 18v-3a5.972 5.972 0 00-.75-2.906A3.005 3.005 0 0119 15v3h-3zM4.75 12.094A5.973 5.973 0 004 15v3H1v-3a3 3 0 013.75-2.906z" />
                                </svg>
                                <span>Team</span>
                            </div>
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 transform transition-transform duration-200" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                            </svg>
                        </button>
                        <div id="teamContent" class="hidden">
                            <x-dropdown label="Scramble" :items="['Scramble', 'Personal', 'Team Alpha', 'Team Beta']" id="teamSwitcher" class="w-full" />
                        </div>
                    </div>

                    <!-- Project Section -->
                    <div id="projectSection" class="space-y-2">
                        <button class="flex items-center justify-between w-full text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent rounded-md p-2" onclick="toggleSection('projectContent')">
                            <div class="flex items-center space-x-2">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                    <path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h4a2 2 0 012 2v1H8a3 3 0 00-3 3v1.5a1.5 1.5 0 01-3 0V6z" clip-rule="evenodd" />
                                    <path d="M6 12a2 2 0 012-2h8a2 2 0 012 2v2a2 2 0 01-2 2H2h2a2 2 0 002-2v-2z" />
                                </svg>
                                <span>Project</span>
                            </div>
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 transform transition-transform duration-200" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                            </svg>
                        </button>
                        <div id="projectContent" class="hidden">
                            <x-dropdown label="OIDC" :items="['Project X', 'Project Y', 'Project Z']" id="projectSwitcher" class="w-full" />
                        </div>
                    </div>
                </div>
            </div>
            <div id="sidebarDivider" class="my-4 mx-4 h-[1px] bg-sidebar-border opacity-50" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}"></div>
            <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col" style="{{ $isCollapsed ? 'opacity: 0;' : '' }}">
                <div class="flex-grow overflow-y-auto">
                    <div class="w-[270px] p-4">
                        <ul class="space-y-2">
                            @foreach(range(1, 5) as $index)
                            <li class="bg-sidebar-accent bg-opacity-10 p-2 rounded">
                                <p class="text-sm font-medium text-sidebar-foreground">Message #{{ $index }}</p>
                                <p class="text-xs text-sidebar-foreground opacity-70">Lorem ipsum dolor sit amet...</p>
                            </li>
                            @endforeach
                        </ul>
                    </div>
                </div>
                <div class="flex-shrink-0">
                    <x-sidebar-footer />
                </div>
            </div>
        </div>
    </div>
</div>

<script>
    document.addEventListener('DOMContentLoaded', function() {
        const sidebar = document.getElementById('sidebar');
        const sidebarContent = document.getElementById('sidebarContent');
        const sidebarHeader = document.getElementById('sidebarHeader');
        const newTeamButton = document.getElementById('newTeamButton');
        const sidebarDivider = document.getElementById('sidebarDivider');

        function toggleSidebar() {
            const isCollapsed = sidebar.style.width === '60px';

            sidebar.style.width = isCollapsed ? '270px' : '60px';
            [sidebarContent, sidebarHeader, newTeamButton, sidebarDivider].forEach(el => {
                el.style.opacity = isCollapsed ? '1' : '0';
            });

            // Dispatch custom event
            document.dispatchEvent(new CustomEvent('sidebar-toggled', {
                detail: !isCollapsed
            }));
        }

        document.getElementById('sidebarToggle').addEventListener('click', toggleSidebar);

        function toggleSection(sectionId) {
            const content = document.getElementById(sectionId);
            const isHidden = content.classList.contains('hidden');
            content.classList.toggle('hidden', !isHidden);
            const button = content.previousElementSibling;
            const svg = button.querySelector('svg:last-child');
            svg.style.transform = isHidden ? 'rotate(180deg)' : '';
        }

        window.toggleSection = toggleSection;
    });
</script>