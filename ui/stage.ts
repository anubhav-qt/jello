// The start page, "Stage": a dark stage of dots, lit in amber, like the
// dot-matrix display on a piece of studio gear. A held key lights up at once
// and fades out slowly when let go, like a lamp cooling down. Every note
// sends a beam of light up the grid of dots from its key, and the beam keeps
// rising after the key is let go, so the last notes played stay in sight
// for a moment. The page powers on from the top down.

import { makeElement } from "./dom";
import { drawKeyboard, noteName, showWaveOnKeys, type KeyLayout, type Platform } from "./keyboard";
import { drawTopBar } from "./topbar";

export interface StageSetup {
  platform: Platform;
  layout: KeyLayout;
  // What the top bar's buttons do.
  onWaveClick: () => void;
  onRenderClick: () => void;
}

// How ui/main.ts drives the page.
export interface Stage {
  // Puts the page in `page` and powers it on.
  show(page: HTMLElement): void;
  // A key went down or came back up. `code` is its place, like "KeyQ".
  keyDown(code: string): void;
  keyUp(code: string): void;
  // Every key came up at once: Cmd was pressed, or the window lost focus.
  releaseAll(): void;
  // The wave new notes play, like "saw".
  showWave(wave: string): void;
  // A message for the player, like where a file was saved.
  showStatus(message: string): void;
}

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
// How long the page takes to power on, in milliseconds, and how it moves:
// slow, then fast, then slow.
const POWER_ON_TIME = 520;
const POWER_ON_EASING = "cubic-bezier(0.65, 0, 0.35, 1)";
// How long a message stays up once it is typed out, and how long each
// letter takes to type, in milliseconds.
const STATUS_TIME = 4000;
const LETTER_TIME = 16;

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

