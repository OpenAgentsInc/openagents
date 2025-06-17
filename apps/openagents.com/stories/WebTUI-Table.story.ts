export const title = "WebTUI Table"
export const component = "Table"

export const Default = {
  name: "Default Table",
  html: `
    <table>
      <tr>
        <th>Name</th>
        <th>Role</th>
        <th>Status</th>
      </tr>
      <tr>
        <td>John Doe</td>
        <td>Developer</td>
        <td>Active</td>
      </tr>
      <tr>
        <td>Jane Smith</td>
        <td>Designer</td>
        <td>Active</td>
      </tr>
      <tr>
        <td>Bob Johnson</td>
        <td>Manager</td>
        <td>Away</td>
      </tr>
    </table>
  `,
  description: "Basic table with WebTUI styling"
}

export const WithSemanticHTML = {
  name: "Table with Semantic HTML",
  html: `
    <table>
      <thead>
        <tr>
          <th>Product</th>
          <th>Price</th>
          <th>Stock</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>Widget A</td>
          <td>$19.99</td>
          <td>In Stock</td>
        </tr>
        <tr>
          <td>Widget B</td>
          <td>$29.99</td>
          <td>Limited</td>
        </tr>
        <tr>
          <td>Widget C</td>
          <td>$39.99</td>
          <td>Out of Stock</td>
        </tr>
      </tbody>
    </table>
  `,
  description: "Table with thead and tbody sections"
}

export const BoxStyles = {
  name: "Table with Box Styles",
  html: `
    <div style="display: flex; flex-direction: column; gap: 2rem;">
      <table box-="square">
        <tr>
          <th>Square Box</th>
          <th>Value</th>
        </tr>
        <tr>
          <td>Item 1</td>
          <td>100</td>
        </tr>
        <tr>
          <td>Item 2</td>
          <td>200</td>
        </tr>
      </table>
      
      <table box-="round">
        <tr>
          <th>Round Box</th>
          <th>Value</th>
        </tr>
        <tr>
          <td>Item 1</td>
          <td>100</td>
        </tr>
        <tr>
          <td>Item 2</td>
          <td>200</td>
        </tr>
      </table>
      
      <table box-="double">
        <tr>
          <th>Double Box</th>
          <th>Value</th>
        </tr>
        <tr>
          <td>Item 1</td>
          <td>100</td>
        </tr>
        <tr>
          <td>Item 2</td>
          <td>200</td>
        </tr>
      </table>
    </div>
  `,
  description: "Tables with different box border styles"
}

export const DataTable = {
  name: "Data Table Example",
  html: `
    <table>
      <thead>
        <tr>
          <th>ID</th>
          <th>Username</th>
          <th>Email</th>
          <th>Joined</th>
          <th>Status</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>001</td>
          <td>alice</td>
          <td>alice@example.com</td>
          <td>2024-01-15</td>
          <td>Active</td>
        </tr>
        <tr>
          <td>002</td>
          <td>bob</td>
          <td>bob@example.com</td>
          <td>2024-02-20</td>
          <td>Active</td>
        </tr>
        <tr>
          <td>003</td>
          <td>charlie</td>
          <td>charlie@example.com</td>
          <td>2024-03-10</td>
          <td>Inactive</td>
        </tr>
        <tr>
          <td>004</td>
          <td>diana</td>
          <td>diana@example.com</td>
          <td>2024-04-05</td>
          <td>Active</td>
        </tr>
      </tbody>
    </table>
  `,
  description: "Typical data table layout"
}

export const CompactTable = {
  name: "Compact Table",
  html: `
    <table style="font-size: 0.9em;">
      <tr>
        <th>Key</th>
        <th>Value</th>
      </tr>
      <tr>
        <td>Version</td>
        <td>1.0.0</td>
      </tr>
      <tr>
        <td>License</td>
        <td>MIT</td>
      </tr>
      <tr>
        <td>Downloads</td>
        <td>1,234</td>
      </tr>
      <tr>
        <td>Stars</td>
        <td>456</td>
      </tr>
    </table>
  `,
  description: "Compact table for metadata display"
}

export const ResponsiveTable = {
  name: "Responsive Table Container",
  html: `
    <div style="overflow-x: auto;">
      <table>
        <thead>
          <tr>
            <th>Component</th>
            <th>Version</th>
            <th>Size</th>
            <th>Dependencies</th>
            <th>License</th>
            <th>Last Updated</th>
          </tr>
        </thead>
        <tbody>
          <tr>
            <td>@webtui/core</td>
            <td>2.1.0</td>
            <td>12.4 KB</td>
            <td>0</td>
            <td>MIT</td>
            <td>2024-12-01</td>
          </tr>
          <tr>
            <td>@webtui/components</td>
            <td>2.1.0</td>
            <td>48.2 KB</td>
            <td>1</td>
            <td>MIT</td>
            <td>2024-12-01</td>
          </tr>
          <tr>
            <td>@webtui/themes</td>
            <td>1.5.0</td>
            <td>8.7 KB</td>
            <td>0</td>
            <td>MIT</td>
            <td>2024-11-15</td>
          </tr>
        </tbody>
      </table>
    </div>
  `,
  description: "Table in scrollable container for responsive layouts"
}

export const ComplexTable = {
  name: "Complex Table with Footer",
  html: `
    <table>
      <thead>
        <tr>
          <th>Item</th>
          <th>Quantity</th>
          <th>Price</th>
          <th>Total</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td>Product A</td>
          <td style="text-align: right;">2</td>
          <td style="text-align: right;">$10.00</td>
          <td style="text-align: right;">$20.00</td>
        </tr>
        <tr>
          <td>Product B</td>
          <td style="text-align: right;">1</td>
          <td style="text-align: right;">$25.00</td>
          <td style="text-align: right;">$25.00</td>
        </tr>
        <tr>
          <td>Product C</td>
          <td style="text-align: right;">3</td>
          <td style="text-align: right;">$15.00</td>
          <td style="text-align: right;">$45.00</td>
        </tr>
      </tbody>
      <tfoot>
        <tr>
          <th colspan="3">Subtotal</th>
          <td style="text-align: right; font-weight: bold;">$90.00</td>
        </tr>
        <tr>
          <th colspan="3">Tax (10%)</th>
          <td style="text-align: right;">$9.00</td>
        </tr>
        <tr>
          <th colspan="3">Total</th>
          <td style="text-align: right; font-weight: bold;">$99.00</td>
        </tr>
      </tfoot>
    </table>
  `,
  description: "Table with footer for calculations"
}