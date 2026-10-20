# 0010: Stage look for the start page

- Status: Accepted
- Date: 2026-10-20

## Context

The start page draws the laptop keyboard with each key's note, so the player
can see what they play. Three candidate looks were built and played side by
side, with Space switching between them:

- Glass: light frosted glass over drifting color, keys that spring back.
- Tonal: flat tinted surfaces where the lowest note picks the page's color.
- Stage: a dark grid of dots lit in amber, like the dot-matrix display on a
  piece of studio gear.

## Decision

The start page uses Stage. Glass, Tonal and Space switching are removed.

- The page is a black grid of dots. Held keys light up amber at once and
  fade out slowly when let go.
- Every note sends a beam of light up the grid from its key, which keeps
  rising after the key is let go.
- The held notes show above the keyboard in big Doto dot-matrix letters.
- The page powers on from the top down, then each key flashes once.
- The top bar is shaped like GitHub's, with Open Sans for text and Doto for
  notes and labels. No pill shapes.
- The code is `ui/stage.ts` and `ui/stage.css`.

## Consequences

- Space plays nothing again, so it stays free for the layer keys.
- Later screens should follow Stage's dark, dot-matrix style.
- The fonts still load from Google Fonts. They must ship inside the app,
  after a license check against ADR 0007, so the app works offline.
