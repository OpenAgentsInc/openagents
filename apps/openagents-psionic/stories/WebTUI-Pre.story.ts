export const title = "WebTUI Pre"
export const component = "Pre"

export const Default = {
  name: "Default Pre",
  html: `<pre is-="pre">This is preformatted text.
  It preserves    spacing
    and line breaks.</pre>`,
  description: "Basic preformatted text block"
}

export const NativeVsAttribute = {
  name: "Native Pre vs Attribute Pre",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div>
        <p style="margin-bottom: 0.5rem;">Native &lt;pre&gt; element:</p>
        <pre>function hello() {
  console.log("Hello, World!");
}</pre>
      </div>
      
      <div>
        <p style="margin-bottom: 0.5rem;">Element with is-="pre" attribute:</p>
        <div is-="pre">function hello() {
  console.log("Hello, World!");
}</div>
      </div>
    </div>
  `,
  description: "Both native pre and attribute-based styling"
}

export const Sizes = {
  name: "Pre Sizes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div>
        <p style="margin-bottom: 0.5rem;">Small size:</p>
        <pre is-="pre" size-="small">Small preformatted text with minimal padding</pre>
      </div>
      
      <div>
        <p style="margin-bottom: 0.5rem;">Default size:</p>
        <pre is-="pre">Default preformatted text with standard padding</pre>
      </div>
    </div>
  `,
  description: "Different pre block sizes"
}

export const BoxStyles = {
  name: "Pre with Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <pre is-="pre" box-="square">Square box pre block
with multiple lines
of content</pre>
      
      <pre is-="pre" box-="round">Round box pre block
with multiple lines
of content</pre>
      
      <pre is-="pre" box-="double">Double box pre block
with multiple lines
of content</pre>
    </div>
  `,
  description: "Pre blocks with box borders"
}

export const CodeExample = {
  name: "Code Examples",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <div>
        <h4 style="margin-bottom: 0.5rem;">JavaScript:</h4>
        <pre is-="pre">// Calculate factorial
function factorial(n) {
  if (n <= 1) return 1;
  return n * factorial(n - 1);
}

console.log(factorial(5)); // Output: 120</pre>
      </div>
      
      <div>
        <h4 style="margin-bottom: 0.5rem;">CSS:</h4>
        <pre is-="pre">/* WebTUI button styles */
[is-~="button"] {
  --button-primary: var(--foreground0);
  --button-secondary: var(--background0);
  
  color: var(--mapped-secondary);
  background-color: var(--mapped-primary);
  padding: 0 2ch;
}</pre>
      </div>
      
      <div>
        <h4 style="margin-bottom: 0.5rem;">Terminal Output:</h4>
        <pre is-="pre" box-="square">$ npm install @webtui/core
added 1 package in 1.234s

$ npm run build
> webtui-demo@1.0.0 build
> vite build

✓ 42 modules transformed.
dist/index.html    1.23 kB
dist/assets/index-abc123.js    15.67 kB
✓ built in 456ms</pre>
      </div>
    </div>
  `,
  description: "Various code examples in pre blocks"
}

export const ASCIIArt = {
  name: "ASCII Art Example",
  html: `
    <pre is-="pre" box-="double">
    __        __   _    _____ _   _ ___ 
    \\ \\      / /__| |__|_   _| | | |_ _|
     \\ \\ /\\ / / _ \\ '_ \\| | | | | || | 
      \\ V  V /  __/ |_) | | | |_| || | 
       \\_/\\_/ \\___|_.__/|_|  \\___/|___|
                                       
         Terminal User Interface       
    </pre>
  `,
  description: "ASCII art in preformatted block"
}

export const DataDisplay = {
  name: "Data Display",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div>
        <h4 style="margin-bottom: 0.5rem;">Configuration File:</h4>
        <pre is-="pre" size-="small">{
  "name": "webtui-app",
  "version": "1.0.0",
  "theme": {
    "primary": "foreground0",
    "secondary": "background1"
  },
  "features": {
    "darkMode": true,
    "animations": false
  }
}</pre>
      </div>
      
      <div>
        <h4 style="margin-bottom: 0.5rem;">Log Output:</h4>
        <pre is-="pre" box-="square">[2024-12-17 15:30:01] INFO  Starting application...
[2024-12-17 15:30:02] INFO  Loading configuration...
[2024-12-17 15:30:02] INFO  Connecting to database...
[2024-12-17 15:30:03] INFO  Database connected successfully
[2024-12-17 15:30:03] INFO  Server listening on port 3000
[2024-12-17 15:30:03] INFO  Application ready</pre>
      </div>
    </div>
  `,
  description: "Pre blocks for data and log display"
}

export const ResponsivePre = {
  name: "Responsive Pre",
  html: `
    <div style="max-width: 400px; overflow-x: auto;">
      <pre is-="pre">This is a very long line of preformatted text that might overflow on smaller screens but should be scrollable horizontally when needed to view all content.</pre>
    </div>
  `,
  description: "Pre block in scrollable container"
}