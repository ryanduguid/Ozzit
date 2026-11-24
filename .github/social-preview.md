# Social preview

The 1280 × 640 PNG is rendered from [social-preview.svg](social-preview.svg) in the style of the README banners: a black card inside a dark gold double frame, with a `github.com/ryanduguid/Ozzit` tab at the top and a `duguid.com.au` tab at the bottom. The lambda emblem is drawn as a pixel grid with a dithered Tyrian purple glow, beside the gold block wordmark, a rule and the tagline the README banners carry. All lettering is converted to vector paths from Cascadia Mono and Besley (SIL Open Font Licence 1.1), so the SVG needs no fonts; its `<title>` carries the tagline as text.

Rendered on 24 November 2026 with resvg 0.48.1. The uploaded social preview should be byte-identical to this PNG.

| Asset | SHA-256 |
| --- | --- |
| `social-preview.png` | `9c90fcf39079806415cbe491c5a7c7c083c575a2320e788f00df006e70bcacd5` |

The image is uploaded through the repository's Settings page; GitHub has no API for social previews. `ozzit verify_counts` holds the SVG's `<title>` to the newest release's function count, so re-render, upload and update the hash when that count, the card text or the palette changes.
