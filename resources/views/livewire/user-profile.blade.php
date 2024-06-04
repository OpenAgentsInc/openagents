    <main class="bg-black p-12 relative">
        <div class="flex flex-row gap-x-6">
            @if($user->profile_photo_path)
                <img src="{{ str_replace('_normal', '', $user->profile_photo_path) }}" alt="{{ $user->name }}" class="rounded-xl w-[120px] h-[120px]"/>
            @else
                <img src="/images/nostrich.jpeg" alt="{{ $user->name }}" class="rounded-xl w-[120px] h-[120px]"/>
            @endif

            <div class="flex flex-col justify-center">
                <h1>{{ $user->name }}</h1>
                @if ($user->username)<h2 class="text-gray text-xs">{{ $user->username }}</h2>@endif
                <div class="flex flex-col justify-center">
                    <select id="role" wire:change="handleChange($event.target.value)"
                        class="@if(!$viewerCanModerate) pointer-events-none @endif">

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
                </div>

            </div>



            @if ($user->username)
                @if ($user->auth_provider=="X")
                    <div class="flex flex-col justify-center">
                        <a href="https://x.com/{{ $user->username }}" target="_blank"
                        class="p-1.5 border border-offblack hover:bg-offblack rounded">
                            <x-icon.x class="h-6 w-6"/>
                        </a>
                    </div>
                @endif

            @endif
        </div>

        <h3 class="mt-12 mb-6">Agents</h3>
        <div class="grid grid-cols-2 sm:grid-cols-3 gap-6">
            @foreach($user->agents as $agent)
                <livewire:agent-card :agent="$agent" :key="$agent->id"/>
            @endforeach
        </div>
    </main>

