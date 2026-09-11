# Social preview

The 1280 × 640 PNG is rendered from [social-preview.svg](social-preview.svg).
The SVG retains the existing repository card's purple palette, border and
type hierarchy. It contains text and geometry only.

Copy source: AGENTS.md and README.md at commit 7171e5418317e02d9621028d4f705c16fe3ba852; functions.csv has 138 named entries, including five help tables.
Checked 11 September 2026. No package version, accounting rule or source-review
date changes in this card.

Render with Node.js and Sharp 0.35.4 available in a development environment,
using Windows Segoe UI and Consolas fonts:

```bash
node -e "require('sharp')('.github/social-preview.svg').png().toFile('.github/social-preview.png')"
```

Open the PNG and check its text and 1280 × 640 dimensions before use. Font
substitution on another platform can change its appearance.

GitHub stores its social preview separately from repository files. After
review, upload [social-preview.png](social-preview.png) under Settings,
General, Social preview. Merging this file does not update that setting.
