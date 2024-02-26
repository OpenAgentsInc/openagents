<x-apidoc-layout>
    <livewire:navbar />

    <div class="max-w-8xl mx-auto px-4 sm:px-6 md:px-8">
        <div
            class="z-20 hidden lg:block fixed bottom-0 right-auto w-[18rem] pl-4 pr-6 pb-10 overflow-y-auto stable-scrollbar-gutter top-[85px]">
            <div class="relative lg:text-sm lg:leading-6">
                <ul>
                    <div class="mt-12 lg:mt-8">
                        <h5 class="mb-3.5 lg:mb-2.5 font-semibold text-gray">Agents API
                        </h5>
                        <li>
                            <a class="group mt-2 lg:mt-0 flex items-center -ml-4 py-1.5 rounded-lg focus:outline-primary dark:focus:outline-primary-light hover:bg-gray-600/5 dark:hover:bg-gray-200/5 text-gray-700 hover:text-gray-900 dark:text-gray-400 dark:hover:text-gray-300"
                                style="padding-left:1rem" href="/docs/api/agents/create"><span
                                    class="w-11 flex items-center"><span
                                        class="px-1 py-0.5 mr-2 rounded-[0.3rem] text-[0.55rem] leading-tight font-bold bg-blue-400/20 text-blue-700 dark:bg-blue-400/20 dark:text-blue-400">POST</span></span>
                                <div class="flex-1 flex items-center space-x-2.5">
                                    <div>Create Agent</div>
                                </div>
                            </a>
                        </li>
                        <li>
                            <a class="group mt-2 lg:mt-0 flex items-center -ml-4 py-1.5 rounded-lg focus:outline-primary dark:focus:outline-primary-light bg-primary/10 text-primary font-semibold dark:text-primary-light dark:bg-primary-light/10"
                                style="padding-left:1rem" href="/docs/api/agents/get"><span
                                    class="w-11 flex items-center"><span
                                        class="px-1 py-0.5 mr-2 rounded-[0.3rem] text-[0.55rem] leading-tight font-bold bg-[#2AB673] text-white">GET</span></span>
                                <div class="flex-1 flex items-center space-x-2.5">
                                    <div>Get Agents</div>
                                </div>
                            </a>
                        </li>
                        <li>
                            <a class="group mt-2 lg:mt-0 flex items-center -ml-4 py-1.5 rounded-lg focus:outline-primary dark:focus:outline-primary-light hover:bg-gray-600/5 dark:hover:bg-gray-200/5 text-gray-700 hover:text-gray-900 dark:text-gray-400 dark:hover:text-gray-300"
                                style="padding-left:1rem" href="/docs/api/agents/delete"><span
                                    class="w-11 flex items-center"><span
                                        class="px-1 py-0.5 mr-2 rounded-[0.3rem] text-[0.55rem] leading-tight font-bold bg-red-400/20 text-red-700 dark:bg-red-400/20 dark:text-red-400">DEL</span></span>
                                <div class="flex-1 flex items-center space-x-2.5">
                                    <div>Delete Agent</div>
                                </div>
                            </a>
                        </li>
                    </div>
                </ul>
            </div>
        </div>
        <div class="lg:pl-[20rem]">
            <div class="flex flex-row items-stretch gap-12 pt-[6.5rem]">
                <div class="relative grow mx-auto px-1 overflow-hidden xl:-ml-12 xl:pl-14">
                    <header id="header" class="relative">
                        <div class="mt-0.5 space-y-2.5">
                            <div class="flex items-center">
                                <h1
                                    class="inline-block text-2xl sm:text-3xl font-extrabold text-gray-900 tracking-tight dark:text-gray-200">
                                    Create Agent</h1>
                            </div>
                        </div>
                        <div class="mt-2 text-lg prose prose-gray text-gray">Create an agent.</div>
                    </header>
                    <div class="mt-12">
                        <div class="flex items-baseline border-b pb-2.5 border-darkgray">
                            <h4 class="flex-1 mb-0">Query Parameters</h4>
                            <div class="flex items-center"></div>
                        </div>

                        <x-markdown>

                            POST https://openagents.com/api/v1/agents

                            ## Request parameters
                            * name (string, required): The name of the agent.
                            * description (string, required): A brief description of the agent's purpose.
                            * instructions (string, required): Detailed instructions on how the agent operates.
                            * welcome_message (string, required): A message that users will see when they start
                            interacting with the agent.

                            ### Request example


                            ```shell
                            curl https://openagents.com/api/v1/agents \
                            -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
                            -H 'Content-Type: application/json' \
                            -d '{
                            "name": "Data Visualizer",
                            "description": "Analyzes .csv files and creates data visualizations.",
                            "instructions": "Upload a .csv file to begin.",
                            "welcome_message": "Welcome to Data Visualizer! Please upload a .csv file."
                            }'
                            ```
                        </x-markdown>


                        <div class="py-6 border-gray-100 dark:border-darkgray border-b last:border-b-0">
                            <div class="flex font-mono text-sm">
                                <div class="flex-1 flex content-start py-0.5 mr-5">
                                    <div class="flex items-center flex-wrap gap-2">
                                        <div class="font-bold text-primary dark:text-primary-light"><span
                                                class="text-gray-500 dark:text-gray-400"></span>limit</div>
                                        <div class="flex items-center space-x-2 text-xs font-medium">
                                            <div
                                                class="px-2 py-0.5 rounded-md bg-gray-100/50 dark:bg-white/5 text-gray-600 dark:text-gray-200">
                                                integer</div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                            <div class="mt-4">
                                <div class="prose prose-sm prose-gray dark:prose-invert">
                                    <p node="[object Object]" class="m-0">The maximum number of results to return</p>
                                </div>
                            </div>
                        </div>
                    </div>
                </div>
                <div class="hidden xl:flex self-start h-auto sticky top-32">
                    <div class="gap-8 flex flex-col pb-6 w-[28rem] gap-6">
                        <div
                            class="not-prose bg-[#0F1117] dark:bg-codeblock rounded-xl dark:ring-1 dark:ring-gray-800/50 relative">
                            <div class="flex text-xs bg-black/40 leading-6 rounded-t-xl border-b border-darkgray"
                                role="tablist" aria-orientation="horizontal">
                                <div class="flex overflow-x-auto">
                                    <div class="group flex items-center relative px-2 pt-2.5 pb-2 text-gray-400 outline-none whitespace-nowrap font-medium"
                                        id="headlessui-tabs-tab-:rl:" role="tab" type="button" aria-selected="false"
                                        tabindex="-1" data-headlessui-state=""
                                        aria-controls="headlessui-tabs-panel-:rr:">
                                        <div
                                            class="px-2 rounded-md group-hover:bg-gray-700/60 group-hover:text-primary-light">
                                            <div class="z-10">Example request</div>
                                        </div>
                                    </div>
                                </div>
                                <div class="flex-auto flex justify-end items-center pr-4 rounded-tr">
                                    <div class="group z-10 relative"><button
                                            class="h-7 w-7 flex items-center justify-center rounded-md"><svg
                                                class="fill-gray-700 group-hover:fill-gray-400" width="16" height="16"
                                                viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path
                                                    d="M2 14.5H9C9.275 14.5 9.5 14.275 9.5 14V12H11V14C11 15.1031 10.1031 16 9 16H2C0.896875 16 0 15.1031 0 14V7C0 5.89687 0.896875 5 2 5H4V6.5H2C1.725 6.5 1.5 6.725 1.5 7V14C1.5 14.275 1.725 14.5 2 14.5ZM7 11C5.89687 11 5 10.1031 5 9V2C5 0.896875 5.89687 0 7 0H14C15.1031 0 16 0.896875 16 2V9C16 10.1031 15.1031 11 14 11H7Z">
                                                </path>
                                            </svg></button>
                                        <div
                                            class="absolute top-11 left-1/2 transform -translate-x-1/2 -translate-y-1/2 hidden group-hover:block text-white rounded-lg px-1.5 py-0.5 text-xs bg-primary-dark">
                                            Copy</div>
                                    </div>
                                </div>
                            </div>
                            <div class="flex overflow-auto">
                                <div class="flex-none text-gray-50 p-5 min-w-full text-sm overflow-x-auto text-xs leading-[1.35rem]"
                                    id="headlessui-tabs-panel-:rm:" role="tabpanel" tabindex="0"
                                    data-headlessui-state="selected" aria-labelledby="headlessui-tabs-tab-:rg:"
                                    style="font-variant-ligatures: none;">
                                    <pre
                                        class="language-bash"><code class="language-bash"><span class="token function">curl</span> <span class="token parameter variable">--request</span> GET <span class="token punctuation">\</span>
  <span class="token parameter variable">--url</span> https://openagents.com/api/v1/agents <span class="token punctuation">\</span>
  <span class="token parameter variable">--header</span> <span class="token string">'Authorization: &lt;authorization&gt;'</span></code></pre>
                                </div><span id="headlessui-tabs-panel-:rn:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:rh:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span><span
                                    id="headlessui-tabs-panel-:ro:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:ri:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span><span
                                    id="headlessui-tabs-panel-:rp:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:rj:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span><span
                                    id="headlessui-tabs-panel-:rq:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:rk:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span><span
                                    id="headlessui-tabs-panel-:rr:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:rl:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span>
                            </div>
                        </div>
                        <div
                            class="not-prose bg-[#0F1117] dark:bg-codeblock rounded-xl dark:ring-1 dark:ring-gray-800/50 relative">
                            <div class="flex text-xs bg-black/40 leading-6 rounded-t-xl border-b border-darkgray"
                                role="tablist" aria-orientation="horizontal">
                                <div class="flex overflow-x-auto"><button
                                        class="group flex items-center relative px-2 pt-2.5 pb-2 text-gray-400 outline-none whitespace-nowrap font-medium text-primary-light"
                                        id="headlessui-tabs-tab-:rs:" role="tab" type="button" aria-selected="true"
                                        tabindex="0" data-headlessui-state="selected"
                                        aria-controls="headlessui-tabs-panel-:ru:">
                                        <div
                                            class="px-2 rounded-md group-hover:bg-gray-700/60 group-hover:text-primary-light">
                                            <div class="z-10">200</div>
                                        </div>
                                        <div class="pointer-events-none absolute inset-0 border-b border-primary-light">
                                        </div>
                                    </button><button
                                        class="group flex items-center relative px-2 pt-2.5 pb-2 text-gray-400 outline-none whitespace-nowrap font-medium"
                                        id="headlessui-tabs-tab-:rt:" role="tab" type="button" aria-selected="false"
                                        tabindex="-1" data-headlessui-state=""
                                        aria-controls="headlessui-tabs-panel-:rv:">
                                        <div
                                            class="px-2 rounded-md group-hover:bg-gray-700/60 group-hover:text-primary-light">
                                            <div class="z-10">400</div>
                                        </div>
                                    </button></div>
                                <div class="flex-auto flex justify-end items-center pr-4 rounded-tr">
                                    <div class="group z-10 relative"><button
                                            class="h-7 w-7 flex items-center justify-center rounded-md"><svg
                                                class="fill-gray-700 group-hover:fill-gray-400" width="16" height="16"
                                                viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg">
                                                <path
                                                    d="M2 14.5H9C9.275 14.5 9.5 14.275 9.5 14V12H11V14C11 15.1031 10.1031 16 9 16H2C0.896875 16 0 15.1031 0 14V7C0 5.89687 0.896875 5 2 5H4V6.5H2C1.725 6.5 1.5 6.725 1.5 7V14C1.5 14.275 1.725 14.5 2 14.5ZM7 11C5.89687 11 5 10.1031 5 9V2C5 0.896875 5.89687 0 7 0H14C15.1031 0 16 0.896875 16 2V9C16 10.1031 15.1031 11 14 11H7Z">
                                                </path>
                                            </svg></button>
                                        <div
                                            class="absolute top-11 left-1/2 transform -translate-x-1/2 -translate-y-1/2 hidden group-hover:block text-white rounded-lg px-1.5 py-0.5 text-xs bg-primary-dark">
                                            Copy</div>
                                    </div>
                                </div>
                            </div>
                            <div class="flex overflow-auto">
                                <div class="flex-none text-gray-50 p-5 min-w-full text-sm overflow-x-auto text-xs leading-[1.35rem]"
                                    id="headlessui-tabs-panel-:ru:" role="tabpanel" tabindex="0"
                                    data-headlessui-state="selected" aria-labelledby="headlessui-tabs-tab-:rs:"
                                    style="font-variant-ligatures: none;">
                                    <pre class="language-json"><code class="language-json"><span class="token punctuation">[</span>
  <span class="token punctuation">{</span>
    <span class="token property">"name"</span><span class="token operator">:</span> <span class="token string">"&lt;string&gt;"</span><span class="token punctuation">,</span>
    <span class="token property">"tag"</span><span class="token operator">:</span> <span class="token string">"&lt;string&gt;"</span>
  <span class="token punctuation">}</span>
<span class="token punctuation">]</span></code></pre>
                                </div><span id="headlessui-tabs-panel-:rv:" role="tabpanel" tabindex="-1"
                                    aria-labelledby="headlessui-tabs-tab-:rt:"
                                    style="position: fixed; top: 1px; left: 1px; width: 1px; height: 0px; padding: 0px; margin: -1px; overflow: hidden; clip: rect(0px, 0px, 0px, 0px); white-space: nowrap; border-width: 0px;"></span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    </div>
    </x-blank-layout>
