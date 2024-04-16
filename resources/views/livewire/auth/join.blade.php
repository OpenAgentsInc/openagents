<div class="bg-black text-white p-[32px]" x-data="{ hasNostr: typeof window.nostr !== 'undefined' }">
    <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">Join OpenAgents</h2>

    <div class="my-6">
        <a href="/login/x">
            <x-secondary-button class="w-full flex justify-center gap-2 mb-0 h-[44px]">
                <x-icon.x class="h-5 w-5"></x-icon.x>
                <span class="">Continue with X</span>
            </x-secondary-button>
        </a>
    </div>

    <div class="my-6" x-show="hasNostr" x-cloak>
        <a href="/login/nostr" class="mt-6">
            <x-secondary-button class="w-full flex justify-center gap-2 mb-0 h-[44px]">
                <span class="">Continue with Nostr</span>
            </x-secondary-button>
        </a>
    </div>

    <div class="text-center">
        <p class="mt-2 mb-0 text-sm text-gray">
            By continuing you agree to our
            <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
               href="/terms" target="_blank">
                Terms of Service
            </a>
            and
            <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
               href="/privacy" target="_blank">
                Privacy Policy.
            </a>
        </p>
    </div>
</div>
