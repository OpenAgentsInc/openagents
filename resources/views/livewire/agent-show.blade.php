<div class="flex h-screen w-full overflow-hidden">
    <div class="m-24 w-[1000px]">
        <h1>{{ $agent->name }}</h1>
        <p>{{ $agent->description }}</p>
        <livewire:graph />
    </div>
</div>
