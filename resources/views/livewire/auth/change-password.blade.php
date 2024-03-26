<div>
    {{-- Success is as dangerous as failure. --}}
    @livewire('navbar')
    <div class="flex items-center justify-end w-full h-screen">
        <div class="border border-[#3C3E42] bg-black rounded-lg p-6  max-w-lg h-auto mx-auto  shadow-lg transform transition-all duration-300">
            <!-- Modal Header -->

            <div class="">
                <h2 class="block text-xl text-center font-bold text-gray-200">Reset Password</h2>
            </div>

            <div class="p-4 sm:p-7">

                <div class="mb-4 flex justify-center">
                    <span class="mb-4 inline-flex justify-center items-center  rounded-full">
                        <x-icon.padlock class="w-[100px] h-[100px]">
                        </x-icon.padlock>
                    </span>
                </div>



                <div class="mb-4">
                    <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus placeholder="New Password" />
                </div>

                <div class="mb-4">
                    <x-input id="password" class="block mt-1 w-full" type="password" name="password" required autofocus placeholder="New Password" />
                </div>


                <div class="mt-5">

                    <x-button class="w-full flex justify-center gap-2 hover:bg-gray">
                        <x-icon.agent class="h-5 w-5 text-white"></x-icon.agent>
                        Change Password
                    </x-button>



                </div>


            </div>





        </div>
    </div>
</div>
