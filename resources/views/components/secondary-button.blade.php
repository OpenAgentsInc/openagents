<button
    {{ $attributes->merge(['type' => 'button', 'class' => 'inline-flex items-center px-4 py-2 bg-black border border-gray rounded-md font-semibold text-xs text-gray uppercase tracking-widest shadow-sm hover:bg-gray focus:outline-none focus:ring-2 focus:ring-gray focus:ring-offset-2 disabled:opacity-25 transition ease-in-out duration-150']) }}>
    {{ $slot }}
</button>
