# Pixel Hunter icon

A lime green pixel inside four ivory targeting corners on a charcoal tile.
The mark represents screen pixel tracking and stays recognizable at small sizes.

The transparent RGBA source is `../app-icon.png`, generated with the built-in
imagegen tool. Desktop assets were generated with the project's Tauri CLI:

```powershell
npm exec tauri icon -- src-tauri/app-icon.png --output "$env:TEMP/pixel-hunter-icons"
Get-ChildItem -LiteralPath "$env:TEMP/pixel-hunter-icons" -File | Copy-Item -Destination src-tauri/icons
Copy-Item src-tauri/icons/32x32.png public/app-icon.png
```

Run these commands from the repository root. Copy only desktop files; mobile
assets in the generated subdirectories are not used by this project.

`icon.ico` is an ICO container with RGBA images at 16, 24, 32, 48, 64 and 256 px.
Tauri's `bundle.icon` configuration uses it for the Windows executable and
the native window title bar. The PNG and ICNS assets cover the other desktop
formats. `public/app-icon.png` is the browser favicon.

Generation prompt:

> Create one production desktop application icon for Pixel Hunter, a utility that monitors screen pixels and triggers keyboard actions. A striking minimalist geometric mark: four thick ivory targeting corner brackets surrounding a single vivid lime green square pixel, on a solid deep charcoal rounded square tile. Flat vector-like precision, perfectly centered, symmetric, bold simple shapes optimized to remain legible at 16x16 and 32x32. Tile fills nearly all square canvas, modest rounded corners, transparent outside tile. No letters, no text, no gradient, no shadows, no glow, no extra pixels, no mockup. Single icon only, square 1024x1024.
