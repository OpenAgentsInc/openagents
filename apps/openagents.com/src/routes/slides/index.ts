import { document, html } from "@openagentsinc/psionic"

export default function slides() {
  return document({
    title: "OpenAgents - Terminal Slides",
    styles: `
      body {
        margin: 0;
        padding: 0;
        background: #000;
        color: #fff;
        font-family: "Berkeley Mono", "Cascadia Code", "Source Code Pro", monospace;
        overflow: hidden;
        height: 100vh;
        display: flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        user-select: none;
      }
      
      .slides-container {
        width: 100%;
        height: 100%;
        position: relative;
        display: flex;
        align-items: center;
        justify-content: center;
      }
      
      .slide {
        position: absolute;
        width: 100%;
        height: 100%;
        display: flex;
        align-items: center;
        justify-content: center;
        opacity: 0;
        transition: opacity 0.5s ease-in-out;
      }
      
      .slide.active {
        opacity: 1;
      }
      
      .slide-content {
        text-align: center;
        font-size: clamp(2rem, 6vw, 4rem);
        font-weight: bold;
        text-shadow: 0 0 20px currentColor;
        line-height: 1.2;
        padding: 2rem;
      }
      
      .slide-1 {
        color: #fff;
      }
      
      .slide-1 .slide-content {
        font-size: clamp(2rem, 6vw, 4rem);
      }
      
      .slide-2 {
        background: #000;
      }
      
      .slide-2 img {
        max-width: 60%;
        max-height: 60%;
        object-fit: contain;
      }
      
      .slide-3 {
        color: #fff;
      }
      
      .slide-3 .slide-content {
        font-size: clamp(2rem, 6vw, 4rem);
      }
      
      .slide-4 {
        background: #000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }
      
      .slide-4 img {
        max-width: 40%;
        max-height: 40%;
        object-fit: contain;
      }
      
      .slide-4 .tagline {
        color: #fff;
        font-size: clamp(1rem, 3vw, 1.5rem);
        text-align: center;
        margin-top: 2rem;
        padding: 0 2rem;
        max-width: 80%;
        line-height: 1.6;
      }
      
      .slide-5 {
        color: #fff;
      }
      
      .slide-5 .slide-content {
        font-size: clamp(1.2rem, 3.5vw, 2rem);
        max-width: 80%;
        margin: 0 auto;
      }
      
      .slide-6 {
        color: #fff;
      }
      
      .slide-6 .slide-content {
        font-size: clamp(2rem, 6vw, 4rem);
      }
      
      .nav-dots {
        position: fixed;
        bottom: 2rem;
        left: 50%;
        transform: translateX(-50%);
        display: flex;
        gap: 1rem;
        z-index: 100;
      }
      
      .dot {
        width: 12px;
        height: 12px;
        border: 2px solid #fff;
        background: transparent;
        cursor: pointer;
        transition: background 0.3s;
      }
      
      .dot.active {
        background: #fff;
      }
      
      /* Terminal cursor blink effect */
      @keyframes blink {
        0%, 49% { opacity: 1; }
        50%, 100% { opacity: 0; }
      }
      
      .cursor {
        display: inline-block;
        width: 0.8em;
        height: 1.2em;
        background: currentColor;
        margin-left: 0.1em;
        animation: blink 1s infinite;
        vertical-align: text-bottom;
      }
      
      /* Glitch effect for slide transitions */
      @keyframes glitch {
        0%, 100% { transform: translate(0); }
        20% { transform: translate(-2px, 2px); }
        40% { transform: translate(-2px, -2px); }
        60% { transform: translate(2px, 2px); }
        80% { transform: translate(2px, -2px); }
      }
      
      .slide.transitioning {
        animation: glitch 0.3s;
      }
      
      /* Mobile responsive */
      @media (max-width: 768px) {
        .slide-content {
          font-size: clamp(1.5rem, 5vw, 3rem);
        }
        
        .slide-1 .slide-content,
        .slide-3 .slide-content,
        .slide-6 .slide-content {
          font-size: clamp(1.5rem, 5vw, 3rem);
        }
        
        .slide-5 .slide-content {
          font-size: clamp(1rem, 3vw, 1.5rem);
        }
      }
    `,
    body: html`
      <div class="slides-container">
        <div class="slide slide-1 active" data-slide="1">
          <div class="slide-content">
            Open Agents Win<span class="cursor"></span>
          </div>
        </div>
        
        <div class="slide slide-2" data-slide="2">
          <img src="/openagents.png" alt="OpenAgents">
        </div>
        
        <div class="slide slide-3" data-slide="3">
          <div class="slide-content">
            n &lt; n<sup>2</sup> &lt; 2<sup>n</sup>
          </div>
        </div>
        
        <div class="slide slide-4" data-slide="4">
          <img src="/sdk.png" alt="SDK">
          <div class="tagline">
            One SDK for Bitcoin, Lightning, Ecash, Ark, Nostr Wallet Connect,<br>
            Data Vending Machines, Model Context Protocol, A2A, and more.
          </div>
        </div>
        
        <div class="slide slide-5" data-slide="5">
          <div class="slide-content">
            <h2 style="margin-bottom: 2rem;">Psionic Framework</h2>
            <ul style="text-align: left; list-style: none; padding: 0;">
              <li style="margin-bottom: 1rem;">• Sync-first hypermedia framework on Bun & Elysia</li>
              <li style="margin-bottom: 1rem;">• Built-in component explorer for rapid UI development</li>
              <li style="margin-bottom: 1rem;">• Effect.js integration for functional composition</li>
            </ul>
          </div>
        </div>
        
        <div class="slide slide-6" data-slide="6">
          <div class="slide-content">
            Join Us<span class="cursor"></span>
          </div>
        </div>
        
        <div class="nav-dots">
          <div class="dot active" data-goto="1"></div>
          <div class="dot" data-goto="2"></div>
          <div class="dot" data-goto="3"></div>
          <div class="dot" data-goto="4"></div>
          <div class="dot" data-goto="5"></div>
          <div class="dot" data-goto="6"></div>
        </div>
      </div>
      
      <script>
        let currentSlide = 1;
        const totalSlides = 6;
        
        function showSlide(n) {
          // Remove active class from all slides and dots
          document.querySelectorAll('.slide').forEach(slide => {
            slide.classList.remove('active', 'transitioning');
          });
          document.querySelectorAll('.dot').forEach(dot => {
            dot.classList.remove('active');
          });
          
          // Wrap around
          if (n > totalSlides) currentSlide = 1;
          if (n < 1) currentSlide = totalSlides;
          
          // Add active class to current slide and dot
          const activeSlide = document.querySelector(\`.slide[data-slide="\${currentSlide}"]\`);
          activeSlide.classList.add('active', 'transitioning');
          document.querySelector(\`.dot[data-goto="\${currentSlide}"]\`).classList.add('active');
          
          // Remove transitioning class after animation
          setTimeout(() => {
            activeSlide.classList.remove('transitioning');
          }, 300);
        }
        
        function nextSlide() {
          currentSlide++;
          showSlide(currentSlide);
        }
        
        function prevSlide() {
          currentSlide--;
          showSlide(currentSlide);
        }
        
        function goToSlide(n) {
          currentSlide = parseInt(n);
          showSlide(currentSlide);
        }
        
        // Keyboard navigation
        document.addEventListener('keydown', (e) => {
          if (e.key === 'ArrowRight' || e.key === ' ') {
            e.preventDefault();
            nextSlide();
          } else if (e.key === 'ArrowLeft') {
            e.preventDefault();
            prevSlide();
          } else if (e.key >= '1' && e.key <= '6') {
            goToSlide(e.key);
          }
        });
        
        // Click navigation
        document.addEventListener('click', (e) => {
          if (e.target.classList.contains('dot')) {
            goToSlide(e.target.dataset.goto);
          } else if (!e.target.closest('.nav-dots')) {
            nextSlide();
          }
        });
        
        // Touch navigation for mobile
        let touchStartX = 0;
        let touchEndX = 0;
        
        document.addEventListener('touchstart', (e) => {
          touchStartX = e.changedTouches[0].screenX;
        });
        
        document.addEventListener('touchend', (e) => {
          touchEndX = e.changedTouches[0].screenX;
          handleSwipe();
        });
        
        function handleSwipe() {
          if (touchEndX < touchStartX - 50) nextSlide();
          if (touchEndX > touchStartX + 50) prevSlide();
        }
        
        // Fullscreen on F key
        document.addEventListener('keydown', (e) => {
          if (e.key === 'f' || e.key === 'F') {
            if (!document.fullscreenElement) {
              document.documentElement.requestFullscreen();
            } else {
              document.exitFullscreen();
            }
          }
        });
      </script>
    `
  })
}
