//! Objects, and the built-in classes they can be made from.

use crate::parser::Routine;
use crate::value::Value;
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

/// A shared, mutable object. Two references are equal when they are the same object.
#[derive(Clone)]
pub struct ObjRef(pub Rc<RefCell<Object>>);

impl PartialEq for ObjRef {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl std::fmt::Debug for ObjRef {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("Object")
    }
}

pub struct Object {
    /// Identifies the object to the UI.
    pub id: u64,
    /// Key of its built-in class: "FORM", "TEXTBOX"...
    pub base: &'static str,
    /// The user class it was created from, if any.
    pub class: Option<String>,
    /// Properties by upper-cased name. Child objects are properties too.
    pub props: HashMap<String, Value>,
    /// Names of the child objects, in creation order.
    pub children: Vec<String>,
    /// Weak, so a form and its controls do not keep each other alive.
    pub parent: Option<Weak<RefCell<Object>>>,
    /// Methods of this object alone, with where they were defined:
    /// `PROCEDURE child.Event` in a class, or the code of a form file.
    pub methods: HashMap<String, (Rc<Routine>, Option<String>)>,
    /// What AddItem added to a ComboBox or ListBox.
    pub items: Vec<String>,
}

impl Object {
    pub fn parent(&self) -> Option<ObjRef> {
        self.parent.as_ref()?.upgrade().map(ObjRef)
    }

    pub fn kids(&self) -> Vec<ObjRef> {
        let kid = |name: &String| match self.props.get(name) {
            Some(Value::Object(o)) => Some(o.clone()),
            _ => None,
        };
        self.children.iter().filter_map(kid).collect()
    }

    /// Items of a ComboBox or ListBox: RowSource when RowSourceType is 1 (a comma list), else the AddItem ones.
    pub fn list(&self) -> Vec<String> {
        match (self.props.get("ROWSOURCETYPE"), self.props.get("ROWSOURCE")) {
            (Some(Value::Num(t)), Some(Value::Str(s))) if *t == 1.0 => {
                s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()
            }
            _ => self.items.clone(),
        }
    }
}

/// A built-in class: its default properties and the events the UI can raise on it.
pub struct Base {
    pub key: &'static str,
    pub name: &'static str,
    pub props: Vec<(&'static str, Value)>,
    pub events: &'static [&'static str],
}

const WHITE: f64 = 16777215.0;
const GRAY: f64 = 15790320.0; // RGB(240, 240, 240)
const SILVER: f64 = 10526880.0; // RGB(160, 160, 160)
const BLUE: f64 = 16711680.0; // RGB(0, 0, 255)

