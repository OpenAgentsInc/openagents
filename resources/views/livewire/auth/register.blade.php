<div>
    {{-- In work, do what you enjoy. --}}

    @if($this->step == 1)
    <div class="">
        <h2 class="block text-xl text-center font-bold text-gray-800 dark:text-gray-200">Get Started</h2>
    </div>

    <div class="p-4 sm:p-7">

        <div class="mb-4">
            <x-input id="email" class="block mt-1 w-full" type="email" name="email" :value="old('email')" required autofocus autocomplete="username" placeholder="email" />
        </div>


        <div class="mt-5">
            {{-- <a class="w-full py-3 px-4 inline-flex justify-center items-center gap-x-2 text-sm font-medium rounded-lg border border-gray-200 bg-white text-gray-800 shadow-sm hover:bg-gray-50 disabled:opacity-50 disabled:pointer-events-none dark:bg-slate-900 dark:border-gray-700 dark:text-white dark:hover:bg-gray-800 dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">

            </a> --}}
            <x-button class="w-full flex justify-center gap-2 hover:bg-gray">
                <x-icon.agent class="h-5 w-5 text-black"></x-icon.agent>
                Get Started
            </x-button>

            <div class="py-3 flex items-center text-xs text-gray-400 uppercase before:flex-[1_1_0%] before:border-t before:border-gray-200 before:me-6 after:flex-[1_1_0%] after:border-t after:border-gray-200 after:ms-6 dark:text-gray-500 dark:before:border-gray-600 dark:after:border-gray-600">Or</div>


            <x-secondary-button class="w-full flex justify-center gap-2" wire:click="set_step">
                <x-icon.google class="h-5 w-5"></x-icon.google>
                Continue with Google
            </x-secondary-button>

        </div>

        <div class="text-center">
            <p class="mt-2 text-sm text-gray">
                By clicking either button above, you agree to our
                <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">
                    Terms of Service
                </a>
                and
                <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600" href="#">
                    Privacy Policy.
                </a>

            </p>
        </div>
    </div>
    @elseif ($this->step == 2)


    <div class="">
        <h2 class="block text-xl text-center font-bold text-gray-200">Create Password</h2>
    </div>

    <div class="p-4 sm:p-7">

        <div class="mb-4 flex justify-center">
            <span class="mb-4 inline-flex justify-center items-center  rounded-full">
                <x-icon.padlock class="w-[100px] h-[100px]">
                </x-icon.padlock>
            </span>
        </div>



        <div class="mb-4">
            <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus placeholder="EnterPassword" />
        </div>

        <div class="mb-4">
            <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus placeholder="Confirm Password" />
        </div>


        <div class="mt-5">

            <x-button class="w-full flex justify-center gap-2 hover:bg-gray">
                <x-icon.agent class="h-5 w-5 text-white"></x-icon.agent>
                Create Password
            </x-button>



        </div>


    </div>


    @endif

</div>
