<x-blog-layout>
    <div class="pt-4 pb-24 bg-gray-100 dark:bg-gray-900">
        <div class="min-h-screen flex flex-col items-center pt-6 sm:pt-0">
            <a href="/" wire:navigate class="mt-12">
                <x-icon.logo class="w-20 h-20 text-white"/>
            </a>

            <a href="/" wire:navigate>
                <h3 class="text-[16px] fixed top-[18px] left-[24px] text-gray"> &larr; Back to chat</h3>
            </a>

            <div class="w-full sm:max-w-2xl mt-6 p-6 bg-black shadow-md overflow-hidden sm:rounded-lg prose prose-invert">
                {!! $policy !!}
            </div>
        </div>
    </div>
</x-blog-layout>
