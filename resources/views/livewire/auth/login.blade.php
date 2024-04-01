<div class="bg-black text-white p-[32px]">
    <div class="">
        <h2 class="block text-xl text-center font-bold text-gray-800 dark:text-gray-200">Get started</h2>
    </div>

    <div class="px-2 pt-7 sm:px-7">

        <div class="mb-4">
            <x-input id="email" class="block mt-1 w-full" type="email" name="email" :value="old('email')" required
                     autofocus autocomplete="username" placeholder="Enter email..."/>
        </div>

        <div class="mt-5">
            <x-button class="w-full flex justify-center gap-2">
                Get started
            </x-button>

            <div class="py-5 w-full text-center text-sm text-gray before:flex-[1_1_0%]">
                or
            </div>

            <x-secondary-button class="w-full flex justify-center gap-2" wire:click="set_step">
                <x-icon.google class="h-5 w-5"></x-icon.google>
                Continue with Google
            </x-secondary-button>
        </div>

        <div class="text-center">
            <p class="mb-0 mt-6 text-sm text-gray">
                By continuing you agree to our
                <a target="_blank"
                   class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                   href="/terms">
                    Terms of Service
                </a>
                and
                <a target="_blank"
                   class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                   href="/privacy">
                    Privacy Policy.
                </a>
            </p>
        </div>
    </div>
</div>
