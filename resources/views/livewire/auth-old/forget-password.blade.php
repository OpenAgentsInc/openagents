<div>
    <div class="flex justify-end items-center  pb-4">
        <button @click="open = false" class="text-gray-500 hover:text-gray-700 focus:outline-none">
            <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none"
                 stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
                 class="feather feather-x">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
        </button>
    </div>

    <div class="">
        <h2 class="block text-md md:text-xl lg:text-2xl text-center font-bold text-white">{{$this->show ? 'Reset password' : 'Forgot Password'}}</h2>
    </div>

    @if(!$this->show)
        <div class="p-4 sm:p-7">
            <div class="mb-4">
                <x-input id="forget-password-email" class="block mt-1 w-full" type="email" name="email"
                         :value="old('email')" required autofocus autocomplete="username" placeholder="email"/>
            </div>
            <div class="my-5">
                <x-button class="w-full flex justify-center gap-2 " wire:click='sendResetLink()'>
                    Reset account
                </x-button>
            </div>

            <div class="text-center">
                <p class="mt-4 text-sm text-gray">
                    You will receive a password reset link if you have an
                    <a href="#"
                       class="text-white decoration-2 hover:underline font-medium dark:focus:outline-none dark:focus:ring-1 dark:focus:ring-gray-600">
                        account
                    </a>
                    with us.
                </p>
            </div>
        </div>
    @elseif ($this->show)
        <div class="p-4 sm:p-7">
            <div class="text-center">
                <p class="mt-2 text-sm md:text-md text-gray">
                    We sent a reset link to satoshi@nakamoto.com.
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
