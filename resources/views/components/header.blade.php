<header class="relative">
    <div class="px-4 sm:px-6 md:px-8">
        <div class="absolute inset-0 bottom-10 bg-bottom bg-no-repeat bg-slate-50 dark:bg-haiti">
            <div class="absolute inset-0 bg-grid-slate-900/[0.04] bg-[bottom_1px_center] dark:bg-grid-slate-400/[0.05]"
                style="mask-image:linear-gradient(to bottom, transparent, black);-webkit-mask-image:linear-gradient(to bottom, transparent, black)">
            </div>
        </div>
        <div
            class="relative pt-6 flex items-center justify-between text-slate-700 font-semibold text-sm leading-6 dark:text-slate-200">
            <div class="text-slate-900 dark:text-white w-44">
                @include('partials.logo')
            </div>
            <div class="flex items-center">
                <button type="button"
                    class="text-slate-500 hover:text-slate-600 w-8 h-8 -my-1 flex items-center justify-center md:hidden dark:hover:text-slate-300">
                    <span class="sr-only">Search</span>
                    <svg width="24" height="24" fill="none" stroke="currentColor" stroke-width="2"
                        stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                        <path d="m19 19-3.5-3.5"></path>
                        <circle cx="11" cy="11" r="6"></circle>
                    </svg>
                </button>
                <div class="-my-1 ml-2 -mr-1 md:hidden">
                    <button type="button"
                        class="text-slate-500 w-8 h-8 flex items-center justify-center hover:text-slate-600 dark:text-slate-400 dark:hover:text-slate-300">
                        <span class="sr-only">Navigation</span><svg width="24" height="24" fill="none"
                            aria-hidden="true">
                            <path
                                d="M12 6v.01M12 12v.01M12 18v.01M12 7a1 1 0 1 1 0-2 1 1 0 0 1 0 2Zm0 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Zm0 6a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"
                                stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
                            </path>
                        </svg>
                    </button>
                    <div
                        style="position:fixed;top:1px;left:1px;width:1px;height:0;padding:0;margin:-1px;overflow:hidden;clip:rect(0, 0, 0, 0);white-space:nowrap;border-width:0;display:none">
                    </div>
                </div>
                <div class="hidden md:flex items-center">
                    <nav>
                        <ul class="flex items-center gap-x-8">
                            <li><a class="hover:text-purple-500 dark:hover:text-purple-400"
                                    href="/docs/installation">Docs</a></li>
                            <li><a class="hover:text-purple-500 dark:hover:text-purple-400" href="/blog">Blog</a></li>
                        </ul>
                    </nav>

                    <div class="flex items-center border-l border-slate-200 ml-6 pl-6 dark:border-portgore">
                        <label class="sr-only">Theme</label>
                        <x-theme-switcher />
                        <a href="https://github.com/OpenAgentsInc/openagents" target="_blank"
                            class="ml-6 block text-slate-400 hover:text-slate-500 dark:hover:text-slate-300"><span
                                class="sr-only">OpenAgents on GitHub</span><svg viewBox="0 0 16 16" class="w-5 h-5"
                                fill="currentColor" aria-hidden="true">
                                <path
                                    d="M8 0C3.58 0 0 3.58 0 8c0 3.54 2.29 6.53 5.47 7.59.4.07.55-.17.55-.38 0-.19-.01-.82-.01-1.49-2.01.37-2.53-.49-2.69-.94-.09-.23-.48-.94-.82-1.13-.28-.15-.68-.52-.01-.53.63-.01 1.08.58 1.23.82.72 1.21 1.87.87 2.33.66.07-.52.28-.87.51-1.07-1.78-.2-3.64-.89-3.64-3.95 0-.87.31-1.59.82-2.15-.08-.2-.36-1.02.08-2.12 0 0 .67-.21 2.2.82.64-.18 1.32-.27 2-.27.68 0 1.36.09 2 .27 1.53-1.04 2.2-.82 2.2-.82.44 1.1.16 1.92.08 2.12.51.56.82 1.27.82 2.15 0 3.07-1.87 3.75-3.65 3.95.29.25.54.73.54 1.48 0 1.07-.01 1.93-.01 2.2 0 .21.15.46.55.38A8.013 8.013 0 0016 8c0-4.42-3.58-8-8-8z">
                                </path>
                            </svg>
                        </a>
                    </div>
                </div>
            </div>
        </div>
        <div class="relative max-w-5xl mx-auto pt-20 sm:pt-24 lg:pt-32">
            <h1
                class="text-slate-900 font-extrabold text-3xl sm:text-4xl lg:text-5xl tracking-tight text-center dark:text-white">
                An open platform for AI agents</h1>
            <p class="mt-6 text-xl text-slate-600 text-center max-w-3xl mx-auto dark:text-bluebell">Built in public
                from scratch. Launching soon!</p>
        </div>
    </div>
</header>
