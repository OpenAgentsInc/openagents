# The app icon

The big E of the Everglade logo
([`icon.svg`](../../openagents-web/realms/everglade2/domain/icon.svg)), gold on
the logo's green, with the green lighter behind the letter.

| Source | Becomes |
| --- | --- |
| [`ios.svg`](ios.svg) | `ios/Assets.xcassets/AppIcon.appiconset/icon-1024.png`. Square and opaque: iOS rounds the corners |
| [`android-foreground.svg`](android-foreground.svg) | `android/res/mipmap-*/ic_launcher_foreground.png`. The E sits inside the middle 66 of 108, which every launcher's mask keeps |
| [`android-monochrome.svg`](android-monochrome.svg) | `android/res/mipmap-*/ic_launcher_monochrome.png`, for Android's themed icons |

The Android background is `android/res/drawable/ic_launcher_background.xml`.

The pictures are checked in, so a build needs no SVG tool. To draw them again
after changing a source, with `rsvg-convert` (`brew install librsvg`):

```sh
cd apps/everglade-mobile
rsvg-convert -w 1024 -h 1024 -b '#16301c' icon/ios.svg -o ios/Assets.xcassets/AppIcon.appiconset/icon-1024.png
for d in mdpi:108 hdpi:162 xhdpi:216 xxhdpi:324 xxxhdpi:432; do
  rsvg-convert -w ${d##*:} -h ${d##*:} icon/android-foreground.svg -o android/res/mipmap-${d%%:*}/ic_launcher_foreground.png
  rsvg-convert -w ${d##*:} -h ${d##*:} icon/android-monochrome.svg -o android/res/mipmap-${d%%:*}/ic_launcher_monochrome.png
done
```
