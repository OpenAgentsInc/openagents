<div class="p-4 border-t border-sidebar-border">
    <div x-data="{ open: false }" class="relative">
        <button @click="open = !open" class="flex items-center space-x-3 w-full cursor-pointer">
            <div class="w-6 h-6 flex items-center justify-center bg-sidebar-accent rounded-full">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 text-sidebar-foreground" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z" />
                </svg>
            </div>
            <div class="flex-grow text-left">
                <p class="text-sm font-medium text-sidebar-foreground">{{ Auth::user()->name }}</p>
                <p class="text-xs text-sidebar-foreground opacity-70">{{ Auth::user()->email }}</p>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4 transition duration-200 ease-in-out text-sidebar-foreground" :class="{ 'transform rotate-180': open }">
                <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
        </button>
        <div x-show="open" @click.away="open = false" class="absolute bottom-full left-0 w-full mb-2 bg-sidebar-background rounded-md shadow-md border border-sidebar-border">
            <div class="p-2">
                <a href="/logout" class="block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground rounded-md">Log out</a>
            </div>
        </div>
    </div>
</div>
