    <main class="bg-black p-12 relative">
        <div class="flex flex-row gap-x-6">
            @if($user->profile_photo_path)
                <img src="{{ str_replace('_normal', '', $user->profile_photo_path) }}" alt="{{ $user->name }}" class="rounded-xl w-[120px] h-[120px]"/>
            @else
                <img src="/images/nostrich.jpeg" alt="{{ $user->name }}" class="rounded-xl w-[120px] h-[120px]"/>
            @endif

            <div class="flex flex-col justify-center">
                <h1>{{ $user->name }}

                 <select id="role" wire:change="handleChange($event.target.value)"
                        class="@if(!$viewerCanModerate) pointer-events-none @endif text-xs">

                    >
                        @if($viewerCanModerate)
                            @foreach ($assignableRolesByViewer as $arole)
                                <option value="{{ $arole->value }}" {{ $arole->value === $user->getRole()->value ? 'selected' : '' }}>
                                    Role: {{ $arole->getLabel() }}
                                </option>
                            @endforeach
                        @endif

                        @if(!in_array($user->getRole()->value, array_map(function($role) { return $role->value; }, $this->assignableRolesByViewer)))
                            <option disabled value="{{ $user->getRole()->value }}" selected>
                                Role: {{ $user->getRole()->getLabel() }}
                            </option>
                        @endif
                    </select>

                </h1>

                <livewire:lightning-address-display :lightning-address="$user->getLightningAddress()" />
                @if ($user->auth_provider=="X")
                    <a href="https://x.com/{{ $user->username }}" target="_blank" class="text-gray text-xs inline-flex items-center m-2">
                        <x-icon.x class="h-4 w-4 mr-1"/> {{ "@".$user->username }}
                    </a>
                @endif
                @if ($user->auth_provider=="nostr")
                    <a href="https://njump.me/{{ $user->username }}" target="_blank" class="text-gray text-xs inline-flex items-center m-2">
                    <img src="/images/nostrich.jpeg" alt="{{ $user->name }}" class="rounded-xl w-4 h-4  mr-1"/>
                    <span>{{ Str::limit($user->username, 64, '...') }}</span>
                    </a>
                @endif







            </div>




        </div>

        @if(count($user->agents)>0)
            <h3 class="mt-12 mb-6">Agents</h3>
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-6">
                @foreach($user->agents as $agent)
                    <livewire:agent-card :agent="$agent" :key="$agent->id"/>
                @endforeach
            </div>
        @endif

        @if(count($user->plugins)>0)
            <h3 class="mt-12 mb-6">Plugins</h3>
            <div class="grid grid-cols-2 sm:grid-cols-3 gap-6">
                @foreach($user->plugins as $plugin)
                    <x-plugin-card :plugin="$plugin" />

                @endforeach
            </div>
        @endif
    </main>

