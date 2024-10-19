@php
$isCollapsed = false; // Default value
@endphp

<div
    x-data="{ isCollapsed: localStorage.getItem('sidebarCollapsed') === 'true' }"
    x-init="$watch('isCollapsed', value => localStorage.setItem('sidebarCollapsed', value))"
    id="sidebar"
    class="h-full overflow-hidden transition-all duration-300 ease-in-out border-r border-sidebar-border"
    :style="{ width: isCollapsed ? '60px' : '270px' }"
>
    <div class="sidebar-inner bg-sidebar-background h-full">
        <div class="sidebar-content flex flex-col">
            <div class="flex flex-col space-y-4 py-3">
                <x-team-switcher :isCollapsed="$isCollapsed" />
            </div>
            <div id="sidebarDivider" class="my-4 mx-4 h-[1px] bg-sidebar-border opacity-50" :style="{ opacity: isCollapsed ? 0 : 1 }"></div>
            <div id="sidebarContent" class="flex-grow overflow-hidden flex flex-col" :style="{ opacity: isCollapsed ? 0 : 1 }">
                <div class="flex-grow overflow-y-auto">
                    <div class="w-[270px] p-4">
                        <!-- Chats Section -->
                        <div id="chatsSection" class="space-y-2 mb-4">
                            <button class="flex items-center justify-between w-full text-sm font-medium text-sidebar-foreground hover:bg-sidebar-accent rounded-md p-2" onclick="toggleSection('chatsContent')">
                                <div class="flex items-center space-x-2">
                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" viewBox="0 0 20 20" fill="currentColor">
                                        <path fill-rule="evenodd" d="M18 10c0 3.866-3.582 7-8 7a8.841 8.841 0 01-4.083-.98L2 17l1.338-3.123C2.493 12.767 2 11.434 2 10c0-3.866 3.582-7 8-7s8 3.134 8 7zM7 9H5v2h2V9zm8 0h-2v2h2V9zM9 9h2v2H9V9z" clip-rule="evenodd" />
                                    </svg>
                                    <span>Chats</span>
                                </div>
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 transform transition-transform duration-200" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                                </svg>
                            </button>
                            <div id="chatsContent" class="hidden">
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
                    <x-sidebar-footer />
                </div>
            </div>
        </div>
    </div>
</div>