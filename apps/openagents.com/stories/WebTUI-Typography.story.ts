export const title = "WebTUI Typography"
export const component = "Typography"

export const Headings = {
  name: "Headings",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <h1>Heading Level 1</h1>
      <h2>Heading Level 2</h2>
      <h3>Heading Level 3</h3>
      <h4>Heading Level 4</h4>
      <h5>Heading Level 5</h5>
      <h6>Heading Level 6</h6>
    </div>
  `,
  description: "All heading levels with automatic # prefixes"
}

export const HeadingsWithCode = {
  name: "Headings with Code",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <h1>Installing <code>@webtui/core</code></h1>
      <h2>The <code>button</code> Component</h2>
      <h3>Using <code>is-="button"</code> Attribute</h3>
    </div>
  `,
  description: "Headings containing inline code"
}

export const Paragraphs = {
  name: "Paragraphs",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <p>This is a regular paragraph with standard text. It demonstrates the basic typography settings including font family, size, and line height.</p>
      
      <p>Paragraphs can contain <strong>bold text</strong> for emphasis, <a href="#">links</a> to other content, and <code>inline code</code> for technical references.</p>
      
      <p>Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.</p>
    </div>
  `,
  description: "Basic paragraph styling with inline elements"
}

export const Lists = {
  name: "Lists",
  html: `
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 2rem;">
      <div>
        <h4>Unordered Lists</h4>
        <ul>
          <li>Default dash marker</li>
          <li>Second item</li>
          <li>Third item with <code>code</code></li>
          <li>Fourth item with <strong>bold</strong></li>
        </ul>
        
        <h4 style="margin-top: 1rem;">Bullet Marker</h4>
        <ul marker-="bullet">
          <li>Bullet point one</li>
          <li>Bullet point two</li>
          <li>Bullet point three</li>
        </ul>
      </div>
      
      <div>
        <h4>Ordered List</h4>
        <ol>
          <li>First numbered item</li>
          <li>Second numbered item</li>
          <li>Third numbered item</li>
          <li>Fourth numbered item</li>
        </ol>
      </div>
    </div>
  `,
  description: "Different list styles"
}

export const NestedLists = {
  name: "Nested Lists",
  html: `
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 2rem;">
      <div>
        <h4>Nested Unordered</h4>
        <ul>
          <li>Parent item 1
            <ul>
              <li>Child item 1.1</li>
              <li>Child item 1.2</li>
            </ul>
          </li>
          <li>Parent item 2
            <ul>
              <li>Child item 2.1</li>
              <li>Child item 2.2
                <ul>
                  <li>Grandchild 2.2.1</li>
                  <li>Grandchild 2.2.2</li>
                </ul>
              </li>
            </ul>
          </li>
        </ul>
      </div>
      
      <div>
        <h4>Nested Ordered</h4>
        <ol>
          <li>Chapter 1
            <ol>
              <li>Section 1.1</li>
              <li>Section 1.2</li>
            </ol>
          </li>
          <li>Chapter 2
            <ol>
              <li>Section 2.1</li>
              <li>Section 2.2</li>
            </ol>
          </li>
        </ol>
      </div>
    </div>
  `,
  description: "Lists with nested items"
}

export const TreeLists = {
  name: "Tree Style Lists",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <div>
        <h4>Tree List (Default)</h4>
        <ul marker-="tree">
          <li>Root directory</li>
          <li>src folder</li>
          <li>package.json</li>
          <li>README.md</li>
        </ul>
      </div>
      
      <div>
        <h4>Tree List (Open Start)</h4>
        <ul marker-="open-tree">
          <li>components</li>
          <li>utils</li>
          <li>styles</li>
          <li>index.ts</li>
        </ul>
      </div>
      
      <div>
        <h4>Tree List (Open End)</h4>
        <ul marker-="tree-open">
          <li>First item</li>
          <li>Second item</li>
          <li>Third item</li>
          <li>More items...</li>
        </ul>
      </div>
    </div>
  `,
  description: "Tree-style lists for file structures"
}

