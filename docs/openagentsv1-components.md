# OpenAgents v1 Component Library

This document contains extracted HTML structures and Tailwind CSS classes from the OpenAgents v1 Laravel/Livewire application. These components have been analyzed and documented for recreation as pure HTML/CSS components.

## Color Scheme

The application uses a dark theme with the following key colors:
- Background: `bg-black`
- Text: `text-white`, `text-gray`, `text-lightgray`, `text-text`
- Borders: `border-darkgray`, `border-gray`, `border-offblack`
- Accents: `text-green`, `text-red`

## Layout Structure

### Main App Layout
```html
<body class="h-full bg-black antialiased">
  <!-- User menu in top-right -->
  <div class="absolute top-4 right-4 h-[52px] z-10 px-5">
    <!-- User menu component -->
  </div>
  
  <!-- Main container -->
  <div class="relative z-0 flex h-full w-full overflow-hidden">
    <!-- Sidebar -->
    <div class="flex-shrink-0 overflow-x-hidden sidebar fixed h-full">
      <div class="relative h-full w-[260px]">
        <!-- Sidebar content -->
      </div>
    </div>
    
    <!-- Main content area -->
    <div class="relative flex h-full max-w-full flex-1 flex-col overflow-hidden">
      <main class="relative h-full w-full flex-1 overflow-auto transition-width z-[1]">
        <!-- Page content -->
      </main>
    </div>
  </div>
</body>
```

## Core Components

### 1. Button Component
```html
<!-- Primary Button -->
<button class="inline-flex items-center px-4 py-2 bg-white border border-transparent rounded-md font-semibold text-black hover:bg-gray-300 focus:bg-gray-300 active:bg-gray-500 focus:outline-none focus:ring-2 focus:ring-black focus:ring-offset-2 transition ease-in-out duration-150">
  Button Text
</button>

<!-- Secondary Button -->
<button class="inline-flex items-center px-4 py-2 bg-black border border-gray-300 dark:border-gray-500 rounded-md font-semibold text-gray-700 dark:text-gray-300 tracking-wide shadow-sm hover:bg-gray-50 dark:hover:bg-gray-700 focus:outline-none disabled:opacity-25 transition ease-in-out duration-150">
  Secondary Button
</button>
```

### 2. Input Component
```html
<input class="border-darkgray bg-black text-white focus:border-white focus:ring-white rounded-md shadow-sm" />
```

### 3. Textarea Component
```html
<div class="relative">
  <textarea 
    class="resize-none flex w-full rounded-md border-2 bg-transparent px-3 py-[0.65rem] pr-10 text-[16px] placeholder:text-[#777A81] focus-visible:outline-none focus-visible:ring-0 focus-visible:border-white focus-visible:ring-white border-[#3D3E42] transition-all duration-300 ease-in-out"
    placeholder="Message OpenAgents..."
    rows="1">
  </textarea>
</div>
```

### 4. Pane Component (Card-like container)
```html
<div class="bg-black text-text font-mono w-full max-w-[1050px] mx-auto">
  <div class="border-text border-2 relative pt-[18px] px-[16px] pb-[14px] mb-5">
    <div class="select-none flex justify-between items-center">
      <div class="absolute text-lg font-bold top-[-15px] left-[6px] bg-black px-2.5">Title</div>
      <div class="text-text text-sm absolute top-[-12px] right-[6px] bg-black px-2.5">Subtitle</div>
    </div>
    <div class="pt-[2px]">
      <!-- Content -->
    </div>
  </div>
</div>
```

### 5. Chat Interface

