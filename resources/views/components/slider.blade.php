<div class="range">
    <input type="range" min="0" max="100" value="0" step="1" id="range" />
    <div
        class="relative bg-elevation3 p-2 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300">
        <svg class="w-6 h-6" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
        </svg>
        <div class="value">0</div>
    </div>
</div>

<script>
    const sliderEl = document.querySelector("#range")
    const sliderValue = document.querySelector(".value")

    sliderEl.addEventListener("input", (event) => {
        const tempSliderValue = event.target.value;

        sliderValue.textContent = tempSliderValue;

        const progress = (tempSliderValue / sliderEl.max) * 100;

        sliderEl.style.background = `linear-gradient(to right, #FF9900 ${progress}%, #ccc ${progress}%)`;
    })

</script>
