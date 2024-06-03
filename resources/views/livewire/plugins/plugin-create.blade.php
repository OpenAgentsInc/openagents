<div>
    {{-- Be like water. --}}
    <div class="mt-10 p-5 y-5 mx-auto w-full max-w-5xl md:max-w-[800px]">
        <h1 class="text-md md:text-3xl font-bold my-6 md:mb-10 text-center">Plugin Editor</h1>
        <a href="{{ route('plugins.index') }}" wire:navigate class="order-1 mb-4 md:mb-0 md:text-left">
            <h3 class="text-[16px] text-gray">&larr; Back</h3>
        </a>
        <div class="my-5 mx-auto max-w-5xl">

            <form wire:submit.prevent="submit">
                <x-pane title="Metadata">

                    <div class="mt-5">
                        <label for="name">Name</label>
                        <x-input id="name" class="block mt-1 w-full " type="text" name="name"
                            wire:model='name' dusk="name" placeholder="Name your plugin" />
                        @error('name')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>

                    <div class="mt-5">
                        <label for="about">Description</label>
                        <x-textarea wire:model='description'
                            placeholder="Add a short description about what this plugin does" id="about"
                            class="block mt-1 w-full" dusk="description" min-rows="3" name="about" />
                        @error('about')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>

                    <div class="mt-5">
                        <label for="web">Website</label>
                        <x-input id="web" class="block mt-1 w-full " type="text" name="web"
                            wire:model='web' dusk="web" placeholder="Youe plugin website" />
                        @error('web')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>


                    <div class="mt-5">
                        <label for="privacy">Link to Privacy Policy</label>
                        <x-input id="privacy" class="block mt-1 w-full " type="text" name="privacy"
                            wire:model='privacy' dusk="privacy" placeholder="Your plugin privacy policy" />
                        @error('privacy')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>


                    <div class="mt-5">
                        <label for="tos">Link to Terms of Service</label>
                        <x-input id="tos" class="block mt-1 w-full " type="text" name="tos"
                            wire:model='tos' dusk="tos" placeholder="Name your plugin" />
                        @error('name')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>
                    <div class="my-5">
                        <label for="tags">Tags</label>
                        <x-tags-input wire:model='tags' name="tags" :tags="[]" />
                        @error('tags')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>
                </x-pane>

                <div class="my-12" />


                <x-pane title="WASM File">
                    <p class="mt-1 mb-4">Upload the WebAssembly file for your plugin</p>
                    <div class="mt-1 border-2 border-darkgray rounded-md">
                        <x-filepond ref="wasmFile" wire:model="wasm_upload" allowFileTypeValidation
                            imagePreviewMaxHeight="300" acceptedFileTypes="['application/wasm']"
                            fileValidateTypeLabelExpectedTypesMap="{{ json_encode([
                                'application/wasm' => '.wasm',
                            ]) }}"
                            allowFileSizeValidation maxFileSize="10MB" />
                    </div>
                     @error('wasm_upload')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                </x-pane>


                <div class="my-12" />


                <x-pane title="Inputs">

                    <p>Inputs are values that everyone can set when using the action</p>
                    <table class="table-auto w-full">
                        <thead>
                            <tr>
                                @if (count($inputs) > 0)
                                <th class="px-4 py-2">Key</th>
                                <th class="px-4 py-2">Type</th>
                                <th class="px-4 py-2">Default</th>

                                <th class="px-4 py-2">Description</th>
                                <th class="px-4 py-2">Required</th>
                                <th></th>
                                @endif
                            </tr>
                        </thead>
                        <tbody>
                            @foreach ($inputs as $key => $input)
                                <tr>
                                    <td class="border px-4 py-2">
                                        <x-input type="text" onkeypress="return /[a-z0-9]/i.test(event.key)"
                                            wire:model="inputs.{{ $key }}.name"
                                            id="inputs_{{ $key }}_name" class="text-sm block mt-1 w-full "
                                            placeholder="name" autocomplete="off" />
                                    </td>
                                    <td class="border px-4 py-2">
                                        <select wire:model='inputs.{{ $key }}.type'
                                            class="text-sm w-full border-darkgray bg-black text-white focus:border-white focus:ring-white rounded-md shadow-sm">
                                            <option value="string">String</option>
                                            <option value="integer">Integer</option>
                                            <option value="array">Array</option>
                                            <option value="object">Object</option>
                                        </select>
                                    </td>
                                    <td class="border px-4 py-2"> <x-input type="text"
                                            wire:model='inputs.{{ $key }}.default'
                                            class="text-sm block mt-1 w-full" placeholder="value"
                                            autocomplete="off" /></td>

                                    <td class="border px-4 py-2"> <x-input type="text"
                                            wire:model='inputs.{{ $key }}.description'
                                            class="text-sm block mt-1 w-full" placeholder="description"
                                            autocomplete="off" /></td>
                                    <td class="border px-4 py-2  "><x-switch class="w-full "
                                            wire:model='inputs.{{ $key }}.required' /></td>
                                    <td class="border px-4 py-2 ">
                                        <div class="flex justify-between">

                                            <x-icon.copy class="h-4 w-4 m-2 cursor-pointer"
                                                onclick="navigator.clipboard.writeText('\{\{in.'+document.querySelector('#inputs_{{ $key }}_name').value+'\}\}'); alert('TAG copied')" />
                                            @if ($key > 0)
                                                <x-icon.trash wire:click="removeInput({{ $key }})"
                                                    class="w-4 h-4 m-2 cursor-pointer" />
                                            @endif
                                        </div>
                                    </td>
                                </tr>
                            @endforeach
                        </tbody>
                    </table>

                    <div class="flex flex-col">
                        @error('inputs.' . $key . '.name')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                        @error('inputs.' . $key . '.required')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                        @error('inputs.' . $key . '.default')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                        @error('inputs.' . $key . '.type')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                        @error('inputs.' . $key . '.description')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                    </div>
                    <div wire:click="addInput"
                        class="flex items-center justify-center text-white text-sm py-4 w-full cursor-pointer">
                        <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"
                            xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd"
                                d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z"
                                clip-rule="evenodd"></path>
                        </svg>
                        <p class="ml-2">Add New Input</p>
                    </div>
                </x-pane>

                <div class="my-12" />



                <x-pane title="Secrets">
                    <p>Secrets are available only to our nodes, they can be used to set sensitive information, like API
                        keys</p>

                    <table class="table-auto w-full">
                        <thead>
                            <tr>
                                @if (count($secrets) > 0)
                                <th class="px-4 py-2">Key</th>
                                <th class="px-4 py-2">Value</th>
                                <th></th>
                                @endif
                            </tr>
                        </thead>
                        <tbody>
                            @foreach ($secrets as $key => $secret)
                                <tr>
                                    <td class="border px-4 py-2">
                                        <x-input type="text" onkeypress="return /[a-z0-9]/i.test(event.key)"
                                            wire:model="secrets.{{ $key }}.key"
                                            id="secrets_{{ $key }}_key" class="text-sm block mt-1 w-full "
                                            placeholder="name" autocomplete="off" />
                                    </td>
                                    <td class="border px-4 py-2">
                                        <x-input type="text" wire:model='secrets.{{ $key }}.value'
                                            class="text-sm block mt-1 w-full" placeholder="value"
                                            autocomplete="off" />
                                    </td>
                                    <td class="border px-4 py-2 ">
                                        <div class="flex justify-between">
                                            <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                                onclick="navigator.clipboard.writeText('%secret.'+(document.querySelector('#secrets_{{ $key }}_key').value)+'%');alert('TAG copied');  " />
                                            <x-icon.trash wire:click="removeSecretInput({{ $key }})"
                                                class="w-4 h-4 m-2 cursor-pointer" />
                                        </div>
                                    </td>
                                </tr>
                            @endforeach
                        </tbody>
                    </table>
                    <div class="flex flex-col">
                        @error('secrets.' . $key . '.key')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror
                        @error('secrets.' . $key . '.value')
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror

                    </div>
                    <div wire:click="addSecretInput"
                        class="flex items-center justify-center text-white text-sm py-4 w-full cursor-pointer">
                        <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"
                            xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd"
                                d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z"
                                clip-rule="evenodd"></path>
                        </svg>
                        <p class="ml-2">Add Secret</p>
                    </div>
                </x-pane>


                <div class="my-12" />

                <x-pane title="Environment Variables">
                    <p>Variables set by the environment, you can use them anywhere in the input template</p>

                    <table class="table-auto w-full">
                        <thead>
                            <tr>
                                <th class="px-4 py-2">Key</th>
                                <th class="px-4 py-2">Description</th>
                                <th></th>
                            </tr>
                        </thead>
                        <tbody>
                            <tr>
                                <td class="border px-4 py-2 text-sm ">
                                    sys.timestamp_seconds
                                </td>
                                <td class="border px-4 py-2 text-sm">
                                    Current timestamp in seconds
                                </td>
                                <td class="border px-4 py-2  ">
                                    <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                        onclick="navigator.clipboard.writeText('\{\{sys.timestamp_seconts\}\}');alert('TAG copied');  " />
                                </td>
                            </tr>
                            <tr>
                                <td class="border px-4 py-2 text-sm ">
                                    sys.expiration_timestamp_seconds
                                </td>
                                <td class="border px-4 py-2 text-sm">
                                    The expiration timestamp in seconds for the current job
                                </td>
                                <td class="border px-4 py-2  ">
                                    <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                        onclick="navigator.clipboard.writeText('\{\{sys.expiration_timestamp_seconds\}\}');alert('TAG copied');  " />
                                </td>
                            </tr>
                            <tr>
                                <td class="border px-4 py-2 text-sm ">
                                    sys.bidAmount
                                </td>
                                <td class="border px-4 py-2 text-sm ">
                                    The amount bidded for the current job
                                </td>
                                <td class="border px-4 py-2 ">
                                    <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                        onclick="navigator.clipboard.writeText('\{\{sys.bidAmount\}\}');alert('TAG copied');  " />
                                </td>
                            </tr>
                            <tr>
                                <td class="border px-4 py-2 text-sm ">
                                    sys.bidCurrency
                                </td>
                                <td class="border px-4 py-2 text-sm ">
                                    The currency bidded for the current job
                                </td>
                                <td class="border px-4 py-2 ">
                                    <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                        onclick="navigator.clipboard.writeText('\{\{sys.bidCurrency\}\}');alert('TAG copied');  " />
                                </td>
                            </tr>
                            <tr>
                                <td class="border px-4 py-2 text-sm ">
                                    sys.bidProtocol
                                </td>
                                <td class="border px-4 py-2 text-sm">
                                    The protocol used to bid for the current job
                                </td>
                                <td class="border px-4 py-2 ">
                                    <x-icon.copy class="w-4 h-4 m-2 cursor-pointer"
                                        onclick="navigator.clipboard.writeText('\{\{sys.bidProtocol\}\}');alert('TAG copied');  " />
                                </td>
                            </tr>
                        </tbody>
                    </table>


                </x-pane>

                <div class="my-12" />


                <x-pane title="Input Template">
                    <p class="mt-1 mb-4">
                        The <a href="https://mustache.github.io/" target="_blank">@{{ mustache }}</a> template
                        to define the layout of the input.
                        <br>
                        Click on the <x-icon.copy class="w-4 h-4   inline-block" /> icons on this page
                        to copy the mustache TAGs for variables and secrets.

                    </p>
                    <x-textarea wire:model='input_template' {{-- placeholder='{{ '{' }}text{{ '}' }}: {{ '{' }}{in.text}}{{ '}' }}, {{ '{' }}to{{ '}' }}: {{ '{' }}{in.target_lang}}{{ '}' }}, {{ '{' }}api_key{{ '}' }}: "%secret.api_key%"' --}} id="34rqwerrty"
                        class="block mt-1 w-full" dusk="description" min-rows="3" name="input_template" />
                    @error('input_template')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </x-pane>

                <div class="my-12" />
                  <x-pane title="Allowed Hosts">
                    <p>The hosts to which the plugin can connect to</p>

                    <table class="table-auto w-full">
                        <thead>
                            <tr>
                                @if (count($allowed_hosts) > 0)
                                <th class="px-4 py-2">Host</th>
                                <th></th>
                                @endif
                            </tr>
                        </thead>
                        <tbody>
                            @foreach ($allowed_hosts as $key => $host)
                                <tr>
                                    <td class="border px-4 py-2">
                                        <x-input type="text"
                                            wire:model="allowed_hosts.{{ $key }}"
                                            class="text-sm block mt-1 w-full "
                                            placeholder="example.com" autocomplete="off" />
                                    </td>

                                    <td class="border px-4 py-2 ">
                                        <div class="flex justify-between">
                                            <x-icon.trash wire:click="removeAllowedHost({{ $key }})"
                                                class="w-4 h-4 m-2 cursor-pointer" />
                                        </div>
                                    </td>
                                </tr>
                            @endforeach
                        </tbody>
                    </table>
                    <div class="flex flex-col">
                        @error('allowed_hosts.' . $key )
                            <span class="text-red mt-2 text-xs">{{ $message }}</span>
                        @enderror

                    </div>
                    <div wire:click="addAllowedHost"
                        class="flex items-center justify-center text-white text-sm py-4 w-full cursor-pointer">
                        <svg class="w-4 h-4" fill="currentColor" viewBox="0 0 20 20"
                            xmlns="http://www.w3.org/2000/svg">
                            <path fill-rule="evenodd"
                                d="M10 18a8 8 0 100-16 8 8 0 000 16zm1-11a1 1 0 10-2 0v2H7a1 1 0 100 2h2v2a1 1 0 102 0v-2h2a1 1 0 100-2h-2V7z"
                                clip-rule="evenodd"></path>
                        </svg>
                        <p class="ml-2">Add Host</p>
                    </div>
                </x-pane>


                <div class="my-12" />

                <x-pane title="Payments">
                    <x-input id="name" class="block mt-1 w-full " type="text" name="payment"
                        wire:model='payment' dusk="payment" placeholder="Bitcoin lightning address" />
                    @error('payment')
                        <span class="text-red mt-2 text-xs">{{ $message }}</span>
                    @enderror
                </x-pane>

                <div class="my-12" />


                <div class="mt-5 w-full text-center">
                    <x-button type="submit" class="text-center justify-center gap-2 py-2 my-4"
                        dusk="create-plugin-button">
                        Save
                    </x-button>
                </div>
            </form>
        </div>
    </div>
</div>
