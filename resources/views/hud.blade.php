<x-blank-layout>

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
