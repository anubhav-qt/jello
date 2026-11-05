// Right Command (docs/design.md#ai-and-learning-rethought-2026-11-03):
// while it is down, Command shortcuts like Command Q are blocked. macOS
// sends those to the app menu before the page sees them, so the menu is put
// away while the key is down and given back when it comes up. Jello runs
// fullscreen, so the menu bar is hidden and nothing on screen changes.

use std::sync::Mutex;

use tauri::menu::Menu;
use tauri::{AppHandle, State, Wry};

// The app menu while it is put away. Generic so tests need no app.
pub struct PutAway<M> {
    menu: Option<M>,
    away: bool,
}

impl<M> Default for PutAway<M> {
    fn default() -> Self {
        PutAway { menu: None, away: false }
    }
}

impl<M> PutAway<M> {
    // Whether the menu is away now.
    pub fn is_away(&self) -> bool {
        self.away
    }

    // Keeps `menu`, the one just taken off. Only the first is kept, so a
    // second key-down never loses the real menu.
    pub fn keep(&mut self, menu: Option<M>) {
        if self.away {
            return;
        }
        self.menu = menu;
        self.away = true;
    }

    // The menu to give back, if it is away. Giving back twice gives nothing.
    pub fn give_back(&mut self) -> Option<M> {
        self.away = false;
        self.menu.take()
    }
}

pub type HeldMenu = Mutex<PutAway<Menu<Wry>>>;

// The page calls this as right Command goes down.
#[tauri::command]
pub fn talk_key_down(app: AppHandle, held: State<'_, HeldMenu>) -> Result<(), String> {
    let mut held = held.lock().unwrap();
    if held.is_away() {
        return Ok(());
    }
    let menu = app.remove_menu().map_err(|error| error.to_string())?;
    held.keep(menu);
    Ok(())
}

// The page calls this as right Command comes up, or the window loses focus.
#[tauri::command]
pub fn talk_key_up(app: AppHandle, held: State<'_, HeldMenu>) -> Result<(), String> {
    let mut held = held.lock().unwrap();
    let Some(menu) = held.give_back() else {
        return Ok(());
    };
    app.set_menu(menu).map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_menu_comes_back_after_the_key() {
        let mut held = PutAway::default();
        assert!(!held.is_away());
        held.keep(Some("menu"));
        assert!(held.is_away());
        assert_eq!(held.give_back(), Some("menu"));
        assert!(!held.is_away());
    }

    #[test]
    fn a_second_key_down_never_loses_the_menu() {
        // While away, the app has no menu, so a second take-off finds none.
        let mut held = PutAway::default();
        held.keep(Some("menu"));
        held.keep(None);
        assert_eq!(held.give_back(), Some("menu"));
    }

    #[test]
    fn giving_back_without_a_key_down_gives_nothing() {
        let mut held: PutAway<&str> = PutAway::default();
        assert_eq!(held.give_back(), None);
        held.keep(Some("menu"));
        held.give_back();
        assert_eq!(held.give_back(), None);
    }
}
