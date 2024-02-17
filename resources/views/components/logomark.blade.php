@props(['size' => 1])

    @php
        // Map size numbers to exact SVG dimensions and corresponding font sizes
        $sizes = [
        1 => ['width' => '32px', 'height' => '32px', 'fontClass' => 'text-[40px]'], // Logo 32x32, font size 40
        2 => ['width' => '25.6px', 'height' => '25.6px', 'fontClass' => 'text-[32px]'], // Logo 25.6x25.6, font size 32
        3 => ['width' => '19.2px', 'height' => '19.2px', 'fontClass' => 'text-[24px]'], // Logo 19.2x19.2, font size 24
        4 => ['width' => '16px', 'height' => '16px', 'fontClass' => 'text-[20px]'], // Logo 16x16, font size 20
        5 => ['width' => '12.8px', 'height' => '12.8px', 'fontClass' => 'text-[16px]'], // Logo 12.8x12.8, font size 16
        ];

        // Determine SVG dimensions and font class based on size prop
        $selectedSize = $sizes[$size] ?? $sizes[3]; // Fallback to default size if out of bounds
    @endphp

    <div class="flex items-center pointer-events-none">
        <svg style="width: {{ $selectedSize['width'] }}; height: {{ $selectedSize['height'] }};"
            viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
            <path
                d="M7.72114 4.47436L7.71503 4.47842C3.51138 7.27186 1 11.9475 1 16.9937C1 25.2729 7.73142 32 16 32C24.2686 32 31 25.2729 31 16.9937C31 11.9474 28.4886 7.27173 24.2802 4.47829L24.279 4.47754C23.4779 3.94941 22.3898 4.16483 21.8573 4.97703C21.32 5.78505 21.5421 6.87727 22.3457 7.41311L22.3466 7.41371C25.571 9.55182 27.4935 13.1337 27.4935 16.9937C27.4935 23.3252 22.3396 28.4853 16 28.4853C9.66035 28.4853 4.50649 23.3252 4.50649 16.9937C4.50649 13.1337 6.42903 9.55182 9.65339 7.41371L9.6543 7.4131C10.4578 6.87738 10.6799 5.78548 10.143 4.97752C9.81295 4.47167 9.25963 4.19218 8.69969 4.19092C8.35648 4.18555 8.01823 4.28419 7.72114 4.47436Z"
                fill="white" />
            <path
                d="M16 0C15.0355 0 14.2468 0.783471 14.2468 1.75733V13.6814C14.2468 14.6553 15.0355 15.4388 16 15.4388C16.9645 15.4388 17.7532 14.6553 17.7532 13.6814V1.75733C17.7532 0.783471 16.9645 0 16 0Z"
                fill="white" />
        </svg>
        <span class="{{ $selectedSize['fontClass'] }} ml-2 font-extrabold">OpenAgents</span>
    </div>
