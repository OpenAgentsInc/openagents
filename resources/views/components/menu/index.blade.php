<div x-data="{ menuOpen: false }">
    <div x-menu x-model="menuOpen" class="relative flex items-center">
        {{ $slot }}
    </div>
</div>