#### Chat Container
```html
<div role="presentation" tabindex="0" class="flex flex-col h-full min-h-screen">
  <div class="flex-1 overflow-hidden">
    <!-- Chat header -->
    <div class="h-[52px] sticky top-0 flex flex-row items-center justify-between z- px-5 z-10 bg-black">
      <!-- Model selector -->
    </div>
    
    <!-- Messages area -->
    <div class="w-full overflow-y-auto flex flex-col items-center">
      <div class="w-full prose prose-invert messages max-w-4xl flex flex-col text-sm pb-9">
        <div class="xl:-ml-[50px] pt-8 chat">
          <!-- Chat messages -->
        </div>
      </div>
    </div>
  </div>
  
  <!-- Input area -->
  <div class="w-full lg:-ml-[25px] px-3">
    <div class="sm:w-[584px] lg:w-[768px] mx-auto">
      <!-- Chat input form -->
    </div>
  </div>
</div>
```

#### Chat Message
```html
<div class="z-[-1] w-full text-lightgray">
  <div class="px-1 justify-center text-base md:gap-4 m-auto">
    <div class="flex flex-1 text-base mx-auto gap-3 md:px-5 lg:px-1 xl:px-5 md:max-w-3xl lg:max-w-[800px]">
      <!-- Avatar -->
      <div class="flex-shrink-0 flex flex-col relative items-end not-prose">
        <div class="m-[2px] w-[28px] p-[2px] border border-darkgray rounded">
          <!-- User icon or avatar image -->
        </div>
      </div>
      
      <!-- Message content -->
      <div class="relative flex w-full flex-col items-start">
        <span class="mb-1 font-semibold select-none text-white">Author Name</span>
        <div class="flex-col">
          <div class="-mt-4 flex flex-grow flex-col max-w-[936px]">
            <div class="text-md text-text markdown-content">
              Message content here
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</div>
```

### 6. Model/Agent Dropdown
```html
<div class="flex gap-1 items-center justify-center overflow-hidden" role="button" tabindex="0">
  <div class="flex flex-row gap-3 items-center select-none">
    <img src="model-image.png" class="w-6 h-6">
    <span class="my-0 text-[18px]">Model Name</span>
  </div>
  
  <div class="relative flex-1 text-right bg-black">
    <button class="p-1.5 rounded-md text-white hover:bg-gray-50 active:bg-gray-100 focus:outline-none">
      <!-- Expand icon -->
    </button>
    
    <!-- Dropdown menu -->
    <div class="mt-3 -ml-[125px] fixed z-[50] divide-y divide-white/15 min-w-60 shadow-md rounded-lg p-2 bg-black border border-white/25 overflow-y-scroll overflow-x-hidden max-h-[80vh] sm:max-h-screen">
      <!-- Menu items -->
      <a class="flex items-center gap-x-3.5 py-1 px-3 rounded-lg text-sm focus:ring-2 focus:ring-gray-500 text-gray-400 hover:text-gray-400 hover:bg-white/15">
        <img src="item-image.png" class="w-6 h-6">
        <div class="flex flex-col">
          <span class="text-indigo-50 my-0 text-sm">Item Name</span>
        </div>
      </a>
    </div>
  </div>
</div>
```

### 7. Wallet Screen Layout
```html
<div class="p-4 md:p-12 mx-auto flex flex-col justify-center w-full items-center">
  <div class="w-full md:max-w-3xl md:min-w-[600px]">
    <h3 class="mb-16 font-bold text-3xl text-center select-none">Wallet</h3>
    
    <!-- Balance pane -->
    <div class="bg-black text-text font-mono w-full max-w-[1050px] mx-auto">
      <div class="border-text border-2 relative pt-[18px] px-[16px] pb-[14px] mb-5">
        <!-- Balance content -->
        <table class="w-full">
          <tr>
            <th class="text-center">Available</th>
            <th class="text-center">Pending</th>
          </tr>
          <tr>
            <td class="text-center">0 sats</td>
            <td class="text-center">0 sats</td>
          </tr>
        </table>
        
        <div class="px-4 mt-6 pt-2 flex justify-evenly">
          <button>Withdraw</button>
          <button>Deposit</button>
        </div>
      </div>
    </div>
  </div>
</div>
```

