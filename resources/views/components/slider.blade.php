<div class="range text-bitcoin">
    <input type="range" min="0" max="100" value="0" step="1" id="range" />
    <div
        class="relative bg-elevation3 p-2 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300">
        <div class="text-2xl text-bitcoin mr-1">₿</div>
        <div class="value">0</div>
    </div>
</div>

<script>
    const sliderEl = document.querySelector("#range")
    const sliderValue = document.querySelector(".value")
    const sliderInput = document.querySelector("#slider-input")

    sliderEl.addEventListener("input", (event) => {
        const tempSliderValue = event.target.value;

        sliderValue.textContent = tempSliderValue;

        const progress = (tempSliderValue / sliderEl.max) * 100;

        sliderEl.style.background = `linear-gradient(to right, #FF9900 ${progress}%, #ccc ${progress}%)`;

        sliderInput.value = tempSliderValue;
    })

</script>
