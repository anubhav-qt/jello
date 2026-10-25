<!-- A whole number drawn in white Doto digits that can be typed over, like
     the transport's tempo. Return or leaving the field sets it; a number
     past the ends is kept within them, and anything that is not a number
     puts the old value back. -->
<script lang="ts">
  interface Props {
    value: number;
    lowest: number;
    highest: number;
    // What a screen reader says the field is.
    label: string;
    disabled?: boolean;
    onSet: (value: number) => void;
  }
  let { value, lowest, highest, label, disabled = false, onSet }: Props =
    $props();

  // As many digits wide as the highest value.
  let digits = $derived(String(highest).length);

  function typed(event: Event) {
    const field = event.currentTarget as HTMLInputElement;
    const text = field.value.trim();
    const number = Number(text);
    // Shown again until Rust answers with the new value.
    field.value = String(value);
    if (text === "" || !Number.isFinite(number)) {
      return;
    }
    let within = Math.round(number);
    if (within < lowest) {
      within = lowest;
    }
    if (within > highest) {
      within = highest;
    }
    if (within === value) {
      return;
    }
    onSet(within);
  }

  // Clicking selects the digits, so typing replaces them.
  function selectAll(event: FocusEvent) {
    const field = event.currentTarget as HTMLInputElement;
    field.select();
  }

  // WebKit puts the caret where the mouse lets go, which undoes the
  // selection, so the let-go of the click that focused the field is ignored.
  let clickedIn = false;

  function noteClick(event: MouseEvent) {
    const field = event.currentTarget as HTMLInputElement;
    if (document.activeElement !== field) {
      clickedIn = true;
    }
  }

  function keepSelection(event: MouseEvent) {
    if (clickedIn) {
      event.preventDefault();
      clickedIn = false;
    }
  }
</script>

<input
  class="digit-field"
  type="text"
  inputmode="numeric"
  maxlength={digits + 2}
  aria-label={label}
  style:--digits={digits}
  {disabled}
  value={value}
  onmousedown={noteClick}
  onmouseup={keepSelection}
  onfocus={selectAll}
  onchange={typed}
/>

<style>
  /* Its own font is 1.31 times its row's, so its em here is that size. */
  .digit-field {
    width: calc(var(--digits) * 1ch + 0.35em);
    padding: 0.12em 0.18em;
    color: rgba(var(--ink), 0.92);
    text-shadow: 0 0 10px rgba(var(--ink), 0.25);
    background: none;
    border: 1px solid transparent;
    border-radius: 3px;
    outline: none;
    font-family: "Doto", monospace;
    font-weight: 800;
    font-size: 1.31em;
    text-align: center;
  }
  .digit-field:hover:not(:disabled),
  .digit-field:focus {
    border-color: rgba(var(--accent), 0.6);
  }
  .digit-field:disabled {
    opacity: 0.5;
  }
</style>
