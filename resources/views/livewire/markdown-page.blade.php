<div class="relative z-0 flex h-full w-full overflow-hidden min-h-screen">
    <main class="relative h-full w-full flex-1 overflow-auto transition-width">
        <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
            <div class="min-h-screen flex flex-col items-center pt-6 sm:pt-0">
                <a href="/" wire:navigate class="mt-12">
                    <x-icon.logo class="w-20 h-20 text-white"/>
                </a>

                <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
                    {!! $markdownContent!!}
                </div>
            </div>
        </div>
    </main>
    @if ($includeTwitterSdk)
        @include('partials.twitter')
    @endif
</div>