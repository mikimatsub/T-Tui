# Demo faces

These 19 portraits are used only to capture the README tour and screenshots.
They are **not** shipped in the binary, npm package or release archives; a
normal `ttui --mock` still draws generated gradient placeholders.

## Provenance

- Source: [thispersondoesnotexist.com](https://thispersondoesnotexist.com/),
  which serves StyleGAN2 images of people who do not exist. Downloaded
  2026-10-28, one image per request.
- Every image depicts a synthetic adult. None shows a real person, and none
  comes from Tinder or any personal photo.
- Edits: center-cropped from 1024×1024 to the app's 4:5 portrait ratio,
  resized to 640×800 and saved as JPEG.
- The StyleGAN2 model is published by NVIDIA under the
  [NVIDIA Source Code License](https://github.com/NVlabs/stylegan2/blob/master/LICENSE.txt);
  generated images are used here for illustration only.

## Capturing README media

`mock-user-N.jpg` maps to demo profile `N` (`mock-own-user.jpg` is your own
demo profile). Point the demo at this folder; later photos in a profile are
flipped or zoomed variants:

```sh
TTUI_DEMO_PHOTOS=docs/demo-faces ttui --mock
```

Use a terminal with Kitty or Sixel graphics so the photos render as pixels.
