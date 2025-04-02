# Electron App Icons Guide

This guide explains the required icon formats for an Electron Forge application and how to implement them properly across platforms.

## Required Icon Files

You should provide these icon files in your application's resources directory (e.g., `src/images/`):

| File | Format | Size | Purpose |
|------|--------|------|---------|
| `icon.svg` | SVG | Vector | Source file for all icons |
| `glyph.svg` | SVG | Vector | Simplified icon for menu bar/tray |
| `icon.png` | PNG | 1024×1024 | Main application icon |
| `icon@2x.png` | PNG | 2048×2048 | High-DPI application icon |
| `iconTemplate.png` | PNG | 22×22 | macOS menu bar icon |
| `iconTemplate@2x.png` | PNG | 44×44 | macOS menu bar icon (Retina) |
| `icon.ico` | ICO | Multiple | Windows application icon |
| `icon.icns` | ICNS | Multiple | macOS application icon |

## Generating Icons

You can use the following script to generate all required icons from your SVG source files:

```bash
#!/usr/bin/env sh

# Create template icons for the menu bar
convert -background none -resize 22x22 glyph.svg iconTemplate.png
convert -background none -resize 44x44 glyph.svg iconTemplate@2x.png

# Create main application icons from icon.svg
convert -background none -resize 1024x1024 icon.svg icon.png
convert -background none -resize 2048x2048 icon.svg icon@2x.png

# Create Windows icon (ico) with multiple sizes
convert icon.svg -background none -define icon:auto-resize=256,128,64,48,32,16 icon.ico

# Create macOS icon set (icns)
mkdir -p icon.iconset
convert -background none -resize 16x16 icon.svg icon.iconset/icon_16x16.png
convert -background none -resize 32x32 icon.svg icon.iconset/icon_16x16@2x.png
convert -background none -resize 32x32 icon.svg icon.iconset/icon_32x32.png
convert -background none -resize 64x64 icon.svg icon.iconset/icon_32x32@2x.png
convert -background none -resize 128x128 icon.svg icon.iconset/icon_128x128.png
convert -background none -resize 256x256 icon.svg icon.iconset/icon_128x128@2x.png
convert -background none -resize 256x256 icon.svg icon.iconset/icon_256x256.png
convert -background none -resize 512x512 icon.svg icon.iconset/icon_256x256@2x.png
convert -background none -resize 512x512 icon.svg icon.iconset/icon_512x512.png
convert -background none -resize 1024x1024 icon.svg icon.iconset/icon_512x512@2x.png
iconutil -c icns icon.iconset
rm -rf icon.iconset
```

This script requires ImageMagick (`convert` command) and `iconutil` (macOS).

## Implementation in Electron

### 1. Configure Forge

In your `forge.config.ts` or `forge.config.js`:

```typescript
module.exports = {
  packagerConfig: {
    // Include images folder in resources
    extraResource: ['src/bin', 'src/images'],
    // Set base icon
    icon: 'src/images/icon',
    // Platform-specific icons
    win32: {
      icon: 'src/images/icon.ico',
      // ... other Windows config
    },
    // ... other config
  },
  // ... makers, plugins, etc.
};
```

### 2. Configure BrowserWindow

In your main process file (e.g., `main.ts`):

```typescript
const mainWindow = new BrowserWindow({
  // ... other options
  icon: path.join(__dirname, '../images/icon'), // Electron will choose appropriate format
  // ... other options
});
```

### 3. Configure Tray Icon

For systems tray/menu bar icon:

```typescript
const createTray = () => {
  const isDev = process.env.NODE_ENV === 'development';
  let iconPath: string;

  if (isDev) {
    iconPath = path.join(process.cwd(), 'src', 'images', 'iconTemplate.png');
  } else {
    iconPath = path.join(process.resourcesPath, 'images', 'iconTemplate.png');
  }

  const tray = new Tray(iconPath);
  // ... configure tray
};
```

## Platform-Specific Notes

### macOS
- Use `iconTemplate.png` and `iconTemplate@2x.png` for menu bar icons - the "Template" in the name is special and tells macOS to automatically adapt the icon for light/dark themes
- The `.icns` format is required for the application icon

### Windows
- The `.ico` format is required and should contain multiple resolutions
- Windows requires specific sizes: 16×16, 32×32, 48×48, 64×64, 128×128, and 256×256

### Linux
- Most Linux distributions use PNG icons at various sizes
- Many desktop environments support SVG icons directly

## Best Practices

1. Always start with high-quality vector SVG source files
2. Use transparent backgrounds for better integration across platforms
3. Design a simplified glyph version for small sizes (menu bar/tray)
4. Test your icons on light and dark backgrounds
5. Keep icon visually recognizable at small sizes
