<!-- Twitter Card tags -->
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:site" content="@OpenAgentsInc">
<meta name="twitter:creator" content="@OpenAgentsInc">

<!-- Open Graph tags for fallback -->
<meta property="og:url" content="{{ url()->current() }}">
<meta property="og:title" content="Chat with OpenAgents">
<meta property="og:description"
      content="It's the coolest AI chat. Literally 1000x better than the rest. Try it now or else.">
<meta property="og:image"
      content="{{ request()->is('launch') ? 'https://staging.openagents.com/images/one.png' : 'https://staging.openagents.com/images/openagents.png' }}">
