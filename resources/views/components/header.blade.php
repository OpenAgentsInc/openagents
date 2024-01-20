<header class="relative">
    <div class="px-4 sm:px-6 md:px-8">
        <div
            class="relative pt-6 flex items-center justify-between text-slate-700 font-semibold text-sm leading-6 dark:text-slate-200">
            <div class="text-slate-900 dark:text-moonraker w-44">
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

    </div>
</header>
