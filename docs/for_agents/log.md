[2024-10-20 14:33:22] testing.INFO: Test setup complete {"user_id":1,"team1_id":1,"team2_id":2,"project1_id":1,"project2_id":2,"project3_id":3}
[2024-10-20 14:33:22] testing.INFO: Response content: {"content":"<!-- Team Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"teamSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Team 1</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"teamSwitcher\">
            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/1\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"YB18QQtp16NGNlNAziqIf8KrhXnR0hTVuBvci2xj\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 1</button>
            </form>
    </a>            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/2\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"YB18QQtp16NGNlNAziqIf8KrhXnR0hTVuBvci2xj\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 2</button>
            </form>
    </a>
        </div>
    </div>
</div>
<!-- Project Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"projectSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Project</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"projectSwitcher\">

        </div>
    </div>
</div>
<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
            console.log('Team switched successfully');
        }
    });
</script>"}
[2024-10-20 14:33:22] testing.INFO: Test setup complete {"user_id":1,"team1_id":1,"team2_id":2,"project1_id":1,"project2_id":2,"project3_id":3}
[2024-10-20 14:33:22] testing.INFO: Response content for personal projects: {"content":"<!-- Team Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"teamSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Personal</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"teamSwitcher\">
            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/1\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"snXgPGjaODzMGMbMBn7CJHLtA3l6e6nE0L77L4H3\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 1</button>
            </form>
    </a>            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/2\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"snXgPGjaODzMGMbMBn7CJHLtA3l6e6nE0L77L4H3\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 2</button>
            </form>
    </a>
        </div>
    </div>
</div>
<!-- Project Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"projectSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Project</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"projectSwitcher\">

        </div>
    </div>
</div>
<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
            console.log('Team switched successfully');
        }
    });
</script>"}
[2024-10-20 14:33:22] testing.INFO: Test setup complete {"user_id":1,"team1_id":1,"team2_id":2,"project1_id":1,"project2_id":2,"project3_id":3}
[2024-10-20 14:33:22] testing.INFO: Response content after switching teams: {"content":"<!-- Team Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"teamSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Team 2</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"teamSwitcher\">
            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/1\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"sbNH9s5H1LUqGyCRN5ha910Yn1DiJmaaNxSNVZBn\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 1</button>
            </form>
    </a>            <a href=\"#\" class=\"block px-4 py-2 text-sm text-sidebar-foreground hover:bg-sidebar-accent hover:text-sidebar-accent-foreground cursor-pointer\" role=\"menuitem\">
        <form hx-post=\"http://localhost:8000/switch-team/2\" hx-target=\"#sidebarHeader\" hx-swap=\"innerHTML\" class=\"w-full h-full\">
                <input type=\"hidden\" name=\"_token\" value=\"sbNH9s5H1LUqGyCRN5ha910Yn1DiJmaaNxSNVZBn\" autocomplete=\"off\">                <button type=\"submit\" class=\"w-full h-full text-left\">Team 2</button>
            </form>
    </a>
        </div>
    </div>
</div>
<!-- Project Dropdown -->
<div x-data=\"{ open: false }\"
     class=\"relative inline-block text-left w-full\"
     id=\"projectSwitcher\">
    <div>
        <button @click=\"!false && (open = !open)\" type=\"button\"
                class=\"inline-flex justify-between w-full rounded-md border border-sidebar-border bg-transparent px-4 py-2 text-sm font-medium shadow-sm focus:outline-none focus:ring-0 focus:ring-sidebar-ring transition-opacity duration-200\"
                :class=\"{ 'text-sidebar-foreground hover:bg-sidebar-accent': !false, 'text-sidebar-muted cursor-not-allowed opacity-50': false }\"
                aria-haspopup=\"true\" x-bind:aria-expanded=\"open.toString()\"
                :disabled=\"false\">
            <span class=\"flex items-center\">
                                <span class=\"flex-grow text-left\">Project</span>
            </span>
                            <svg class=\"h-5 w-5\" xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 20 20\" fill=\"currentColor\" aria-hidden=\"true\">
                    <path fill-rule=\"evenodd\" d=\"M5.293 7.293a1 1 0 011.414 0L10 10.586l3.293-3.293a1 1 0 111.414 1.414l-4 4a1 1 0 01-1.414 0l-4-4a1 1 0 010-1.414z\" clip-rule=\"evenodd\" />
                </svg>
                    </button>
    </div>

    <div x-show=\"open && !false\" @click.away=\"open = false\" class=\"origin-top-right absolute right-0 mt-2 w-full rounded-md shadow-lg bg-sidebar-background ring-1 ring-sidebar-border ring-opacity-5 divide-y divide-sidebar-border z-50\">
        <div class=\"py-1\" role=\"menu\" aria-orientation=\"vertical\" aria-labelledby=\"projectSwitcher\">

        </div>
    </div>
</div>
<script>
    document.body.addEventListener('htmx:afterSwap', function(event) {
        if (event.detail.target.id === 'sidebarHeader') {
            // Reinitialize any JavaScript components or listeners if needed
            console.log('Team switched successfully');
        }
    });
</script>"}