export const Blockquotes = {
  name: "Blockquotes",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1.5rem;">
      <blockquote>
        <p>This is a simple blockquote. It has a vertical line on the left side to indicate quoted content.</p>
      </blockquote>
      
      <blockquote>
        <p>Blockquotes can contain multiple paragraphs. This is the first paragraph.</p>
        <p>This is the second paragraph within the same blockquote. The vertical line extends to cover all content.</p>
      </blockquote>
      
      <blockquote>
        <p>Blockquotes can also contain <strong>formatted text</strong>, <code>inline code</code>, and <a href="#">links</a>.</p>
        <p>— <em>Attribution Source</em></p>
      </blockquote>
    </div>
  `,
  description: "Blockquote styling with border indicator"
}

export const InlineElements = {
  name: "Inline Elements",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <p>Text can be <strong>bold for emphasis</strong> or contain <code>inline code snippets</code>.</p>
      
      <p>Links are <a href="#">underlined by default</a> and can appear <a href="#"><strong>bold</strong></a> or with <a href="#"><code>code</code></a>.</p>
      
      <p>You can combine elements: <strong>Bold text with <code>inline code</code> inside</strong>.</p>
      
      <p>Code blocks can contain longer snippets: <code>const result = items.filter(item => item.active)</code></p>
    </div>
  `,
  description: "Inline text formatting elements"
}

export const ComplexDocument = {
  name: "Complex Document Example",
  html: `
    <article style="max-width: 60ch;">
      <h1>WebTUI Documentation</h1>
      
      <p>WebTUI is a terminal-inspired CSS framework that uses <strong>attribute-based selectors</strong> instead of classes. This approach provides a unique and semantic way to style HTML elements.</p>
      
      <h2>Getting Started</h2>
      
      <p>To use WebTUI components, apply the appropriate attributes to your HTML elements:</p>
      
      <pre is-="pre" box-="square">npm install @webtui/core
import '@webtui/core/full.css'</pre>
      
      <h3>Basic Usage</h3>
      
      <p>Here's how to create a simple button:</p>
      
      <blockquote>
        <p>Remember: WebTUI uses attributes like <code>is-="button"</code> instead of class names.</p>
      </blockquote>
      
      <h3>Component List</h3>
      
      <p>WebTUI includes the following components:</p>
      
      <ul marker-="tree">
        <li>Form Components
          <ul>
            <li>Input</li>
            <li>Textarea</li>
            <li>Checkbox</li>
            <li>Radio</li>
            <li>Switch</li>
          </ul>
        </li>
        <li>Interactive Components
          <ul>
            <li>Button</li>
            <li>Badge</li>
            <li>Dialog</li>
            <li>Popover</li>
            <li>Tooltip</li>
          </ul>
        </li>
        <li>Display Components
          <ul>
            <li>Table</li>
            <li>Separator</li>
            <li>Pre</li>
            <li>Typography</li>
          </ul>
        </li>
      </ul>
      
      <h2>Advanced Features</h2>
      
      <ol>
        <li>Theme customization through CSS variables</li>
        <li>Box border styles (square, round, double)</li>
        <li>Size variants for most components</li>
        <li>Built-in dark mode support</li>
      </ol>
      
      <p>For more information, visit the <a href="#">official documentation</a>.</p>
    </article>
  `,
  description: "Complete document showcasing all typography elements"
}

export const TypographyBlock = {
  name: "Typography Block Attribute",
  html: `
    <div style="display: flex; flex-direction: column; gap: 1rem;">
      <div is-="typography-block">
        This div uses the typography-block attribute to apply paragraph-like styling to a non-paragraph element.
      </div>
      
      <section is-="typography-block">
        <strong>Section content</strong> can also use typography styling with proper <code>font-family</code>, <code>font-size</code>, and <code>line-height</code>.
      </section>
      
      <span is-="typography-block" style="display: block;">
        Even inline elements can be styled as typography blocks when needed.
      </span>
    </div>
  `,
  description: "Using typography-block attribute for custom elements"
}