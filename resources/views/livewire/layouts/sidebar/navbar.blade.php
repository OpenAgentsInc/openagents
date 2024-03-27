<div>
    {{-- Close your eyes. Count to one. That is how long forever feels. --}}
    <!-- ========== HEADER ========== -->
    <header x-data="{ isOpen: false }" class=" flex flex-wrap sm:justify-start sm:flex-nowrap z-[48] w-full  text-sm py-2.5 sm:py-4 bg-black relative">
        <nav class="flex basis-full items-center w-full mx-auto px-4 sm:px-6 md:px-8" aria-label="Global">
            <div class="me-5 lg:me-0 lg:hidden">
                <button type="button" @click="sidebarOpen = !sidebarOpen" class="px-4 border-r border-gray-200 text-white focus:outline-none focus:ring-2 focus:ring-inset focus:ring-[] lg:hidden">
                    <span class="sr-only">Open sidebar</span>
                    <svg class="h-6 w-6" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke="currentColor" aria-hidden="true">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h7" />
                    </svg>
                </button>
            </div>

            <div class="w-full justify-end flex items-center  ms-auto md:justify-between sm:gap-x-3 sm:order-3">


                <div class="items-center py-4 hidden md:flex">

                    <!-- Breadcrumb -->
                    <ol class="ms-3 flex items-center whitespace-nowrap" aria-label="Breadcrumb">
                        <li class="flex items-center text-sm text-gray hover:text-white">
                            <x-icon.logo class="w-8 h-8"></x-icon.logo>
                            <svg class="flex-shrink-0 mx-3 overflow-visible size-2.5 text-gray-400 " width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                                <path d="M5 1L10.6869 7.16086C10.8637 7.35239 10.8637 7.64761 10.6869 7.83914L5 14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path>
                            </svg>
                        </li>
                        <li class="text-sm font-semibold text-gray-800 truncate " x-data="{ dropdown: false }" aria-current="page">
                            <div class="flex gap-2 items-center justify-center overflow-hidden" role="button" tabindex="0" @click="dropdown= !dropdown">
                                <x-icon.chatgpt class="w-8 h-8"></x-icon.chatgpt>
                                <div class="flex flex-col">
                                    <span class="text-indigo-50 my-0 text-sm">ChatGPT </span>

                                </div>
                                <div class="relative flex-1 text-right bg-black">
                                    <div>
                                        <button class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                                            {{-- <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="w-5 h-5">
                                                <path fill-rule="evenodd" d="M10 3a.75.75 0 01.55.24l3.25 3.5a.75.75 0 11-1.1 1.02L10 4.852 7.3 7.76a.75.75 0 01-1.1-1.02l3.25-3.5A.75.75 0 0110 3zm-3.76 9.2a.75.75 0 011.06.04l2.7 2.908 2.7-2.908a.75.75 0 111.1 1.02l-3.25 3.5a.75.75 0 01-1.1 0l-3.25-3.5a.75.75 0 01.04-1.06z" clip-rule="evenodd" />
                                            </svg> --}}
                                            <x-icon.expand-down class="w-5 h-5"></x-icon.expand-down>
                                        </button>

                                        <div x-show="dropdown" @click.away="dropdown= false" class="fixed z-[90000] divide-y divide-white/15   min-w-60  shadow-md rounded-lg p-2 bg-black border border-white/45" aria-labelledby="hs-dropdown-with-header">

                                            <div class=" py-0 first:pt-0 last:pb-0 bg-black">
                                                <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                    <x-icon.chatgpt class="w-8 h-8"></x-icon.chatgpt>
                                                    <div class="flex flex-col">
                                                        <span class="text-indigo-50 my-0 text-sm">ChatGPT </span>

                                                    </div>
                                                </a>

                                                <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                    <x-icon.claude class="w-8 h-8"></x-icon.claude>
                                                    <div class="flex flex-col">
                                                        <span class="text-indigo-50 my-0 text-sm">Claude </span>

                                                    </div>
                                                </a>

                                                <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                    <x-icon.gemini class="w-8 h-8"></x-icon.gemini>
                                                    <div class="flex flex-col">
                                                        <span class="text-indigo-50 my-0 text-sm">Gemini </span>

                                                    </div>
                                                </a>

                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </li>
                    </ol>
                    <!-- End Breadcrumb -->
                </div>

                <div class="flex flex-row items-center justify-end gap-2">
                    {{-- <button type="button" class="w-[2.375rem] h-[2.375rem] inline-flex justify-center items-center gap-x-2 text-sm font-semibold rounded-full border border-transparent text-gray-800 hover:bg-gray-100 disabled:opacity-50 disabled:pointer-events-none ">
                        <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
                            <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" /></svg>
                    </button>
                    <button type="button" class="w-[2.375rem] h-[2.375rem] inline-flex justify-center items-center gap-x-2 text-sm font-semibold rounded-full border border-transparent text-gray-800 hover:bg-gray-100 disabled:opacity-50 disabled:pointer-events-none " data-hs-offcanvas="#hs-offcanvas-right">
                        <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                            <path d="M22 12h-4l-3 9L9 3l-3 9H2" /></svg>
                    </button> --}}

                    <div class="hs-dropdown relative inline-flex [--placement:bottom-right]">


                        {{-- <div x-data="{ showModal: true }">
                                <!-- Trigger button -->
                                <button @click="showModal = true" class="bg-blue-500 text-white px-4 py-2 rounded-lg hover:bg-blue-600 transition duration-300">
                                    Create New Project
                                </button>

                                <!-- Modal -->
                                <div x-show="showModal" class="fixed inset-0 flex items-center justify-center z-50">
                                    <div class="bg-white rounded-lg p-6 w-96 max-w-full shadow-lg transform transition-all duration-300" x-show.transition.opacity="showModal">
                                        <!-- Modal Header -->
                                        <div class="flex justify-between items-center border-b-2 border-gray-200 pb-4">
                                            <h2 class="text-2xl font-semibold">Create or Import Project</h2>
                                            <button @click="showModal = false" class="text-gray-500 hover:text-gray-700 focus:outline-none">
                                                <svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="feather feather-x">
                                                    <line x1="18" y1="6" x2="6" y2="18"></line>
                                                    <line x1="6" y1="6" x2="18" y2="18"></line>
                                                </svg>
                                            </button>
                                        </div>

                                        <!-- Modal Content -->
                                        <div class="mt-6 space-y-4">
                                            <p class="text-lg text-gray-600">Choose how you want to create or import a project:</p>
                                            <div class="flex flex-col space-y-4">
                                                <button class="bg-blue-500 text-white px-4 py-2 rounded-lg hover:bg-blue-600 transition duration-300">Create New Project</button>
                                                <button class="flex items-center justify-center gap-2 bg-gray-900 text-gray-100 px-4 py-2 rounded-lg hover:bg-black transition duration-300">
                                                  <img src="https://svgur.com/i/yp2.svg" alt="Github Icon">
                                                  Import from GitHub
                                                  </button>
                                            </div>
                                        </div>

                                        <!-- Additional Information -->
                                        <div class="mt-6 text-sm text-gray-500">
                                            <p>Create a new project from scratch or import an existing project from your GitHub repository.</p>
                                        </div>
                                    </div>
                                </div>
                            </div> --}}




                        <x-secondary-button x-data @click="$dispatch('open-login-modal')" id="hs-dropdown-with-header" type="button" class=" inline-flex justify-center items-center gap-x-2 text-sm font-semibold rounded-full  text-gray-800 hover:bg-gray-100 disabled:opacity-50 disabled:pointer-events-none ">
                            Sign In
                        </x-secondary-button>









                        <div class="hs-dropdown-menu transition-[opacity,margin] duration hs-dropdown-open:opacity-100 opacity-0 hidden min-w-60 bg-white shadow-md rounded-lg p-2 " aria-labelledby="hs-dropdown-with-header">
                            <div class="py-3 px-5 -m-2  rounded-t-lg bg-gray-700">
                                <p class="text-sm text-gray-500 ">Signed in as</p>
                                <p class="text-sm font-medium text-gray-800 dark:text-gray-300">james@site.com</p>
                            </div>
                            <div class="mt-2 py-2 first:pt-0 last:pb-0">
                                <a class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm text-gray-400  focus:ring-2 focus:ring-gray-500 hover:bg-gray-700 hover:text-gray-300" href="#">
                                    <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
                                        <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" /></svg>
                                    Newsletter
                                </a>
                                <a class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm text-gray-400  focus:ring-2 focus:ring-gray-500  hover:bg-gray-700 hover:text-gray-300" href="#">
                                    <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M6 2 3 6v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V6l-3-4Z" />
                                        <path d="M3 6h18" />
                                        <path d="M16 10a4 4 0 0 1-8 0" /></svg>
                                    Purchases
                                </a>
                                <a class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm text-gray-400 focus:ring-2 focus:ring-gray-500 hover:bg-gray-700 hover:text-gray-300" href="#">
                                    <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M4 14.899A7 7 0 1 1 15.71 8h1.79a4.5 4.5 0 0 1 2.5 8.242" />
                                        <path d="M12 12v9" />
                                        <path d="m8 17 4 4 4-4" /></svg>
                                    Downloads
                                </a>
                                <a class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm text-gray-400 focus:ring-2 focus:ring-gray-500  hover:bg-gray-700 hover:text-gray-300" href="#">
                                    <svg class="flex-shrink-0 size-4" xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                        <path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" />
                                        <circle cx="9" cy="7" r="4" />
                                        <path d="M22 21v-2a4 4 0 0 0-3-3.87" />
                                        <path d="M16 3.13a4 4 0 0 1 0 7.75" /></svg>
                                    Team Account
                                </a>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </nav>
    </header>
    <!-- ========== END HEADER ========== -->


    <div class="sticky top-0 inset-x-0  border-y border-[#1B1B1B] px-4 sm:px-6 md:px-8 md:hidden">
        <div class="flex items-center py-4">


            <!-- Breadcrumb -->
            <!-- Breadcrumb -->
            <ol class="ms-3 flex items-center whitespace-nowrap" aria-label="Breadcrumb">
                <li class="flex items-center text-sm text-gray hover:text-white">
                    <x-icon.logo class="w-8 h-8"></x-icon.logo>
                    <svg class="flex-shrink-0 mx-3 overflow-visible size-2.5 text-gray-400 " width="16" height="16" viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                        <path d="M5 1L10.6869 7.16086C10.8637 7.35239 10.8637 7.64761 10.6869 7.83914L5 14" stroke="currentColor" stroke-width="2" stroke-linecap="round"></path>
                    </svg>
                </li>
                <li class="text-sm font-semibold text-gray-800 truncate " x-data="{ dropdown: false }" aria-current="page">
                    <div class="flex gap-2 items-center justify-center overflow-hidden" role="button" tabindex="0" @click="dropdown= !dropdown">
                        <x-icon.chatgpt class="w-8 h-8"></x-icon.chatgpt>

                        <div class="flex flex-col">
                            <span class="text-indigo-50 my-0 text-sm">ChatGPT </span>

                        </div>
                        <div class="relative flex-1 text-right bg-black">
                            <div>
                                <button class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                                    {{-- <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="w-5 h-5">
                                    <path fill-rule="evenodd" d="M10 3a.75.75 0 01.55.24l3.25 3.5a.75.75 0 11-1.1 1.02L10 4.852 7.3 7.76a.75.75 0 01-1.1-1.02l3.25-3.5A.75.75 0 0110 3zm-3.76 9.2a.75.75 0 011.06.04l2.7 2.908 2.7-2.908a.75.75 0 111.1 1.02l-3.25 3.5a.75.75 0 01-1.1 0l-3.25-3.5a.75.75 0 01.04-1.06z" clip-rule="evenodd" />
                                </svg> --}}
                                    <x-icon.expand-down class="w-5 h-5"></x-icon.expand-down>
                                </button>

                                <div x-show="dropdown" @click.away="dropdown= false" class="fixed z-[90000] divide-y divide-white/15   min-w-60  shadow-md rounded-lg p-2 bg-black border border-white/45" aria-labelledby="hs-dropdown-with-header">

                                    <div class=" py-0 first:pt-0 last:pb-0 bg-black">
                                        <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                            <x-icon.chatgpt class="w-8 h-8"></x-icon.chatgpt>
                                            <div class="flex flex-col">
                                                <span class="text-indigo-50 my-0 text-sm">ChatGPT </span>

                                            </div>
                                        </a>

                                        <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                            <x-icon.claude class="w-8 h-8"></x-icon.claude>
                                            <div class="flex flex-col">
                                                <span class="text-indigo-50 my-0 text-sm">Claude </span>

                                            </div>
                                        </a>

                                        <a href="#" class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                            <x-icon.gemini class="w-8 h-8"></x-icon.gemini>
                                            <div class="flex flex-col">
                                                <span class="text-indigo-50 my-0 text-sm">Gemini </span>

                                            </div>
                                        </a>

                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>
                </li>
            </ol>
            <!-- End Breadcrumb -->
            <!-- End Breadcrumb -->
        </div>
    </div>


    @section('modal')


    @endsection

</div>
