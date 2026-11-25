//! Menus: DEFINE MENU, PAD, POPUP and BAR, and what choosing an item runs.

use crate::interp::Interp;
use crate::parser::{Expr, MenuCmd, MenuItem, Stmt};
use crate::value::Value;
use crate::{Error, R, err};
use serde_json::{Value as Json, json};
use std::collections::HashMap;
use std::rc::Rc;

/// The menu that is always on screen, and that nobody has to define.
const SYSMENU: &str = "_MSYSMENU";
/// How deep submenus are drawn: a popup can open itself, and that must end somewhere.
const MAX_DEPTH: usize = 8;

type Code = Rc<Vec<Stmt>>;

struct Item {
    /// The name of a pad, or the number of a bar.
    key: String,
    prompt: String,
    skip: Option<Rc<Expr>>,
    /// The popup it opens. It may not be defined yet.
    popup: Option<String>,
    code: Option<Code>,
}

/// A menu with its pads, or a popup with its bars.
#[derive(Default)]
struct Group {
    items: Vec<Item>,
    /// Runs for the items that have no code of their own.
    code: Option<Code>,
}

#[derive(Default)]
pub(crate) struct Menus {
    menus: HashMap<String, Group>,
    popups: HashMap<String, Group>,
    /// The menus on screen besides the system one, in the order they were activated.
    active: Vec<String>,
    /// What the user chose last, by the name of the function that reports it.
    chosen: HashMap<&'static str, Value>,
}

impl Menus {
    /// MENU(), PAD(), POPUP(), BAR() and PROMPT(). None when `name` is not one of them.
    pub(crate) fn function(&self, name: &str) -> Option<Value> {
        let empty = match name {
            "BAR" => Value::Num(0.0),
            "MENU" | "PAD" | "POPUP" | "PROMPT" => Value::Str(String::new()),
            _ => return None,
        };
        Some(self.chosen.get(name).cloned().unwrap_or(empty))
    }

    fn group(&mut self, popup: bool, name: &str) -> R<&mut Group> {
        if popup {
            return self.popups.get_mut(name).ok_or_else(|| err(format!("Desplegable no definido: {name}")));
        }
        if name == SYSMENU {
            return Ok(self.menus.entry(name.into()).or_default());
        }
        self.menus.get_mut(name).ok_or_else(|| err(format!("Menú no definido: {name}")))
    }
}

impl Interp {
    /// Whether the item is a bar, and its key.
    fn menu_key(&mut self, item: &MenuItem) -> R<(bool, String)> {
        Ok(match item {
            MenuItem::Pad(name) => (false, name.clone()),
            MenuItem::Bar(number) => (true, (self.num(number)? as i64).to_string()),
        })
    }

    pub(crate) fn exec_menu(&mut self, cmd: &MenuCmd) -> R<()> {
        match cmd {
            MenuCmd::Define { popup, name } => {
                let groups = if *popup { &mut self.menus.popups } else { &mut self.menus.menus };
                groups.insert(name.clone(), Group::default());
            }
            MenuCmd::Item { item, of, prompt, skip } => {
                let (popup, key) = self.menu_key(item)?;
                let prompt = self.eval(prompt)?.display();
                let items = &mut self.menus.group(popup, of)?.items;
                // defining an item again changes it where it is, so running a program twice does not duplicate its menu
                match items.iter_mut().find(|i| i.key == key) {
                    Some(old) => (old.prompt, old.skip) = (prompt, skip.clone()),
                    None => items.push(Item { key, prompt, skip: skip.clone(), popup: None, code: None }),
                }
            }
            MenuCmd::Opens { item, of, popup: opens } => self.menu_item(item, of)?.popup = opens.clone(),
            MenuCmd::Selection { item, popup, of, code } => {
                let code = Some(code.clone()).filter(|c| !c.is_empty());
                match item {
                    Some(item) => self.menu_item(item, of)?.code = code,
                    None => self.menus.group(*popup, of)?.code = code,
                }
            }
            MenuCmd::Show { name, on } => {
                self.menus.group(false, name)?;
                self.menus.active.retain(|n| n != name);
                if *on {
                    self.menus.active.push(name.clone());
                }
            }
            MenuCmd::Release { popups, names } => {
                let groups = if *popups { &mut self.menus.popups } else { &mut self.menus.menus };
                // RELEASE MENUS leaves the system menu, as in FoxPro: SET SYSMENU TO empties that one
                groups.retain(|name, _| name == SYSMENU || !(names.is_empty() || names.contains(name)));
                let menus = &self.menus.menus;
                self.menus.active.retain(|n| menus.contains_key(n));
            }
            MenuCmd::ResetSys => {
                self.menus.menus.remove(SYSMENU);
            }
        }
        Ok(())
    }

