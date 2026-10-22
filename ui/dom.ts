// Small helpers for building the page.

// A new element with these class names and, if given, this text.
export function makeElement(tag: string, className: string, text?: string): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  if (text !== undefined) {
    element.textContent = text;
  }
  return element;
}

// Finds the one element matching `selector`, or stops with an error.
export function findElement<T extends HTMLElement>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (element === null) {
    throw new Error(`Missing element: ${selector}`);
  }
  return element;
}

// Runs a button's action, then takes the focus off it, so Space or Enter
// played later can never press it again.
export function clickThenLetGo(event: Event, action: () => void) {
  action();
  const control = event.currentTarget as HTMLElement;
  control.blur();
}

// Follows a drag that starts on `event`'s element inside an SVG drawing,
// calling `move` with where the pointer is, in the drawing's own units
// (its viewBox), each time it moves, until it lets go.
export function dragInDrawing(event: PointerEvent, move: (x: number, y: number) => void) {
  const grabbed = event.currentTarget as SVGGraphicsElement;
  const owner = grabbed.ownerSVGElement;
  if (owner === null) {
    return;
  }
  const drawing: SVGSVGElement = owner;
  grabbed.setPointerCapture(event.pointerId);

  function follow(moved: PointerEvent) {
    // The drawing may be scaled to fit, so the page's pixels are turned
    // back into its own units.
    const toPage = drawing.getScreenCTM();
    if (toPage === null) {
      return;
    }
    const pointer = new DOMPoint(moved.clientX, moved.clientY);
    const inDrawing = pointer.matrixTransform(toPage.inverse());
    move(inDrawing.x, inDrawing.y);
  }

  function stop() {
    grabbed.removeEventListener("pointermove", follow);
    grabbed.removeEventListener("pointerup", stop);
    grabbed.removeEventListener("pointercancel", stop);
  }

  grabbed.addEventListener("pointermove", follow);
  grabbed.addEventListener("pointerup", stop);
  grabbed.addEventListener("pointercancel", stop);
}
