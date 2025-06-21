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

      .slide-4 h2 {
        color: #fff;
        font-size: clamp(2rem, 6vw, 4rem);
        margin-bottom: 3rem;
        text-shadow: 0 0 20px currentColor;
      }

      .slide-4 img {
        max-width: 60%;
        max-height: 60%;
        object-fit: contain;
      }

      .slide-5 {
        background: #000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }

      .slide-5 h2 {
        color: #fff;
        font-size: clamp(2rem, 6vw, 4rem);
        margin-bottom: 3rem;
        text-shadow: 0 0 20px currentColor;
      }

      .slide-5 img {
        max-width: 60%;
        max-height: 60%;
        object-fit: contain;
      }

      .slide-6 {
        background: #000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }

      .slide-6 h2 {
        color: #fff;
        font-size: clamp(2rem, 6vw, 4rem);
        margin-bottom: 3rem;
        text-shadow: 0 0 20px currentColor;
      }

      .slide-6 img {
        max-width: 40%;
        max-height: 40%;
        object-fit: contain;
      }

      .slide-6 .tagline {
        color: #fff;
        font-size: clamp(1rem, 3vw, 1.2rem);
        text-align: center;
        margin-top: 3rem;
        padding: 0 2rem;
        max-width: 53%;
        line-height: 1.6;
      }

      .slide-7 {
        color: #fff;
      }

      .slide-7 .slide-content {
        font-size: clamp(1.2rem, 3.5vw, 2rem);
        max-width: 80%;
        margin: 0 auto;
      }

      .slide-8 {
        background: #000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }

      .slide-8 h2 {
        color: #fff;
        font-size: clamp(2rem, 6vw, 4rem);
        margin-bottom: 3rem;
        text-shadow: 0 0 20px currentColor;
      }

      .slide-8 img {
        max-width: 60%;
        max-height: 60%;
        object-fit: contain;
      }

      .slide-9 {
        background: #000;
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
      }

      .slide-9 h2 {
        color: #fff;
        font-size: clamp(2rem, 6vw, 4rem);
        margin-bottom: 3rem;
        text-shadow: 0 0 20px currentColor;
      }

      .slide-9 img {
        max-width: 60%;
        max-height: 60%;
        object-fit: contain;
      }

      .slide-10 {
        color: #fff;
      }

      .slide-10 .slide-content {
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
        .slide-10 .slide-content {
          font-size: clamp(1.5rem, 5vw, 3rem);
        }

        .slide-7 .slide-content {
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
            <h3 style="font-size: clamp(1.5rem, 4vw, 2.2rem); margin-bottom: 2rem; text-shadow: 0 0 20px currentColor;">Reed's Law of Group-Forming Networks</h3>
            n &lt; n<sup>2</sup> &lt; 2<sup>n</sup>
            <p style="font-size: clamp(1.5rem, 4vw, 2.2rem); margin-top: 2rem; text-shadow: 0 0 20px currentColor;">"mathematically overwhelming"</p>
          </div>
        </div>

        <div class="slide slide-4" data-slide="4">
          <h2>OpenAgents Compute</h2>
          <img src="/174small.png" alt="OpenAgents Compute - Episode 174">
        </div>

        <div class="slide slide-5" data-slide="5">
          <h2>Commander</h2>
          <img src="/commander.png" alt="Commander">
        </div>

        <div class="slide slide-6" data-slide="6">
          <h2>OpenAgents SDK</h2>
          <img src="/sdk.png" alt="SDK">
          <div class="tagline">
            One SDK for Bitcoin, Lightning, Nostr, NWC, L402, Ecash, Spark, Taproot Assets, Ark,
            Data Vending Machines, Model Context Protocol, A2A, and more.
          </div>
        </div>

        <div class="slide slide-7" data-slide="7">
          <div class="slide-content">
            <h2 style="margin-bottom: 2rem; text-shadow: 0 0 20px currentColor;">Psionic</h2>
            <ul style="text-align: left; list-style: none; padding: 0; font-size: clamp(1rem, 2.5vw, 1.5rem); text-shadow: none;">
              <li style="margin-bottom: 1rem;">• Agent-first app framework</li>
              <li style="margin-bottom: 1rem;">• Inspired by HyperCard & HTMX</li>
              <li style="margin-bottom: 1rem;">• End-to-end type safety</li>
            </ul>
          </div>
        </div>

        <div class="slide slide-8" data-slide="8">
          <h2>openagents.com</h2>
          <img src="/dont.png" alt="Don't">
        </div>

        <div class="slide slide-9" data-slide="9">
          <h2>Open Agents NIP</h2>
          <img src="/nipoa.png" alt="NIP-OA">
        </div>

        <div class="slide slide-10" data-slide="10">
          <div class="slide-content">
            <h2 style="margin-bottom: 3rem; text-shadow: 0 0 20px currentColor; font-size: clamp(1.5rem, 5vw, 3.5rem);">Join Us<span class="cursor"></span></h2>
            <div style="font-size: clamp(1.2rem, 3vw, 1.8rem); line-height: 2;">
              <p style="margin: 1rem 0;">X: @OpenAgentsInc</p>
              <p style="margin: 1rem 0;">chris@openagents.com</p>
              <p style="margin: 1rem 0;"><a href="https://github.com/OpenAgentsInc" style="color: #fff; text-decoration: none;">github.com/OpenAgentsInc</a></p>
              <p style="margin: 1rem 0;"><a href="https://stacker.news/~openagents" style="color: #fff; text-decoration: none;">stacker.news/~openagents</a></p>
            </div>
          </div>
        </div>

        <div class="nav-dots">
          <div class="dot active" data-goto="1"></div>
          <div class="dot" data-goto="2"></div>
          <div class="dot" data-goto="3"></div>
          <div class="dot" data-goto="4"></div>
          <div class="dot" data-goto="5"></div>
          <div class="dot" data-goto="6"></div>
          <div class="dot" data-goto="7"></div>
          <div class="dot" data-goto="8"></div>
          <div class="dot" data-goto="9"></div>
          <div class="dot" data-goto="10"></div>
        </div>
      </div>

      <script>
        let currentSlide = 1;
        const totalSlides = 10;

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
          } else if (e.key >= '1' && e.key <= '9') {
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