### 8. Agent Creation Form
```html
<div class="mt-10 p-5 y-5 mx-auto w-full max-w-5xl md:max-w-[800px]">
  <h1 class="text-md md:text-3xl font-bold my-6 md:mb-10 text-center">Create an agent</h1>
  
  <form>
    <!-- Image upload -->
    <div class="col-span-full flex items-center gap-x-8 my-5">
      <img src="placeholder.jpg" class="h-24 w-24 flex-none rounded-lg bg-gray-800 object-cover">
      <div>
        <button type="button" class="rounded-md bg-white/10 px-3 py-2 text-sm font-semibold text-white shadow-sm hover:bg-white/20">
          Change image
        </button>
        <p class="mt-2 text-xs leading-5 text-gray-400">JPG, PNG. 2MB max.</p>
      </div>
    </div>
    
    <!-- Form fields -->
    <div class="mt-5">
      <label for="name">Name</label>
      <input id="name" class="block mt-1 w-full" type="text" placeholder="Name your agent" />
    </div>
    
    <div class="mt-5">
      <label for="about">Description</label>
      <textarea placeholder="Add a short description" class="block mt-1 w-full" rows="3"></textarea>
    </div>
    
    <!-- Submit button -->
    <div class="mt-5 w-full text-center">
      <button type="submit" class="text-center justify-center gap-2 py-2 my-4">
        Create Agent
      </button>
    </div>
  </form>
</div>
```

### 9. Modal Component
```html
<div class="fixed inset-0 z-10 overflow-y-auto">
  <!-- Background overlay -->
  <div class="fixed inset-0 transition-all transform">
    <div class="absolute inset-0 bg-black opacity-75"></div>
  </div>
  
  <!-- Modal content -->
  <div class="p-3 border border-[#3C3E42] inline-block w-full align-bottom bg-black rounded-[12px] text-left overflow-hidden shadow-xl transform transition-all sm:my-8 sm:align-middle sm:w-full">
    <!-- Modal body content -->
  </div>
</div>
```

### 10. Badge/Indicator
```html
<!-- Pro badge -->
<span class="bg-opacity-15 bg-white rounded-md px-1 py-1 text-gray-500 text-sm flex justify-center items-center w-[56px] h-[20px]">
  <svg class="w-[12px] h-[12px] mr-[4px]"><!-- Logo icon --></svg>
  Pro
</span>

<!-- Free badge -->
<span class="bg-opacity-15 bg-white rounded-md px-2 py-1 text-green text-sm flex justify-center items-center w-[44px] h-[20px]">
  Free
</span>
```

## Common Patterns

### Spacing
- Small padding: `p-1`, `px-2`, `py-1`
- Medium padding: `p-4`, `px-4`, `py-2`
- Large padding: `p-5`, `px-5`, `py-4`
- Margins follow similar patterns

### Typography
- Headings: `text-3xl font-bold`, `text-md`
- Body text: `text-sm`, `text-base`
- Small text: `text-xs`
- Colors: `text-white`, `text-gray`, `text-lightgray`

### Borders
- Standard: `border border-darkgray`
- Thick: `border-2`
- Rounded: `rounded-md`, `rounded-lg`, `rounded-full`

### Layout
- Centered content: `mx-auto`
- Max widths: `max-w-3xl`, `max-w-4xl`, `max-w-5xl`
- Responsive widths: `w-full`, `sm:w-[584px]`, `lg:w-[768px]`

### Interactive States
- Hover: `hover:bg-gray-50`, `hover:bg-white/15`
- Focus: `focus:border-white`, `focus:ring-white`
- Active: `active:bg-gray-100`
- Disabled: `disabled:opacity-25`

## Icons
The application uses custom SVG icons wrapped in Blade components. Common icons include:
- User
- Menu (hamburger)
- Plus
- Send
- Logo
- Expand/chevron down
- Close/X

## Z-Index Hierarchy
- Modals: `z-10`
- Dropdowns: `z-[50]`
- Header elements: `z-10`
- Content: `z-[-1]` to `z-[1]`