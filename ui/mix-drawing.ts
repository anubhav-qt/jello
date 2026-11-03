// The maths behind the Mix panel's drawings (docs/design.md#mix): where a
// frequency or a level sits in a drawing, and the curves the EQ, the
// filter and saturation draw.

// A drawing's size in its own units, with room at the edges for squares.
export const WIDTH = 240;
export const HEIGHT = 150;
export const EDGE = 8;

export const LOWEST_HZ = 20;
export const HIGHEST_HZ = 20000;

// A line through `points`, as an SVG path.
export function pathOf(points: [number, number][]): string {
  let path = "";
  for (let place = 0; place < points.length; place += 1) {
    const [x, y] = points[place];
    let move = "L";
    if (place === 0) {
      move = "M";
    }
    path += `${move}${x.toFixed(1)} ${y.toFixed(1)} `;
  }
  return path.trim();
}

function within(value: number, lowest: number, highest: number): number {
  return Math.max(lowest, Math.min(highest, value));
}

// Where `hz` is across a drawing, 20 Hz at the left edge and 20 kHz at the
// right, and back.
export function xOfHz(hz: number): number {
  const share = Math.log(hz / LOWEST_HZ) / Math.log(HIGHEST_HZ / LOWEST_HZ);
  return EDGE + share * (WIDTH - 2 * EDGE);
}

export function hzOfX(x: number): number {
  const share = within((x - EDGE) / (WIDTH - 2 * EDGE), 0, 1);
  return Math.round(LOWEST_HZ * Math.pow(HIGHEST_HZ / LOWEST_HZ, share));
}

// Where `value` is between `lowest` at the bottom and `highest` at the
// top, and back.
export function yOf(value: number, lowest: number, highest: number): number {
  const share = within((value - lowest) / (highest - lowest), 0, 1);
  return HEIGHT - EDGE - share * (HEIGHT - 2 * EDGE);
}

export function valueOfY(y: number, lowest: number, highest: number): number {
  const share = within((HEIGHT - EDGE - y) / (HEIGHT - 2 * EDGE), 0, 1);
  return lowest + share * (highest - lowest);
}

// Where `value` is between `lowest` at the left and `highest` at the
// right, and back.
export function xOf(value: number, lowest: number, highest: number): number {
  const share = within((value - lowest) / (highest - lowest), 0, 1);
  return EDGE + share * (WIDTH - 2 * EDGE);
}

export function valueOfX(x: number, lowest: number, highest: number): number {
  const share = within((x - EDGE) / (WIDTH - 2 * EDGE), 0, 1);
  return lowest + share * (highest - lowest);
}

// The frequencies a curve is drawn at, evenly in octaves.
export function curveHz(count: number): number[] {
  const all: number[] = [];
  for (let place = 0; place < count; place += 1) {
    const share = place / (count - 1);
    all.push(LOWEST_HZ * Math.pow(HIGHEST_HZ / LOWEST_HZ, share));
  }
  return all;
}

// A complex number, for the EQ's shelves.
interface Complex {
  re: number;
  im: number;
}

function times(a: Complex, b: Complex): Complex {
  return { re: a.re * b.re - a.im * b.im, im: a.re * b.im + a.im * b.re };
}

function size(a: Complex): number {
  return Math.sqrt(a.re * a.re + a.im * a.im);
}

// a s^2 + b s + c at s = j w.
function quadratic(a: number, b: number, c: number, w: number): Complex {
  return { re: c - a * w * w, im: b * w };
}

// One EQ band: how far it lifts or cuts `hz`, in dB. `shape` is the band's
// word, `width` its Q, and a cut's `slope` is in dB an octave.
export function eqBandDb(shape: string, hz: number, frequency: number, gainDb: number, width: number, slope: number): number {
  const w = hz / frequency;
  const q = Math.max(width, 0.1);
  if (shape === "low-cut" || shape === "high-cut") {
    // A Butterworth cut of slope / 6 poles.
    const poles = slope / 6;
    let ratio = w;
    if (shape === "low-cut") {
      ratio = 1 / w;
    }
    return -10 * Math.log10(1 + Math.pow(ratio, 2 * poles));
  }
  if (shape === "notch") {
    const top = size(quadratic(1, 0, 1, w));
    const bottom = size(quadratic(1, 1 / q, 1, w));
    return 20 * Math.log10(Math.max(top / bottom, 0.000001));
  }
  const a = Math.pow(10, gainDb / 40);
  if (shape === "bell") {
    const top = size(quadratic(1, a / q, 1, w));
    const bottom = size(quadratic(1, 1 / (a * q), 1, w));
    return 20 * Math.log10(top / bottom);
  }
  const root = Math.sqrt(a);
  let top: Complex;
  let bottom: Complex;
  if (shape === "low-shelf") {
    top = quadratic(1, root / q, a, w);
    bottom = quadratic(a, root / q, 1, w);
  } else {
    top = quadratic(a, root / q, 1, w);
    bottom = quadratic(1, root / q, a, w);
  }
  const gain = times({ re: a, im: 0 }, top);
  return 20 * Math.log10(size(gain) / size(bottom));
}

// How saturation of curve `curve` at `driveDb` turns `input`, from -1 to
// 1, into its output, as drawn.
export function saturated(curve: string, driveDb: number, input: number): number {
  const drive = Math.pow(10, driveDb / 20);
  const driven = input * drive;
  if (curve === "hard") {
    return within(driven, -1, 1);
  }
  if (curve === "tube") {
    // Leans one way, as a tube does.
    const lean = 0.25;
    return (Math.tanh(driven + lean) - Math.tanh(lean)) / (1 - Math.tanh(lean) * Math.tanh(lean) * 0.5);
  }
  if (curve === "fold") {
    // Past 1, it folds back down, and on.
    const turned = (driven + 1) / 4;
    const along = turned - Math.floor(turned);
    return 1 - 4 * Math.abs(along - 0.5);
  }
  return Math.tanh(driven);
}