type Props = Vec<(&'static str, Value)>;

/// Properties of everything that has a place on a form, then the ones of this class (which win on a clash).
fn placed(width: f64, height: f64, own: Props) -> Props {
    use Value::*;
    let mut props = vec![
        ("Left", Num(0.0)),
        ("Top", Num(0.0)),
        ("Width", Num(width)),
        ("Height", Num(height)),
        ("Visible", Bool(true)),
        ("Enabled", Bool(true)),
    ];
    props.retain(|(name, _)| !own.iter().any(|(o, _)| o == name));
    props.extend(own);
    props
}

/// `placed`, plus the properties of everything that shows text.
fn visual(width: f64, height: f64, own: Props) -> Props {
    use Value::*;
    let mut text = vec![("FontSize", Num(9.0)), ("FontBold", Bool(false)), ("ForeColor", Num(0.0)), ("ToolTipText", Str(String::new()))];
    text.retain(|(name, _)| !own.iter().any(|(o, _)| o == name));
    text.extend(own);
    placed(width, height, text)
}

/// The built-in classes, in the order a designer's toolbox shows them.
pub const BASES: &[&str] = &[
    "CUSTOM", "FORM", "LABEL", "TEXTBOX", "EDITBOX", "SPINNER", "COMMANDBUTTON", "CHECKBOX", "COMBOBOX", "LISTBOX",
    "OPTIONGROUP", "OPTIONBUTTON", "COMMANDGROUP", "HYPERLINK", "IMAGE", "LINE", "SHAPE", "TIMER", "CONTAINER",
    "PAGEFRAME", "PAGE",
];

/// For the classes that only hold one kind of child, what `PageCount` or `ButtonCount` creates: its class and name prefix.
pub fn member_of(key: &str) -> Option<(&'static str, &'static str)> {
    match key {
        "PAGEFRAME" => Some(("PAGE", "Page")),
        "OPTIONGROUP" => Some(("OPTIONBUTTON", "Option")),
        "COMMANDGROUP" => Some(("COMMANDBUTTON", "Command")),
        _ => None,
    }
}

pub fn base(key: &str) -> Option<Base> {
    use Value::*;
    let s = |text: &str| Str(text.into());
    let list = || {
        vec![("Value", s("")), ("RowSourceType", Num(0.0)), ("RowSource", s("")), ("ListIndex", Num(0.0)), ("BackColor", Num(WHITE))]
    };
    // a filled box with a border; BackStyle 0 makes it transparent
    let boxed = |back: f64, border: f64| vec![("BackColor", Num(back)), ("BackStyle", Num(1.0)), ("BorderWidth", Num(1.0)), ("BorderColor", Num(border))];
    let with = |mut props: Props, more: Props| {
        props.extend(more);
        props
    };
    const TYPED: &[&str] = &["Init", "InteractiveChange", "GotFocus", "LostFocus", "Destroy"];
    const CLICKED: &[&str] = &["Init", "Click", "Destroy"];
    const CHOSEN: &[&str] = &["Init", "InteractiveChange", "Destroy"];
    let (key, name, props, events): (_, _, _, &'static [&'static str]) = match key {
        "CUSTOM" => ("CUSTOM", "Custom", Vec::new(), &["Init", "Destroy"]),
        "FORM" => (
            "FORM",
            "Form",
            visual(
                375.0,
                250.0,
                vec![
                    ("Caption", s("Form1")),
                    ("Visible", Bool(false)),
                    ("BackColor", Num(GRAY)),
                    ("MinButton", Bool(true)),
                    ("MaxButton", Bool(true)),
                    // 0 normal, 1 minimized, 2 maximized
                    ("WindowState", Num(0.0)),
                    // not a FoxPro property: how round the window's corners are, in pixels
                    ("BorderRadius", Num(0.0)),
                ],
            ),
            &["Load", "Init", "Click", "Destroy"],
        ),
        "LABEL" => ("LABEL", "Label", visual(40.0, 17.0, vec![("Caption", s("Label1"))]), CLICKED),
        "TEXTBOX" => (
            "TEXTBOX",
            "TextBox",
            visual(100.0, 23.0, vec![("Value", s("")), ("ReadOnly", Bool(false)), ("PasswordChar", s("")), ("BackColor", Num(WHITE))]),
            TYPED,
        ),
        "EDITBOX" => {
            ("EDITBOX", "EditBox", visual(100.0, 53.0, vec![("Value", s("")), ("ReadOnly", Bool(false)), ("BackColor", Num(WHITE))]), TYPED)
        }
        "SPINNER" => (
            "SPINNER",
            "Spinner",
            visual(
                100.0,
                24.0,
                vec![
                    ("Value", Num(0.0)),
                    ("Increment", Num(1.0)),
                    ("SpinnerLowValue", Num(-2147483647.0)),
                    ("SpinnerHighValue", Num(2147483647.0)),
                    ("BackColor", Num(WHITE)),
                ],
            ),
            TYPED,
        ),
        "COMMANDBUTTON" => ("COMMANDBUTTON", "CommandButton", visual(84.0, 27.0, vec![("Caption", s("Command1"))]), CLICKED),
        "CHECKBOX" => (
            "CHECKBOX",
            "CheckBox",
            visual(60.0, 17.0, vec![("Caption", s("Check1")), ("Value", Num(0.0))]),
            &["Init", "Click", "InteractiveChange", "Destroy"],
        ),
        "COMBOBOX" => ("COMBOBOX", "ComboBox", visual(100.0, 24.0, list()), CHOSEN),
        "LISTBOX" => ("LISTBOX", "ListBox", visual(100.0, 170.0, list()), CHOSEN),
        // Value is the number of the chosen button, counting from 1
        "OPTIONGROUP" => (
            "OPTIONGROUP",
            "OptionGroup",
            placed(71.0, 46.0, with(vec![("Value", Num(1.0))], boxed(GRAY, SILVER))),
            &["Init", "InteractiveChange", "Click", "Destroy"],
        ),
        "OPTIONBUTTON" => ("OPTIONBUTTON", "OptionButton", visual(61.0, 17.0, vec![("Caption", s("Option1"))]), CLICKED),
        // Value is the number of the button clicked last
        "COMMANDGROUP" => {
            ("COMMANDGROUP", "CommandGroup", placed(94.0, 66.0, with(vec![("Value", Num(0.0))], boxed(GRAY, SILVER))), CLICKED)
        }
        // not FoxPro's invisible Hyperlink: a link the user can click, which opens Url in the browser
        "HYPERLINK" => (
            "HYPERLINK",
            "HyperLink",
            visual(60.0, 17.0, vec![("Caption", s("Hyperlink1")), ("Url", s("")), ("ForeColor", Num(BLUE))]),
            CLICKED,
        ),
        // Stretch: 0 actual size, 1 fit keeping proportions, 2 fill
        "IMAGE" => ("IMAGE", "Image", placed(100.0, 100.0, vec![("Picture", s("")), ("Stretch", Num(0.0))]), CLICKED),
        // drawn corner to corner: Height 0 is horizontal, Width 0 vertical; LineSlant "/" or "\" picks the diagonal
        "LINE" => (
            "LINE",
            "Line",
            placed(100.0, 0.0, vec![("BorderWidth", Num(1.0)), ("BorderColor", Num(0.0)), ("LineSlant", s("\\"))]),
            &["Init", "Destroy"],
        ),
        // Curvature: 0 is a rectangle, 99 an ellipse
        "SHAPE" => ("SHAPE", "Shape", placed(100.0, 60.0, with(boxed(WHITE, 0.0), vec![("Curvature", Num(0.0))])), CLICKED),
        // Interval in milliseconds; 0 stops it
        "TIMER" => ("TIMER", "Timer", placed(23.0, 23.0, vec![("Interval", Num(0.0))]), &["Init", "Timer", "Destroy"]),
        "CONTAINER" => ("CONTAINER", "Container", placed(100.0, 100.0, boxed(GRAY, SILVER)), CLICKED),
        // Caption, when not empty, is a title drawn above the tabs; each tab shows its Page's own Caption
        "PAGEFRAME" => ("PAGEFRAME", "PageFrame", placed(241.0, 169.0, vec![("ActivePage", Num(1.0)), ("Caption", s(""))]), &["Init", "Destroy"]),
        "PAGE" => (
            "PAGE",
            "Page",
            vec![("Caption", s("Page1")), ("Enabled", Bool(true)), ("BackColor", Num(GRAY))],
            &["Init", "Activate", "Destroy"],
        ),
        _ => return None,
    };
    Some(Base { key, name, props, events })
}

/// What a form designer needs to know about each built-in class.
pub fn class_info() -> Json {
    let info = |key: &&str| {
        let b = base(key).unwrap();
        let props: Vec<Json> = b.props.iter().map(|(name, v)| json!([name, to_json(v)])).collect();
        json!({ "name": b.name, "props": props, "events": b.events })
    };
    BASES.iter().map(info).collect()
}

/// None for values that have no JSON form: arrays and objects.
pub fn to_json(v: &Value) -> Option<Json> {
    Some(match v {
        Value::Num(n) => json!(n),
        Value::Str(s) => json!(s),
        Value::Bool(b) => json!(b),
        Value::Null => Json::Null,
        Value::Date(_) => json!(v.display()),
        Value::Array(_) | Value::Object(_) => return None,
    })
}

pub fn from_json(v: &Json) -> Option<Value> {
    Some(match v {
        Json::Number(n) => Value::Num(n.as_f64()?),
        Json::String(s) => Value::Str(s.clone()),
        Json::Bool(b) => Value::Bool(*b),
        Json::Null => Value::Null,
        _ => return None,
    })
}
