<div class="relative max-w-8xl mx-auto lg:border-b lg:border-gray-500/5 dark:border-gray-50/[0.06]">
    <div class="py-5 lg:py-4 lg:px-12 border-b border-gray-500/10 lg:border-0 dark:border-gray-300/10 mx-4 lg:mx-0">
        <div class="relative flex items-center">
            <div class="flex-1 flex items-center space-x-4"><a href="/"><span class="sr-only">OpenAgents home
                        page</span><img class="w-auto h-7 relative block dark:hidden"
                        src="{{ asset('images/light.svg') }}" alt="light logo"><img
                        class="w-auto h-7 relative hidden dark:block"
                        src="{{ asset('images/dark.svg') }}" alt="dark logo"></a></div>
            <div class="flex-1 relative hidden lg:flex items-center ml-auto justify-end">
                <nav class="text-sm leading-6 font-semibold text-gray-700 dark:text-gray-200">
                    <ul class="flex space-x-8 items-center"></ul>
                </nav>
                <div class="flex items-center">
                    <!--
                    <div
                        class="border-l hidden lg:flex border-gray-100 ml-6 pl-6 dark:border-background-dark dark:brightness-200 h-6">
                    </div>
                    -->
                    <x-theme-switcher />
                </div>
            </div>
            <!--
            <div class="flex lg:hidden items-center"><button type="button"
                    class="ml-auto text-gray-500 w-8 h-8 -my-1 items-center justify-center hover:text-gray-600 dark:text-gray-400 dark:hover:text-gray-300"><span
                        class="sr-only">Search</span><svg
                        class="h-4 w-4 bg-gray-500 dark:bg-gray-400 hover:bg-gray-600 dark:hover:bg-gray-300"
                        style="-webkit-mask-image:url(https://mintlify.b-cdn.net/v6.4.0/solid/magnifying-glass.svg);-webkit-mask-repeat:no-repeat;-webkit-mask-position:center"></svg></button><button
                    class="h-7 w-5 flex items-center justify-end"><svg
                        class="h-4 w-4 bg-gray-500 dark:bg-gray-400 hover:bg-gray-600 dark:hover:bg-gray-300"
                        style="-webkit-mask-image:url(https://mintlify.b-cdn.net/v6.4.0/solid/ellipsis-vertical.svg);-webkit-mask-repeat:no-repeat;-webkit-mask-position:center"></svg></button>
            </div>
            -->
        </div>
    </div>
</div>
