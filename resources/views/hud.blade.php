<x-blank-layout>

    <svg width="400" height="250" xmlns="http://www.w3.org/2000/svg">
        <rect x="7" y="1" width="386" height="248" fill="none" stroke="white" stroke-width="1" />
        <!-- Hollow circle with white stroke on the left edge -->
        <circle cx="7" cy="125" r="5" fill="black" stroke="white" stroke-width="2" />

        <!-- Hollow circle with white stroke on the right edge -->
        <circle cx="393" cy="125" r="5" fill="black" stroke="white" stroke-width="2" />
    </svg>



    <svg width="668" height="419" xmlns="http://www.w3.org/2000/svg">
        <!-- Outer rectangle with rounded corners -->
        <rect width="100%" height="100%" rx="30" fill="black" />

        <!-- Inner cutout paths -->
        <path
            d="M30,30 h608 a30,30 0 0 1 30,30 v329 a30,30 0 0 1 -30,30 h-608 a30,30 0 0 1 -30,-30 v-329 a30,30 0 0 1 30,-30 z"
            fill="none" stroke="white" stroke-width="2" />

        <!-- Additional details can be added here -->

        <!-- put all these rects in a group with 50 px down from the top -->
        <g transform="translate(0, 50)">
            <rect x="10" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="50" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="90" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="130" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="170" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="210" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="250" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="290" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="330" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="650" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
            <rect x="690" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        </g>

        <!-- Text element added here -->
        <text x="472" y="105" fill="white" font-family="JetBrains Mono" font-size="34" text-anchor="middle"
            dominant-baseline="middle">
            ATLANTISPLEB
        </text>
    </svg>


    <svg width="500" height="100" xmlns="http://www.w3.org/2000/svg">
        <rect x="10" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="50" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="90" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="130" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="170" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="210" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="250" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="290" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
        <rect x="330" y="30" width="26" height="50" fill="white" transform="skewX(-30)" />
    </svg>


    <svg width="500" height="100" xmlns="http://www.w3.org/2000/svg">
        <!-- Parallelogram 1 -->
        <rect x="50" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="0s;indefinite.end+0.3s" dur="0.3s" values="white;black;white"
                repeatCount="1" id="anim1" />
        </rect>
        <!-- Parallelogram 2 -->
        <rect x="90" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end" dur="0.3s" values="white;black;white" repeatCount="1" />
        </rect>
        <!-- Parallelogram 3 -->
        <rect x="130" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+0.3s" dur="0.3s" values="white;black;white"
                repeatCount="1" />
        </rect>
        <!-- Parallelogram 4 -->
        <rect x="170" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+0.6s" dur="0.3s" values="white;black;white"
                repeatCount="1" />
        </rect>
        <!-- Parallelogram 5 -->
        <rect x="210" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+0.9s" dur="0.3s" values="white;black;white"
                repeatCount="1" />
        </rect>
        <!-- Parallelogram 6 -->
        <rect x="250" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+1.2s" dur="0.3s" values="white;black;white"
                repeatCount="1" />
        </rect>
        <!-- Parallelogram 7 -->
        <rect x="290" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+1.5s" dur="0.3s" values="white;black;white"
                repeatCount="1" />
        </rect>
        <!-- Parallelogram 8 -->
        <rect x="330" y="30" width="26" height="50" fill="white" transform="skewX(-30)">
            <animate attributeName="fill" begin="anim1.end+1.8s" dur="0.3s" values="white;black;white" repeatCount="1"
                id="indefinite" />
        </rect>
    </svg>


    <svg width="500" height="100" xmlns="http://www.w3.org/2000/svg">
        <style>
            .parallelogram {
                fill: white;
                stroke: white;
                /* White border */
                stroke-width: 2;
                /* Border width */
            }

        </style>

        <!-- Adjusting animations to run in sequence -->

        <rect class="parallelogram" x="50" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="0s" dur="0.6s" values="white;black;black;white" keyTimes="0;0.2;0.8;1"
                repeatCount="indefinite" />
        </rect>
        <rect class="parallelogram" x="90" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="0.6s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <rect class="parallelogram" x="130" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="1.2s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <!-- Parallelogram 4 -->
        <rect class="parallelogram" x="170" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="1.8s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <!-- Parallelogram 5 -->
        <rect class="parallelogram" x="210" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="2.4s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <!-- Parallelogram 6 -->
        <rect class="parallelogram" x="250" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="3.0s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <!-- Parallelogram 7 -->
        <rect class="parallelogram" x="290" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="3.6s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
        <rect class="parallelogram" x="330" y="30" width="26" height="50" transform="skewX(-30)">
            <animate attributeName="fill" begin="4.2s" dur="0.6s" values="white;black;black;white"
                keyTimes="0;0.2;0.8;1" repeatCount="indefinite" />
        </rect>
    </svg>





    <svg width="200" height="200" xmlns="http://www.w3.org/2000/svg">
        <!-- Line 1 -->
        <line x1="10" y1="50" x2="190" y2="50" stroke="white" stroke-width="2" />
        <!-- Circle at the start of line 1 -->
        <circle cx="10" cy="50" r="5" fill="white" />
        <!-- Circle at the end of line 1 -->
        <circle cx="190" cy="50" r="5" fill="white" />

        <!-- Line 2 -->
        <line x1="10" y1="150" x2="190" y2="150" stroke="white" stroke-width="2" />
        <!-- Hollow circle at the start of line 2 -->
        <circle cx="10" cy="150" r="5" fill="black" stroke="white" stroke-width="2" />
        <!-- Hollow circle at the end of line 2 -->
        <circle cx="190" cy="150" r="5" fill="black" stroke="white" stroke-width="2" />
    </svg>



    <svg width="200" height="200" viewBox="0 0 200 200" xmlns="http://www.w3.org/2000/svg">
        <!-- Defining the path (invisible line for the motion path) -->
        <path id="motionPath" d="M10 100 L190 100" stroke="none" fill="none" />

        <!-- Line for visual reference -->
        <line x1="10" y1="100" x2="190" y2="100" stroke="white" stroke-width="2" />

        <!-- Static circle (dot) at the start of the line -->
        <circle cx="10" cy="100" r="5" fill="white" />

        <!-- Static circle (dot) at the end of the line -->
        <circle cx="190" cy="100" r="5" fill="white" />

        <!-- Circle (dot) that will move -->
        <circle r="5" fill="white">
            <!-- Animation along the motion path -->
            <animateMotion dur="4s" repeatCount="indefinite" fill="freeze">
                <mpath href="#motionPath" />
            </animateMotion>
        </circle>
    </svg>




    <svg width="100" height="20" xmlns="http://www.w3.org/2000/svg">
        <!-- Hollow circle with white stroke -->
        <circle cx="10" cy="10" r="5" fill="none" stroke="white" stroke-width="2" />

        <!-- White filled circle -->
        <circle cx="30" cy="10" r="5" fill="white" />

        <!-- Hollow circle with black stroke on a white background -->
        <circle cx="50" cy="10" r="5" fill="black" stroke="white" stroke-width="2" />

        <!-- Small white filled circle on a black background -->
        <!-- Black background circle -->
        <circle cx="70" cy="10" r="5" fill="black" />
        <!-- Small white circle on top -->
        <circle cx="70" cy="10" r="3" fill="white" />

        <!-- Smallest white filled circle -->
        <circle cx="90" cy="10" r="2" fill="white" />
    </svg>

    <svg width="300" height="300" viewBox="0 0 100 100" preserveAspectRatio="none">
        <defs>
            <radialGradient id="cornerGradient" cx="100%" cy="0%" r="100%" fx="100%" fy="0%">
                <stop offset="0%" style="stop-color:#AAAAAA;stop-opacity:1" />
                <stop offset="0%" style="stop-color:#555555;stop-opacity:1" />
                <stop offset="100%" style="stop-color:#000000;stop-opacity:1" />
            </radialGradient>
        </defs>
        <rect width="100%" height="100%" fill="url(#cornerGradient)" />
    </svg>


</x-blank-layout>
