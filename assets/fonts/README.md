# Bundled typefaces

Unmodified official font releases, licensed under the accompanying SIL Open Font License 1.1 files:

- [Inter 4.1](https://github.com/rsms/inter/tree/v4.1): `docs/font-files/InterVariable.ttf`, SHA-256 `4989b125924991b90d05b2d16e0e388c48f7d5bb8b30539bbf9c755278d0ccaf`.
- [Vazirmatn 33.003](https://github.com/rastikerdar/vazirmatn/tree/v33.003): `fonts/variable/Vazirmatn[wght].ttf`, SHA-256 `696249a2c74b39ffdef55de4df2809c5b639d3ff80d618d8160a095d2fd49dca`.

Docker images register these files with Fontconfig inside the container. GTK uses Inter Variable for Latin text and Vazirmatn for Persian/Arabic glyphs, preserving the system font size. No host font installation or network access is needed when running the app. A standalone native binary uses the same family list with system fallbacks when these fonts are absent.
