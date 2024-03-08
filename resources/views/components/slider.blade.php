@props(['min' => 0, 'max' => 10000, 'step' => 1, 'value' => 0])

<div class="range text-bitcoin">
    <input type="range" min="{{ $min }}" max="{{ $max }}" value="{{ $value }}" step="{{ $step }}" id="range"/>
    <div
            class="relative bg-elevation3 p-2 rounded-md font-sans inline-flex items-center justify-center text-grey-500 dark:text-grey-300">
        <div class="text-2xl text-bitcoin mr-1">₿</div>
        <div class="value">0</div>
    </div>
</div>

<script>
    document.addEventListener('DOMContentLoaded', function () {
        const sliderEl = document.querySelector("#range");
        const sliderValue = document.querySelector(".value");
        const sliderInput = document.querySelector("#slider-input");

        // Function to update UI based on slider value
        function updateSliderUI(value) {
            sliderValue.textContent = value; // Update text content

            const progress = (value / sliderEl.max) * 100;
            sliderEl.style.background = `linear-gradient(to right, #FF9900 ${progress}%, #ccc ${progress}%)`;

            if (sliderInput) {
                sliderInput.value = value; // Update hidden input if present
            }
        }

        // Initialize slider UI on page load
        updateSliderUI(sliderEl.value);

        // Update UI on slider input change
        sliderEl.addEventListener("input", (event) => {
            updateSliderUI(event.target.value);
        });
    });
</script>