<x-blank-layout>


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



</x-blank-layout>
