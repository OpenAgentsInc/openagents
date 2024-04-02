<div>

    @if(!$this->verification)
        <div class="py-4">
            <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Create Password' : 'Get started'}}</h2>
        </div>

    @else
        <div class="py-4 mt-2">
            <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">Verify your email</h2>
        </div>
    @endif

    @if(!$this->show)
    {{-- Step 1 input email --}}
        <div class="p-4 sm:p-7 mb-2">
            <div class="mb-4">
                <x-input id="register-email" class="block mt-1 w-full" type="email" name="email" :value="old('email')"
                         required autofocus autocomplete="username" placeholder="email"/>
            </div>

            <div class="mt-5 mb-3">
                <x-button class="w-full flex justify-center gap-2 text-center" wire:click='showpassword()'>
                    Get Started
                </x-button>

                <div class="py-3 flex items-center text-xs text-[#777A82] uppercase before:flex-[1_1_0%]  before:me-6 after:flex-[1_1_0%] after:ms-6 dark:text-gray-500 dark:before:border-gray-600 dark:after:border-gray-600">
                    Or
                </div>

                <x-secondary-button class="w-full flex justify-center gap-2 my-2">
                    <x-icon.google class="h-5 w-5"></x-icon.google>
                    <span class="text-sm md:text-md">Continue with Google</span>
                </x-secondary-button>
            </div>

            <div class="text-center">
                <p class="mt-2 text-sm text-gray">
                    By clicking either button above, you agree to our
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Terms of Service
                    </a>
                    and
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Privacy Policy.
                    </a>
                </p>
            </div>
        </div>
    @elseif ($this->show && !$this->verification)
    {{-- Step 2 Input password --}}
        <div class="p-2 sm:p-7">
            <div class="mb-4">
                <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus
                         placeholder="EnterPassword"/>
            </div>

            <div class="mb-4">
                <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus
                         placeholder="Confirm Password"/>
            </div>

            <div class="mt-5">
                <x-button class="w-full flex justify-center text-center gap-2" wire:click='set_verified()'>
                    Create Password
                </x-button>
            </div>
        </div>

    @elseif ($this->show && $this->verification)
    {{-- Send Verification --}}
        <div class="p-4 sm:p-7">
            <div class="text-center">
                <p class="mt-2 text-sm md:text-md text-gray">
                    We sent a verification link to <span class="text-white">satoshi@nakamoto.com</span>
                </p>
                <p>
                    <a class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600"
                       href="#">
                        Resend Email
                    </a>
                </p>
            </div>
        </div>
    @endif
</div>
