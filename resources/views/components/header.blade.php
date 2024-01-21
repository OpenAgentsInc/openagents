<header class="relative">
    <div class="px-4 sm:px-6 md:px-8">
        <div
            class="relative pt-6 flex items-center justify-between text-grey-700 font-semibold text-sm leading-6 dark:text-grey-200">
            <div class="text-black dark:text-white w-44">
                @include('partials.logo')
            </div>
            <div class="flex items-center">
                <nav>
                    <ul class="flex items-center gap-x-8">
                        <li><a class="hover:text-grey-500 dark:hover:text-grey-400" href="/plugins">Plugins</a></li>
                        <li><a class="hover:text-grey-500 dark:hover:text-grey-400" href="/blog">Blog</a></li>
                    </ul>
                </nav>
                <div class="flex items-center border-l border-grey-200 ml-6 pl-6 dark:border-grey-800">
                    <x-theme-switcher />
                    <x-github-icon />
                    <x-twitter-x-icon />
                </div>
            </div>
        </div>

    </div>
</header>
