// The beams of light on the Stage page. Every note sends a beam up the grid
// of dots from its key, and the beam keeps rising after the key is let go, so
// the last notes played stay in sight for a moment. They are drawn on a
// canvas behind everything else, in the screen's own pixels.

// The grid's dots are this many pixels apart, both ways, with one in the
// middle of every square. It must match the grid in stage.css.
const CELL = 12;
// How fast a beam rises, in pixels per millisecond.
const RISE_SPEED = 0.16;
// How far a beam rises before it has faded away, in pixels.
const FADE_DISTANCE = 520;
// How bright a beam's two side columns of dots are, next to its middle one.
const SIDE_BRIGHTNESS = 0.4;
// How wide one lit dot is with its glow, in pixels.
const GLOW_SIZE = 24;

// How ui/Stage.svelte drives the beams.
export interface Beams {
  // Makes the canvas `width` by `height` page pixels.
  fit(width: number, height: number): void;
  // Key `code` went down: a beam starts rising from `startY`, on the column
  // of dots nearest `keyMiddle`. Both are in page pixels.
  start(code: string, keyMiddle: number, startY: number): void;
  // Key `code` came back up: its beam lets go of the panel and floats away.
  end(code: string): void;
}

// The beam of light one key press sends up.
interface Beam {
  // The key that sent it, like "KeyQ".
  code: string;
  // The middle of its middle column of dots, across the page.
  x: number;
  // Where it rises from: the top edge of the keyboard's panel.
  startY: number;
  // When its key went down, and when it came back up (null while held).
  pressedAt: number;
  releasedAt: number | null;
}

// One lit dot with its glow, drawn once and then stamped for every lit dot.
// `scale` is how many screen pixels make one page pixel, so it stays sharp.
function drawGlow(scale: number): HTMLCanvasElement {
  const glow = document.createElement("canvas");
  const size = Math.ceil(GLOW_SIZE * scale);
  glow.width = size;
  glow.height = size;
  const context = glow.getContext("2d");
  if (context === null) {
    return glow;
  }

  const middle = size / 2;
  const light = context.createRadialGradient(middle, middle, 0, middle, middle, middle);
  // A hot, nearly white middle, a little bigger than the grid's dots...
  light.addColorStop(0, "rgba(255, 240, 214, 1)");
  light.addColorStop(0.12, "rgba(255, 196, 112, 1)");
  // ...in a soft amber haze.
  light.addColorStop(0.2, "rgba(255, 176, 72, 0.5)");
  light.addColorStop(1, "rgba(255, 176, 72, 0)");
  context.fillStyle = light;
  context.fillRect(0, 0, size, size);
  return glow;
}

export function createBeams(canvas: HTMLCanvasElement): Beams {
  const context = canvas.getContext("2d");
  let glow = drawGlow(window.devicePixelRatio);
  let width = 0;
  let height = 0;
  // The beams still in sight.
  let beams: Beam[] = [];
  // The next frame asked for, or 0 when nothing is moving.
  let frame = 0;

  function startDrawing() {
    if (frame !== 0) {
      return;
    }
    frame = window.requestAnimationFrame(drawFrame);
  }

  // Draws one frame, and asks for the next while any beam is in sight.
  function drawFrame() {
    drawBeams(performance.now());
    frame = 0;
    if (beams.length > 0) {
      frame = window.requestAnimationFrame(drawFrame);
    }
  }

  // Draws every beam where it is at `now`, and forgets the ones that have
  // faded away.
  function drawBeams(now: number) {
    if (context === null) {
      beams = [];
      return;
    }
    context.clearRect(0, 0, width, height);
    // Where beams cross, their light adds up.
    context.globalCompositeOperation = "lighter";

    const inSight: Beam[] = [];
    for (const beam of beams) {
      // A beam's top rises from the moment its key went down. Its bottom
      // stays on the panel until the key comes back up, then rises too.
      const top = beam.startY - (now - beam.pressedAt) * RISE_SPEED;
      let bottom = beam.startY;
      if (beam.releasedAt !== null) {
        bottom = beam.startY - (now - beam.releasedAt) * RISE_SPEED;
      }
      if (beam.startY - bottom >= FADE_DISTANCE) {
        continue;
      }
      inSight.push(beam);
      drawBeam(context, beam, top, bottom);
    }
    beams = inSight;
  }

  // Lights the grid's dots between `top` and `bottom` in the beam's three
  // columns, each row dimmer the higher it has risen.
  // `pen` is the canvas's drawing context.
  function drawBeam(pen: CanvasRenderingContext2D, beam: Beam, top: number, bottom: number) {
    // Nothing above where beams fade out, or above the page, needs drawing.
    const highest = Math.max(top, beam.startY - FADE_DISTANCE, 0);
    const firstRow = Math.ceil((highest - CELL / 2) / CELL);
    const lastRow = Math.floor((bottom - CELL / 2) / CELL);

    for (let row = firstRow; row <= lastRow; row += 1) {
      const y = row * CELL + CELL / 2;
      const brightness = 1 - (beam.startY - y) / FADE_DISTANCE;
      stampGlow(pen, beam.x, y, brightness);
      stampGlow(pen, beam.x - CELL, y, brightness * SIDE_BRIGHTNESS);
      stampGlow(pen, beam.x + CELL, y, brightness * SIDE_BRIGHTNESS);
    }
  }

  function stampGlow(pen: CanvasRenderingContext2D, x: number, y: number, brightness: number) {
    pen.globalAlpha = brightness;
    pen.drawImage(glow, x - GLOW_SIZE / 2, y - GLOW_SIZE / 2, GLOW_SIZE, GLOW_SIZE);
  }

  return {
    // Resizing a canvas wipes it, and the next frame draws the beams again.
    fit(newWidth, newHeight) {
      const scale = window.devicePixelRatio;
      width = newWidth;
      height = newHeight;
      canvas.width = Math.round(width * scale);
      canvas.height = Math.round(height * scale);
      glow = drawGlow(scale);
      if (context !== null) {
        context.setTransform(scale, 0, 0, scale, 0, 0);
      }
    },

    start(code, keyMiddle, startY) {
      const column = Math.round((keyMiddle - CELL / 2) / CELL);
      beams.push({
        code,
        x: column * CELL + CELL / 2,
        startY,
        pressedAt: performance.now(),
        releasedAt: null,
      });
      startDrawing();
    },

    end(code) {
      const now = performance.now();
      for (const beam of beams) {
        if (beam.code === code && beam.releasedAt === null) {
          beam.releasedAt = now;
        }
      }
    },
  };
}
