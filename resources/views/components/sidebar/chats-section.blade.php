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
    <div id="chatsContent" 
         x-show="chatsExpanded" 
         x-transition:enter="transition ease-out duration-100" 
         x-transition:enter-start="opacity-0 transform scale-95" 
         x-transition:enter-end="opacity-100 transform scale-100" 
         x-transition:leave="transition ease-in duration-75" 
         x-transition:leave-start="opacity-100 transform scale-100" 
         x-transition:leave-end="opacity-0 transform scale-95"
         hx-get="{{ route('threads.index') }}"
         hx-trigger="revealed, teamChanged from:body"
         hx-target="#thread-list"
         hx-include="#team-id-input">
        <input type="hidden" id="team-id-input" name="team_id" value="{{ auth()->user()->currentTeam->id ?? '' }}">
        <div id="thread-list">
            @if(auth()->user()->currentTeam)
                <!-- Thread list will be loaded here -->
                <p class="text-gray-500 dark:text-gray-400">Loading threads...</p>
            @else
                <p class="text-gray-500 dark:text-gray-400">No team selected. Please create or join a team to see chats.</p>
            @endif
        </div>
    </div>
</div>