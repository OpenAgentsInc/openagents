<!DOCTYPE html>
<html lang="{{ str_replace('_', '-', app()->getLocale()) }}">

<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{{ $title ?? 'OpenAgents' }}</title>
    @stack('scripts')
    @include('partials.vite')
    @include('partials.analytics')
    @include('partials.ogtags')
</head>

<body class="h-full bg-black" x-data="{ sidebarOpen: false, showSidebar: true, collapsed: false }">
<div class="h-full">
    <!-- Off-canvas menu for mobile -->
    <div x-show="sidebarOpen" class="fixed inset-0 flex z-40 lg:hidden" role="dialog" aria-modal="true" x-bind:class="{
            'w-16': collapsed,
            '-translate-x-full': !showSidebar
           }">
        <!-- Off-canvas menu overlay -->
        <div class="fixed inset-0 bg-white/15 bg-opacity-75" x-bind:class="{

                'w-[300px]': !collapsed,
                'w-16': collapsed,
                '-translate-x-full': !showSidebar
               }" aria-hidden="true"></div>

        <!-- Off-canvas menu content -->
        <div class="relative flex-1 flex flex-col max-w-xs w-full pt-5 pb-4 bg-black border-r-2 border-[#1B1B1B]"
             x-bind:class="{

                'w-[300px]': !collapsed,
                'w-16': collapsed,
                '-translate-x-full': !showSidebar
               }" aria-hidden="true">
            <!-- Close button -->
            <div class="absolute top-0 right-0 -mr-12 pt-2 z-50">
                <button type="button" @click="sidebarOpen = false"
                        class="ml-1 flex items-center justify-center h-10 w-10 rounded-full focus:outline-none focus:ring-2 focus:ring-inset focus:ring-white">
                    <span class="sr-only">Close sidebar</span>
                    <svg class="h-6 w-6 text-white" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24"
                         stroke="currentColor" aria-hidden="true">
                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/>
                    </svg>
                </button>
            </div>

            <!-- Off-canvas menu items -->

            <!-- Include your menu items here -->
            <div class="h-full flex flex-col  lg:inset-y-0 z-40">

                <!-- Include your sidebar content here -->
                <div x-bind:class="{
                      'bg-black  text-zinc-50 fixed h-screen z-100': true,
                      'w-[300px]': !collapsed,
                      'w-16': collapsed,
                      '-translate-x-full': !showSidebar
                     }">
                    <div x-bind:class="{
                          'flex flex-col justify-between h-screen lg:h-full sticky inset-0': true,
                          'p-4 justify-between': !collapsed,
                          'py-4 justify-center': collapsed
                         }">
                        <div x-bind:class="{
                              'flex items-center  transition-none': true,
                              'p-4 justify-between': !collapsed,
                              'py-4 justify-center': collapsed
                             }">
                            <div class="w-full">
                                <div class="flex gap-2 items-center justify-center overflow-hidden" role="button">
                                    <img src="https://images.unsplash.com/photo-1472099645785-5658abf4ff4e?ixlib=rb-1.2.1&ixid=eyJhcHBfaWQiOjEyMDd9&auto=format&fit=facearea&facepad=2&w=256&h=256&q=80"
                                         height="36" width="36" alt="profile image" class="rounded-full">
                                    <div x-show="!collapsed" class="flex flex-col">
                                        <span class="text-indigo-50 my-0 text-sm">Tom Cook</span>
                                        <!--                 <a href="/" class="text-indigo-200 text-sm">View Profile</a> -->

                                    </div>
                                    <div x-show="!collapsed" class="relative flex-1 text-right">
                                        <div x-data="{ dropdown: false }">
                                            <button @click="dropdown= !dropdown"
                                                    class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"
                                                     fill="currentColor" class="w-5 h-5">
                                                    <path fill-rule="evenodd"
                                                          d="M10 3a.75.75 0 01.55.24l3.25 3.5a.75.75 0 11-1.1 1.02L10 4.852 7.3 7.76a.75.75 0 01-1.1-1.02l3.25-3.5A.75.75 0 0110 3zm-3.76 9.2a.75.75 0 011.06.04l2.7 2.908 2.7-2.908a.75.75 0 111.1 1.02l-3.25 3.5a.75.75 0 01-1.1 0l-3.25-3.5a.75.75 0 01.04-1.06z"
                                                          clip-rule="evenodd"/>
                                                </svg>
                                            </button>

                                            <div x-show="dropdown" @click.away="dropdown= false"
                                                 class="fixed z-100 divide-y divide-white/15 transition-[opacity,margin] duration  min-w-60  shadow-md rounded-lg p-2 bg-black border border-white/45"
                                                 aria-labelledby="hs-dropdown-with-header">
                                                <div class="py-2 flex px-5 -m-2 bg-gray-100 rounded-t-lg text-sm gap-1 hover:bg-white/15 text-gray-300 hover:text-white">
                                                    {{-- <p class="text-xs ">Signed in as</p> --}}
                                                    <p class="text-sm font-medium">james@site.com</p>
                                                </div>
                                                <div class=" py-0 first:pt-0 last:pb-0">
                                                    <a href="#"
                                                       class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                        <!-- Newsletter SVG Icon -->
                                                        <span>Newsletter</span>
                                                    </a>
                                                    <a href="#"
                                                       class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                        <!-- Purchases SVG Icon -->
                                                        <span>Purchases</span>
                                                    </a>
                                                    <a href="#"
                                                       class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                        <!-- Downloads SVG Icon -->
                                                        <span>Downloads</span>
                                                    </a>
                                                    <a href="#"
                                                       class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                        <!-- Team Account SVG Icon -->
                                                        <span>Team Account</span>
                                                    </a>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>

                        </div>
                        <nav class="flex-grow w-full">


                            {{ $sidecontent_mobile ?? '' }}


                        </nav>

                        <div x-bind:class="{
                            'grid place-content-stretch p-4 ': true,
                            'justify-end': !collapsed,
                            'justify-center': collapsed
                              }">
                            <button @click="collapsed = !collapsed"
                                    class="flex hover:bg-[#1B1B1B] w-10 h-10 rounded-full items-center justify-center opacity-100">
                                    <span x-show="collapsed">
                                        <x-icon.session-right/> </span>
                                <span x-show="!collapsed">
                                        <x-icon.session-left/> </span>
                            </button>
                        </div>

                    </div>
                </div>

            </div>
            <!-- end menuitems -->


        </div>

        <!-- Dummy element to force sidebar to shrink -->
        <div class="flex-shrink-0 w-14" aria-hidden="true"></div>
    </div>

    <!-- Static sidebar for desktop -->
    <div class=" h-full hidden lg:flex lg:w-64 lg:flex-col lg:fixed lg:inset-y-0 z-40">

        <!-- Include your sidebar content here -->
        <div x-show="showSidebar" x-bind:class="{
              'bg-black border-r-4 border-[#1B1B1B] text-zinc-50 fixed h-screen lg:static z-20': true,
              'w-[300px]': !collapsed,
              'w-16': collapsed,
              '-translate-x-full': !showSidebar
             }">
            <div x-bind:class="{
                  'flex flex-col justify-between h-screen lg:h-full sticky inset-0': true,
                  'p-4 justify-between': !collapsed,
                  'py-4 justify-center': collapsed
                 }">
                <div x-bind:class="{
                      'flex items-center  transition-none': true,
                      'p-4 justify-between': !collapsed,
                      'py-4 justify-center': collapsed
                     }">
                    <div class="w-full">
                        <div class="flex gap-2 items-center justify-center overflow-hidden">
                            <img src="https://images.unsplash.com/photo-1472099645785-5658abf4ff4e?ixlib=rb-1.2.1&ixid=eyJhcHBfaWQiOjEyMDd9&auto=format&fit=facearea&facepad=2&w=256&h=256&q=80"
                                 height="36" width="36" alt="profile image" class="rounded-full">
                            <div x-show="!collapsed" class="flex flex-col">
                                <span class="text-indigo-50 my-0 text-sm">Tom Cook</span>
                                <!--                 <a href="/" class="text-indigo-200 text-sm">View Profile</a> -->

                            </div>
                            <div x-show="!collapsed" class="relative flex-1 text-right">
                                <div x-data="{ dropdown: false }">
                                    <button @click="dropdown= !dropdown"
                                            class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100">
                                        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor"
                                             class="w-5 h-5">
                                            <path fill-rule="evenodd"
                                                  d="M10 3a.75.75 0 01.55.24l3.25 3.5a.75.75 0 11-1.1 1.02L10 4.852 7.3 7.76a.75.75 0 01-1.1-1.02l3.25-3.5A.75.75 0 0110 3zm-3.76 9.2a.75.75 0 011.06.04l2.7 2.908 2.7-2.908a.75.75 0 111.1 1.02l-3.25 3.5a.75.75 0 01-1.1 0l-3.25-3.5a.75.75 0 01.04-1.06z"
                                                  clip-rule="evenodd"/>
                                        </svg>
                                    </button>

                                    <div x-show="dropdown" @click.away="dropdown= false"
                                         class="fixed z-100 divide-y divide-white/15 transition-[opacity,margin] duration  min-w-60  shadow-md rounded-lg p-2 bg-black border border-white/45"
                                         aria-labelledby="hs-dropdown-with-header">
                                        <div class="py-2 flex px-5 -m-2 bg-gray-100 rounded-t-lg text-sm gap-1 hover:bg-white/15 text-gray-300 hover:text-white">
                                            {{-- <p class="text-xs ">Signed in as</p> --}}
                                            <p class="text-sm font-medium">james@site.com</p>
                                        </div>
                                        <div class=" py-0 first:pt-0 last:pb-0">
                                            <a href="#"
                                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                <!-- Newsletter SVG Icon -->
                                                <span>Newsletter</span>
                                            </a>
                                            <a href="#"
                                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                <!-- Purchases SVG Icon -->
                                                <span>Purchases</span>
                                            </a>
                                            <a href="#"
                                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                <!-- Downloads SVG Icon -->
                                                <span>Downloads</span>
                                            </a>
                                            <a href="#"
                                               class="flex items-center gap-x-3.5 py-2 px-3 rounded-lg text-sm  focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-gray-200">
                                                <!-- Team Account SVG Icon -->
                                                <span>Team Account</span>
                                            </a>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </div>
                    </div>

                </div>
                <nav class="flex-grow w-full">


                    {{ $sidecontent ?? '' }}


                </nav>

                <div x-bind:class="{
                    'grid place-content-stretch p-4 ': true,
                    'justify-end': !collapsed,
                    'justify-center': collapsed
                      }">
                    <button @click="collapsed = !collapsed"
                            class="flex hover:bg-[#1B1B1B] w-10 h-10 rounded-full items-center justify-center opacity-0 lg:opacity-100">
                            <span x-show="collapsed">
                                <x-icon.session-right/> </span>
                        <span x-show="!collapsed">
                                <x-icon.session-left/> </span>
                    </button>
                </div>

            </div>
        </div>

    </div>

    <!-- Navbar -->
    <div class=" flex flex-col  z-100">
        <!-- Navbar content -->
        <div class="fixed top-0 inset-x-0 z-48 flex-shrink-0 flx h-auto bg-black shadow"
             x-bind:class="{
              'lg:pl-[300px]': !collapsed,
              'lg:pl-16': collapsed,
              '-translate-x-full': !showSidebar
           }">

            <!-- Include your navbar content here -->
            @livewire('layouts.sidebar.navbar')
        </div>

        <!-- Main content area -->
        <main class="flex-1">
            <!-- Include your main content here -->

            <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8" x-bind:class="{
                    'lg:pl-[0px]': !collapsed,
                    'lg:pl-16': collapsed,
                    '-translate-x-full': !showSidebar
                 }">
                <!-- Replace this with your actual content -->
                {{$slot}}
                <!-- /End replace -->
            </div>
        </main>
    </div>
</div>

{{-- Modal Pop up here --}}


@include('partials.modals')



@yield('modal')

{{-- End Modal Popup --}}
</body>

@include('partials.twitter')
</html>
