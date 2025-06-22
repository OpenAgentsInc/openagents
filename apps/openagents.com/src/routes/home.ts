import { document, html } from "@openagentsinc/psionic"
import { sharedHeader } from "../components/shared-header"
import { baseStyles } from "../styles"

export async function home() {
  return document({
    title: "OpenAgents",
    styles: baseStyles,
    body: html`
      <!-- Fixed Layout Container -->
      <div class="fixed w-screen h-screen flex flex-col overflow-hidden pt-20">
        ${sharedHeader({ current: "home" })}

        <!-- Main Content -->
        <main class="flex-1 flex items-center justify-center p-8 overflow-hidden">
          <div class="oa-card oa-card-shadow text-center py-12 px-16 min-w-[400px] max-w-[600px] -mt-12">
            <div class="text-3xl font-bold text-white mb-8">Welcome to OpenAgents</div>
            <div class="text-gray-300">
              <p class="my-4 leading-relaxed">We're announcing a few new products today at <a href="https://bitcoinfor.ai/" target="_blank" class="text-blue-400 font-semibold hover:underline">Bitcoin for AI</a> at 5pm CT.</p>
              <p class="my-4 leading-relaxed">In the meantime, explore our resources:</p>
              <div class="flex gap-6 justify-center mt-10">
                <a href="/blog" class="oa-button-primary">
                  Read our blog →
                </a>
                <a href="/docs" class="oa-button-primary">
                  View the docs →
                </a>
              </div>
            </div>
          </div>
        </main>
      </div>

      <style type="text/tailwindcss">
        /* Base styles for html/body to work with Tailwind */
        @layer base {
          html, body {
            @apply m-0 p-0 h-screen overflow-hidden fixed w-full;
          }
        }
        
        /* Responsive overrides */
        @media (max-width: 768px) {
          .box-terminal {
            @apply py-8 px-8 min-w-[250px];
          }
          
          .box-terminal > div:first-child {
            @apply text-2xl;
          }
          
          .box-terminal .flex {
            @apply flex-col gap-4;
          }
        }
      </style>
    `
  })
}
