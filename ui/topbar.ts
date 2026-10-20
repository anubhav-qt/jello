// The page's top bar: where you are, the wave and Render WAV buttons, and
// the tabs.

import { makeButton, makeElement } from "./dom";

export interface TopBar {
  element: HTMLElement;
  // Where the wave's name goes.
  waveName: HTMLElement;
}

// A small piano, drawn in the text's color.
const PIANO_ICON = `<svg class="tab-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
  <rect x="1.75" y="2.75" width="12.5" height="10.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.5"/>
  <path d="M6 9v4.25M10 9v4.25" fill="none" stroke="currentColor" stroke-width="1.5"/>
  <path d="M4.75 2.75h2.5V9h-2.5zM8.75 2.75h2.5V9h-2.5z" fill="currentColor"/>
</svg>`;

// The top bar, built like GitHub's repository header: a title row with where
// you are on the left and actions on the right, then a row of tabs. It starts
// sparse: one tab and two actions.
export function drawTopBar(onWaveClick: () => void, onRenderClick: () => void): TopBar {
  const crumbs = makeElement("div", "crumbs");
  crumbs.append(
    makeElement("span", "logo"),
    makeElement("span", "crumb", "Jello"),
    makeElement("span", "crumb-divider", "/"),
    makeElement("span", "crumb is-current", "Keyboard"),
  );

  const waveName = makeElement("span", "action-value", "");
  const waveButton = makeButton("action", onWaveClick);
  waveButton.title = "Next wave (the ` key)";
  waveButton.append(makeElement("span", "action-label", "Wave"), waveName);

  const renderButton = makeButton("action", onRenderClick);
  renderButton.title = "Save a held middle C as a WAV file";
  renderButton.append(makeElement("span", "action-value", "Render WAV"));

  const actions = makeElement("div", "actions");
  actions.append(waveButton, renderButton);

  const titleRow = makeElement("div", "topbar-row");
  titleRow.append(crumbs, actions);

  const playTab = makeElement("span", "tab is-active");
  playTab.innerHTML = PIANO_ICON;
  playTab.append(makeElement("span", "tab-label", "Play"));
  const tabs = makeElement("nav", "tabs");
  tabs.append(playTab);

  const element = makeElement("header", "topbar");
  element.append(titleRow, tabs);
  return { element, waveName };
}
