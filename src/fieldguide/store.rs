//! Field Guide state lives here (not in the window) so commands from the
//! Stream Deck work whether or not the app's window is open.
//!   checked[page] = [step key, ...]  ticked steps
//!   chosen[page]  = option index      which situation was picked on a choice page
//!   mica[page]    = true | false      the answer to "Is MICA available?" on that page

use super::data::{asks_mica, guide, step_keys};
use std::collections::HashMap;

#[derive(Clone)]
pub struct FieldGuideState {
    pub page: String,
    pub checked: HashMap<String, Vec<String>>,
    pub chosen: HashMap<String, usize>,
    pub mica: HashMap<String, bool>,
    /// Step to bring into view (set when a step is ticked from the Stream Deck).
    pub focus: Option<String>,
}

impl Default for FieldGuideState {
    fn default() -> Self {
        Self { page: "home".into(), checked: HashMap::new(), chosen: HashMap::new(), mica: HashMap::new(), focus: None }
    }
}

pub struct Progress {
    pub keys: Vec<String>,
    pub done: Vec<String>,
    pub needs_choice: bool,
    pub complete: bool,
}

impl FieldGuideState {
    /// What is showing on a page right now: its steps and which of them are ticked.
    pub fn progress(&self, id: &str) -> Progress {
        let chosen = self.chosen.get(id).copied();
        let mica = self.mica.get(id).copied();
        let keys = step_keys(id, chosen, mica);
        let done: Vec<String> = self.checked.get(id).map(|c| c.iter().filter(|k| keys.contains(k)).cloned().collect()).unwrap_or_default();
        let needs_choice = guide().page(id).is_some_and(|p| p.options().is_some()) && chosen.is_none();
        let needs_mica = asks_mica(id, chosen) && mica.is_none();
        let complete = !keys.is_empty() && !needs_choice && !needs_mica && done.len() == keys.len();
        Progress { keys, done, needs_choice, complete }
    }

    fn go(&mut self, page: &str) {
        self.page = page.to_string();
        self.focus = None;
    }

    /// Every command a Stream Deck key (or a click) can run. Returns false if it made no sense.
    pub fn run(&mut self, command: &str, arg: Option<&str>) -> bool {
        let order: Vec<&str> = guide().order().collect();
        let here = order.iter().position(|p| *p == self.page).unwrap_or(0);
        match command {
            "open" => match arg.and_then(|id| guide().page(id)) {
                Some(p) => {
                    self.go(p.id);
                    true
                }
                None => false,
            },
            "home" => {
                self.go("home");
                true
            }
            "next" => {
                self.go(order[(here + 1) % order.len()]);
                true
            }
            "prev" => {
                self.go(order[(here + order.len() - 1) % order.len()]);
                true
            }
            // "option/2" picks the second situation, plain "option" moves to the next one.
            "option" => {
                let Some(options) = guide().page(&self.page).and_then(|p| p.options()) else { return false };
                let current = self.chosen.get(&self.page).copied();
                let index = match arg {
                    Some(n) => match n.parse::<usize>() {
                        Ok(n) if n >= 1 => n - 1,
                        _ => return false,
                    },
                    None => current.map_or(0, |c| (c + 1) % options.len()),
                };
                if index >= options.len() {
                    return false;
                }
                self.chosen.insert(self.page.clone(), index);
                self.focus = None;
                true
            }
            // "mica/yes", "mica/no", or plain "mica" to flip it.
            "mica" => {
                if !asks_mica(&self.page, self.chosen.get(&self.page).copied()) {
                    return false;
                }
                let value = match arg {
                    Some("yes") => true,
                    Some("no") => false,
                    Some(_) => return false,
                    None => !self.mica.get(&self.page).copied().unwrap_or(false),
                };
                self.mica.insert(self.page.clone(), value);
                self.focus = None;
                true
            }
            // Tick the next un-ticked step that is showing.
            "step" => {
                let p = self.progress(&self.page);
                let Some(key) = p.keys.iter().find(|k| !p.done.contains(k)).cloned() else { return false };
                self.checked.entry(self.page.clone()).or_default().push(key.clone());
                self.focus = Some(key);
                true
            }
            // Un-tick the most recently ticked step that is showing.
            "unstep" => {
                let p = self.progress(&self.page);
                let Some(key) = p.done.last().cloned() else { return false };
                if let Some(list) = self.checked.get_mut(&self.page) {
                    list.retain(|k| *k != key);
                }
                self.focus = Some(key);
                true
            }
            // Start the page again: clears the ticks, the chosen situation and the MICA answer.
            "reset" => {
                let page = self.page.clone();
                self.checked.insert(page.clone(), Vec::new());
                self.chosen.remove(&page);
                self.mica.remove(&page);
                self.focus = None;
                true
            }
            "reset-all" => {
                self.checked.clear();
                self.chosen.clear();
                self.mica.clear();
                self.focus = None;
                true
            }
            _ => false,
        }
    }

    pub fn toggle_step(&mut self, key: &str) {
        let list = self.checked.entry(self.page.clone()).or_default();
        if let Some(i) = list.iter().position(|k| k == key) {
            list.remove(i);
        } else {
            list.push(key.to_string());
        }
        self.focus = None;
    }
}
