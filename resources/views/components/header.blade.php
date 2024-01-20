<header class="relative">
    <div class="px-4 sm:px-6 md:px-8">
        <div
            class="relative pt-6 flex items-center justify-between text-slate-700 font-semibold text-sm leading-6 dark:text-slate-200">
            <div class="text-slate-900 dark:text-white w-44">
                @include('partials.logo')
            </div>
            <div class="flex items-center">
                <nav>
                    <!--
                    <ul class="flex items-center gap-x-8">
                        <li><a class="hover:text-slate-500 dark:hover:text-slate-400" href="/blog">Blog</a></li>
                    </ul>
                    -->
                </nav>
                <div class="flex items-center border-l border-slate-200 ml-6 pl-6 dark:border-portgore">
                    <x-theme-switcher />
                    <x-github-icon />
                    <x-twitter-x-icon />
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
