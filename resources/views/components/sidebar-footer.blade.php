<div class="p-4 border-t border-border">
    <div x-data="{ open: false }" class="relative">
        <button @click="open = !open" class="flex items-center space-x-3 w-full cursor-pointer">
            <div class="avatar">
                <div class="w-10 rounded-full">
                    <img src="https://pbs.twimg.com/profile_images/1607882836740120576/3Tg1mTYJ_400x400.jpg" alt="User avatar" />
                </div>
            </div>
            <div class="flex-grow text-left">
                <p class="font-medium">Christopher David</p>
                <p class="text-xs text-muted-foreground">chris@openagents.com</p>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="w-4 h-4 transition duration-200 ease-in-out" :class="{ 'transform rotate-180': open }">
                <polyline points="6 9 12 15 18 9"></polyline>
            </svg>
        </button>
        <div x-show="open" @click.away="open = false" class="absolute bottom-full left-0 w-full mb-2 bg-popover rounded-md shadow-md border border-border">
            <div class="p-2">
                <a href="#" class="block px-4 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-md">Manage Account</a>
                <a href="#" class="block px-4 py-2 text-sm hover:bg-accent hover:text-accent-foreground rounded-md">Logout</a>
            </div>
        </div>
    </div>
</div>