    fn menu_item(&mut self, item: &MenuItem, of: &str) -> R<&mut Item> {
        let (popup, key) = self.menu_key(item)?;
        let found = self.menus.group(popup, of)?.items.iter_mut().find(|i| i.key == key);
        found.ok_or_else(|| err(format!("{of} no tiene la opción {key}")))
    }

    /// The user chose an item: a pad of `menu`, or the bar `(popup, number)` reached through that pad.
    pub fn menu_event(&mut self, menu: &str, pad: &str, bar: Option<(&str, &str)>) -> Result<(), Error> {
        let (group, key) = match bar {
            Some((popup, number)) => (self.menus.popups.get(popup), number),
            None => (self.menus.menus.get(menu), pad),
        };
        // the menu may have been released while the click was on its way
        let Some((group, item)) = group.and_then(|g| Some((g, g.items.iter().find(|i| i.key == key)?))) else {
            return Ok(());
        };
        let (code, prompt) = (item.code.clone().or_else(|| group.code.clone()), item.prompt.clone());
        let text = |s: &str| Value::Str(s.to_string());
        self.menus.chosen = HashMap::from([
            ("MENU", text(menu)),
            ("PAD", text(pad)),
            ("POPUP", text(bar.map_or("", |b| b.0))),
            ("BAR", Value::Num(bar.and_then(|b| b.1.parse().ok()).unwrap_or(0.0))),
            ("PROMPT", text(&prompt)),
        ]);
        let Some(code) = code else { return Ok(()) };
        self.guarded(|it| it.block(&code).map(|_| ())).map_err(|mut e| {
            // the line is in the program that defined the menu, not in what is being run now
            e.file.get_or_insert_with(|| "ON SELECTION".into());
            e
        })
    }

    /// The menus on screen, as the tree a UI needs to draw them.
    pub fn menus_json(&mut self) -> Json {
        let mut shown = vec![SYSMENU.to_string()];
        shown.extend(self.menus.active.iter().filter(|n| *n != SYSMENU).cloned());
        shown.retain(|n| self.menus.menus.get(n).is_some_and(|g| !g.items.is_empty()));
        shown.iter().map(|n| json!({ "name": n, "items": self.menu_items(false, n, 0) })).collect()
    }

    fn menu_items(&mut self, popup: bool, name: &str, depth: usize) -> Json {
        let groups = if popup { &self.menus.popups } else { &self.menus.menus };
        // copied out first: evaluating SKIP FOR needs the whole interpreter
        let items: Vec<_> = groups.get(name).into_iter().flat_map(|g| &g.items).collect();
        let items: Vec<_> = items.iter().map(|i| (i.key.clone(), i.prompt.clone(), i.skip.clone(), i.popup.clone())).collect();
        let mut out = Vec::new();
        for (key, prompt, skip, opens) in items {
            // a condition that fails leaves the item enabled
            let disabled = skip.is_some_and(|cond| self.truthy(&cond).unwrap_or(false));
            let sub = opens.as_ref().filter(|_| depth < MAX_DEPTH).map(|p| self.menu_items(true, p, depth + 1));
            out.push(json!({ "key": key, "prompt": prompt, "disabled": disabled, "popup": opens, "items": sub }));
        }
        Json::Array(out)
    }
}