// The notes the `held` keys play, lowest first, each once.
function heldNotes(held: Set<string>, layout: KeyLayout): number[] {
  const notes: number[] = [];
  for (const code of held) {
    const note = layout.notes[code];
    if (note === undefined) {
      continue;
    }
    if (!notes.includes(note)) {
      notes.push(note);
    }
  }
  notes.sort((low, high) => low - high);
  return notes;
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

export function createStage(setup: StageSetup): Stage {
  const keyboard = drawKeyboard(setup.platform, setup.layout);
  const topBar = drawTopBar(setup.onWaveClick, setup.onRenderClick);
  // The keys held down right now.
  const held = new Set<string>();
  // The beams still in sight.
  let beams: Beam[] = [];

  const root = makeElement("div", "screen stage");

  // The beams are drawn on a canvas behind everything else, in the screen's
  // own pixels.
  const canvas = document.createElement("canvas");
  canvas.className = "stage-beams";
  const context = canvas.getContext("2d");
  let glow = drawGlow(window.devicePixelRatio);
  let width = 0;
  let height = 0;
  // The next frame asked for, or 0 when nothing is moving.
  let frame = 0;

  // The line of light that sweeps down as the page powers on.
  const scanLine = makeElement("div", "stage-scan-line");

  // The held notes, in big glowing dots.
  const readout = makeElement("div", "stage-readout is-idle");

  // The keyboard sits on a dark panel, like the top of a synth.
  const panel = makeElement("div", "stage-panel");
  panel.append(keyboard.element);

  const body = makeElement("main", "screen-body");
  body.append(readout, panel);

  // Messages are typed out at the bottom left, one letter at a time.
  const ticker = makeElement("p", "stage-ticker");
  const tickerText = makeElement("span", "stage-ticker-text");
  ticker.append(tickerText, makeElement("span", "stage-ticker-cursor"));
  let typingTimer = 0;
  let tickerTimer = 0;

  root.append(canvas, scanLine, topBar.element, body, ticker);

  // Keeps the canvas the size of the page. Resizing a canvas wipes it, and
  // the next frame draws the beams again.
  function fitCanvas() {
    const scale = window.devicePixelRatio;
    width = root.clientWidth;
    height = root.clientHeight;
    canvas.width = Math.round(width * scale);
    canvas.height = Math.round(height * scale);
    glow = drawGlow(scale);
    if (context !== null) {
      context.setTransform(scale, 0, 0, scale, 0, 0);
    }
  }

  const resizeWatcher = new ResizeObserver(fitCanvas);

  function drawReadoutNote(note: number): HTMLElement {
    const name = noteName(note);
    const element = makeElement("span", "stage-note");
    element.dataset.note = String(note);
    element.append(makeElement("span", "stage-note-pitch", name.pitch));
    element.append(makeElement("span", "stage-note-octave", String(name.octave)));
    return element;
  }

  // A press shows the notes held now, lit, at once.
  function showNotesPressed() {
    const notes = heldNotes(held, setup.layout);
    if (notes.length === 0) {
      return;
    }
    const names: HTMLElement[] = [];
    for (const note of notes) {
      names.push(drawReadoutNote(note));
    }
    readout.replaceChildren(...names);
    readout.classList.remove("is-idle");
    root.classList.add("is-playing");
  }

  // While other keys are still held, a let-go note stays in the readout,
  // dimmed, until the next press. Once every key is up, the notes played
  // last fade out together, slowly, so a chord let go can still be read.
  function dimNotesLetGo() {
    const notes = heldNotes(held, setup.layout);
    const names = Array.from(readout.querySelectorAll<HTMLElement>(".stage-note"));
    if (notes.length === 0) {
      for (const name of names) {
        name.classList.remove("is-let-go");
      }
      readout.classList.add("is-idle");
      root.classList.remove("is-playing");
      return;
    }
    for (const name of names) {
      const note = Number(name.dataset.note);
      if (!notes.includes(note)) {
        name.classList.add("is-let-go");
      }
    }
  }

  // A key that plays a note sends a beam up from the panel's top edge, on
  // the column of dots nearest the key's middle.
  function startBeam(code: string) {
    const keyElement = keyboard.keys.get(code);
    if (keyElement === undefined) {
      return;
    }
    if (keyElement.dataset.kind !== "note") {
      return;
    }

    const page = root.getBoundingClientRect();
    const key = keyElement.getBoundingClientRect();
    const keyMiddle = key.left + key.width / 2 - page.left;
    const column = Math.round((keyMiddle - CELL / 2) / CELL);
    const panelTop = panel.getBoundingClientRect().top - page.top;

    beams.push({
      code,
      x: column * CELL + CELL / 2,
      startY: panelTop,
      pressedAt: performance.now(),
      releasedAt: null,
    });
    startDrawing();
  }

  // The key's beam lets go of the panel and floats away.
  function endBeam(code: string) {
    const now = performance.now();
    for (const beam of beams) {
      if (beam.code === code && beam.releasedAt === null) {
        beam.releasedAt = now;
      }
    }
  }

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

  function letGo(code: string) {
    const keyElement = keyboard.keys.get(code);
    if (keyElement !== undefined) {
      keyElement.classList.remove("is-held");
    }
    endBeam(code);
  }

  return {
    show(page) {
      page.append(root);
      fitCanvas();
      resizeWatcher.observe(root);

      // The page powers on from the top down...
      root.animate([{ clipPath: "inset(0 0 100% 0)" }, { clipPath: "inset(0 0 0% 0)" }], {
        duration: POWER_ON_TIME,
        easing: POWER_ON_EASING,
      });

      // ...behind a line of light that sweeps down along its edge, then
      // goes out.
      const bottom = `translateY(${root.clientHeight}px)`;
      scanLine.animate(
        [
          { transform: "translateY(0px)", opacity: 1 },
          { transform: bottom, opacity: 1 },
        ],
        { duration: POWER_ON_TIME, easing: POWER_ON_EASING },
      );
      scanLine.animate(
        [
          { transform: bottom, opacity: 1 },
          { transform: bottom, opacity: 0 },
        ],
        { duration: 220, delay: POWER_ON_TIME, easing: "ease-out" },
      );

      // Then each key flashes once, from left to right, like gear testing
      // its lights as it starts up.
      const keyElements = Array.from(keyboard.element.querySelectorAll<HTMLElement>(".key"));
      for (const keyElement of keyElements) {
        keyElement.animate(
          [
            {
              offset: 0,
              backgroundColor: "rgba(255, 176, 72, 0.28)",
              borderColor: "rgba(255, 176, 72, 0.9)",
              boxShadow: "0 0 14px rgba(255, 176, 72, 0.4)",
            },
          ],
          { duration: 600, delay: 380 + keyElement.offsetLeft * 0.45, easing: "ease-out" },
        );
      }
    },

    keyDown(code) {
      held.add(code);
      const keyElement = keyboard.keys.get(code);
      if (keyElement !== undefined) {
        keyElement.classList.add("is-held");
      }
      startBeam(code);
      showNotesPressed();
    },

    keyUp(code) {
      held.delete(code);
      letGo(code);
      dimNotesLetGo();
    },

    releaseAll() {
      for (const code of held) {
        letGo(code);
      }
      held.clear();
      dimNotesLetGo();
    },

    showWave(wave) {
      topBar.waveName.textContent = wave;
      showWaveOnKeys(keyboard, wave);
    },

    showStatus(message) {
      window.clearInterval(typingTimer);
      window.clearTimeout(tickerTimer);
      tickerText.textContent = "";
      ticker.classList.add("is-shown");

      let typed = 0;
      typingTimer = window.setInterval(() => {
        typed += 1;
        tickerText.textContent = message.slice(0, typed);
        if (typed < message.length) {
          return;
        }
        window.clearInterval(typingTimer);
        tickerTimer = window.setTimeout(() => {
          ticker.classList.remove("is-shown");
        }, STATUS_TIME);
      }, LETTER_TIME);
    },
  };
}
