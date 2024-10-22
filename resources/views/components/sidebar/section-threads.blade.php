<div data-sidebar="group" class="relative flex w-full min-w-0 flex-col p-2">
    <div data-sidebar="group-label" class="duration-200 flex h-8 shrink-0 items-center rounded-md px-2 text-xs font-medium text-sidebar-foreground/70 outline-none ring-sidebar-ring transition-[margin,opa] ease-linear focus-visible:ring-2 [&amp;>svg]:size-4 [&amp;>svg]:shrink-0 group-data-[collapsible=icon]:-mt-8 group-data-[collapsible=icon]:opacity-0">Recent Threads</div>
    <ul data-sidebar="menu" class="flex w-full min-w-0 flex-col gap-1">
        <li data-sidebar="menu-item" class="group/menu-item relative group/collapsible" data-state="open">
            <button data-sidebar="menu-button" data-size="default" data-active="false" class="peer/menu-button flex w-full items-center gap-2 overflow-hidden rounded-md p-2 text-left outline-none ring-sidebar-ring transition-[width,height,padding] focus-visible:ring-2 active:bg-sidebar-accent active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 group-has-[[data-sidebar=menu-action]]/menu-item:pr-8 aria-disabled:pointer-events-none aria-disabled:opacity-50 data-[active=true]:bg-sidebar-accent data-[active=true]:font-medium data-[active=true]:text-sidebar-accent-foreground data-[state=open]:hover:bg-sidebar-accent data-[state=open]:hover:text-sidebar-accent-foreground group-data-[collapsible=icon]:!size-8 group-data-[collapsible=icon]:!p-2 [&amp;>span:last-child]:truncate [&amp;>svg]:size-4 [&amp;>svg]:shrink-0 hover:bg-sidebar-accent hover:text-sidebar-accent-foreground h-8 text-sm" type="button" aria-controls="radix-:Rpaapuuuuu6ja:" aria-expanded="true" data-state="open">
                <span>Recent Threads</span>
                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-chevron-right ml-auto transition-transform duration-200 group-data-[state=open]/collapsible:rotate-90">
                    <path d="m9 18 6-6-6-6"></path>
                </svg>
            </button>
            <div data-state="open" id="radix-:Rpaapuuuuu6ja:" style="--radix-collapsible-content-height: auto; --radix-collapsible-content-width: 100%;">
                <ul data-sidebar="menu-sub" class="mx-3.5 flex min-w-0 translate-x-px flex-col gap-1 border-l border-sidebar-border px-2.5 py-0.5 group-data-[collapsible=icon]:hidden">
                    @forelse ($recentThreads as $thread)
                    <li>
                        <a href="{{ route('threads.show', $thread) }}"
                            data-sidebar="menu-sub-button"
                            data-size="md"
                            class="flex h-7 min-w-0 -translate-x-px items-center gap-2 overflow-hidden rounded-md px-2 text-sidebar-foreground outline-none ring-sidebar-ring hover:bg-sidebar-accent hover:text-sidebar-accent-foreground focus-visible:ring-2 active:bg-sidebar-accent active:text-sidebar-accent-foreground disabled:pointer-events-none disabled:opacity-50 aria-disabled:pointer-events-none aria-disabled:opacity-50 [&amp;>span:last-child]:truncate [&amp;>svg]:size-4 [&amp;>svg]:shrink-0 [&amp;>svg]:text-sidebar-accent-foreground data-[active=true]:bg-sidebar-accent data-[active=true]:text-sidebar-accent-foreground text-sm group-data-[collapsible=icon]:hidden"
                            hx-get="{{ route('threads.show', $thread) }}"
                            hx-target="#main-content"
                            hx-push-url="true"
                            title="{{ $thread->title }}">
                            <span>{{ $thread->title }}</span>
                        </a>
                    </li>
                    @empty
                    <li>
                        <span class="flex h-7 min-w-0 -translate-x-px items-center gap-2 overflow-hidden rounded-md px-2 text-sidebar-foreground text-sm">
                            No recent threads
                        </span>
                    </li>
                    @endforelse
                </ul>
            </div>
        </li>
    </ul>
</div>