export const ChatInput = () => {
  return (
    <fieldset className="flex w-full min-w-0 flex-col-reverse">
      <div className="flex  flex-col  bg-zinc-900  gap-1.5  border-0.5  border-border-300  pl-4  pt-2.5  pr-2.5  pb-2.5  -mx-1  sm:mx-0  items-stretch  transition-all  duration-200  relative  shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.035)]  focus-within:shadow-[0_0.25rem_1.25rem_rgba(0,0,0,0.075)]  hover:border-border-200  focus-within:border-border-200  cursor-text  z-10 rounded-t-2xl border-b-0">
        <div class="flex gap-2">
          <div
            aria-label="Write your prompt to Claude"
            class="mt-1 max-h-96 w-full overflow-y-auto break-words"
          >
            <div
              contenteditable="true"
              translate="no"
              enterkeyhint="enter"
              tabindex="0"
              class="focus:outline-none ProseMirror break-words max-w-[60ch]"
              autofocus=""
            >
              <p
                data-placeholder="Reply to Claude..."
                class="is-empty is-editor-empty before:!text-text-500 before:whitespace-nowrap"
              >
                <br class="ProseMirror-trailingBreak" />
              </p>
            </div>
          </div>
          <input
            data-testid="file-upload"
            aria-hidden="true"
            tabindex="-1"
            class="absolute -z-10 h-0 w-0 overflow-hidden opacity-0"
            accept=".pdf,.doc,.docx,.rtf,.epub,.odt,.odp,.pptx,.txt,.py,.ipynb,.js,.jsx,.html,.css,.java,.cs,.php,.c,.cpp,.cxx,.h,.hpp,.rs,.R,.Rmd,.swift,.go,.rb,.kt,.kts,.ts,.tsx,.m,.scala,.rs,.dart,.lua,.pl,.pm,.t,.sh,.bash,.zsh,.csv,.log,.ini,.config,.json,.proto,.yaml,.yml,.toml,.lua,.sql,.bat,.md,.coffee,.tex,.latex,.jpg,.jpeg,.png,.gif,.webp"
            multiple=""
            aria-label="Upload files"
            type="file"
          />
          <button
            class="inline-flex
  items-center
  justify-center
  relative
  shrink-0
  ring-0
  ring-offset-0
  ring-offset-bg-300
  ring-accent-main-100
  focus:outline-none
  focus-visible:outline-none
  focus-visible:ring-0
  disabled:pointer-events-none
  disabled:opacity-50
  disabled:shadow-none
  disabled:drop-shadow-none
          bg-[radial-gradient(ellipse,_var(--tw-gradient-stops))]
          from-bg-500/10
          from-50%
          to-bg-500/30
          border-0.5
          border-border-400
          font-medium
          font-styrene
          text-text-100/90
          transition-colors
          active:bg-bg-500/50
          hover:text-text-000
          hover:bg-bg-500/60 h-8 w-8 rounded-md active:scale-95 !rounded-xl"
            aria-label="Upload content"
            data-state="closed"
          >
            <svg
              xmlns="http://www.w3.org/2000/svg"
              width="20"
              height="20"
              fill="currentColor"
              viewBox="0 0 256 256"
              class="text-text-300"
            >
              <path d="M209.66,122.34a8,8,0,0,1,0,11.32l-82.05,82a56,56,0,0,1-79.2-79.21L147.67,35.73a40,40,0,1,1,56.61,56.55L105,193A24,24,0,1,1,71,159L154.3,74.38A8,8,0,1,1,165.7,85.6L82.39,170.31a8,8,0,1,0,11.27,11.36L192.93,81A24,24,0,1,0,159,47L59.76,147.68a40,40,0,1,0,56.53,56.62l82.06-82A8,8,0,0,1,209.66,122.34Z"></path>
            </svg>
          </button>
        </div>
      </div>
    </fieldset>
  );
};
