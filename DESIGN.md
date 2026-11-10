# Clipfinch design

Clipfinch is a daily-use, keyboard-oriented clipboard utility. Its GTK4 interface should feel quiet, compact and native: prioritize finding and reusing clips over decoration. Keep motion to native feedback; do not add animation to popup or preview navigation.

## Interface structure

- The history popup leads with pinboards and search, followed by a scrollable list of clips and compact keyboard hints. Make search focus, row selection and hover distinguishable.
- Use bounded thumbnails, readable text hierarchy and wrapping or ellipsizing for long content. Support mixed Persian and English, emoji and all clip kinds without horizontal overflow.
- Settings uses a sidebar with General, Privacy, Shortcuts and System pages. Keep labels, status and errors visible; preserve the history-deletion confirmation. Switches have no decorative border or shadow.
- The queue shares the popup's row language and clearly marks the next item and direction. Keep Reverse, Clear and Stop accessible.
- Provide distinct empty-history and no-results states. Preserve preview, pinboards, drag and drop and keyboard navigation.

## Visual language

Use a restrained neutral light or dark palette matching the system preference. The authoritative colors are defined in `crates/app/src/palette-light.css` and `crates/app/src/palette-dark.css`; reusable GTK styling is in `crates/app/src/style.css`. Keep backgrounds and surfaces quiet, secondary information muted, and the blue-gray accent for focus and interaction. Selected rows have both a surface and boundary; do not rely on color alone to communicate state.

Use a 4/8/12/16/24 logical-pixel spacing rhythm, modest rounding, separators and compact symbolic icons. Preserve inherited system text size. Inter is used for Latin and Vazirmatn for Persian; Docker images bundle the fonts and licenses under `assets/fonts/`. Native builds fall back to installed system fonts. Use monospace for content previews where appropriate.

## Accessibility and validation

Keep keyboard focus visible, icon-only actions named or described, and controls operable by keyboard. Aim for at least 4.5:1 contrast for normal text and 3:1 for meaningful control boundaries. Validate light and dark modes, scaling, long and mixed-direction synthetic content, empty states and navigation. Do not treat palette calculations as a complete rendered-widget accessibility audit.

Use isolated synthetic clipboard data for screenshots and testing. High-contrast themes, screen readers and real-session Wayland behavior need further verification.
