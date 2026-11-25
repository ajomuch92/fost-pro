//! The object side of the interpreter: classes, methods, and forms shown by a UI.

use crate::interp::{Frame, Interp, VarRef, var};
use crate::object::{self, ObjRef, Object, from_json, to_json};
use crate::parser::{ClassDef, Expr, Routine, parse};
use crate::value::{Array, Value};
use crate::{Error, R, err};
use serde_json::{Value as Json, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The object with this id in a tree, if it is there.
fn find(obj: &ObjRef, id: u64) -> Option<ObjRef> {
    if obj.0.borrow().id == id {
        return Some(obj.clone());
    }
    let kids = obj.0.borrow().kids();
    kids.iter().find_map(|k| find(k, id))
}

impl Interp {
    /// THIS, or with `form` the form it sits on.
    pub(crate) fn this(&mut self, form: bool) -> R<ObjRef> {
        let Some(mut cur) = self.top().this.clone() else {
            return Err(err("THIS y THISFORM solo pueden usarse dentro de un método"));
        };
        while form && cur.0.borrow().base != "FORM" {
            let parent = cur.0.borrow().parent();
            cur = parent.ok_or_else(|| err("Este objeto no está dentro de un formulario"))?;
        }
        Ok(cur)
    }

    pub(crate) fn object(&mut self, e: &Expr) -> R<ObjRef> {
        match self.eval(e)? {
            Value::Object(o) => Ok(o),
            _ => Err(err("Se esperaba un objeto")),
        }
    }

    pub(crate) fn get_prop(&self, obj: &ObjRef, name: &str) -> R<Value> {
        let o = obj.0.borrow();
        Ok(match name {
            "PARENT" => Value::Object(o.parent().ok_or_else(|| err("Este objeto no tiene Parent"))?),
            "CONTROLCOUNT" => Value::Num(o.children.len() as f64),
            "CONTROLS" | "OBJECTS" => {
                Value::Array(Array { cols: 0, data: o.kids().into_iter().map(Value::Object).collect() })
            }
            "LISTCOUNT" => Value::Num(o.list().len() as f64),
            _ => o.props.get(name).cloned().ok_or_else(|| err(format!("Propiedad no encontrada: {name}")))?,
        })
    }

    /// The user-defined method `name` of `obj`: the routine, where it was defined, and the class
    /// that defines it (None: the object itself). `above` restarts the search over that level, for DODEFAULT().
    fn find_method(&self, obj: &ObjRef, name: &str, above: Option<&Option<String>>) -> Option<(Rc<Routine>, Option<String>, Option<String>)> {
        let o = obj.0.borrow();
        let mut next = match above {
            None => match o.methods.get(name) {
                Some((routine, origin)) => return Some((routine.clone(), origin.clone(), None)),
                None => o.class.clone(),
            },
            Some(None) => o.class.clone(),
            Some(Some(class)) => self.classes.get(class).map(|(c, _)| c.parent.clone()),
        };
        // ends at the built-in class, which is not in `classes`
        while let Some((class, origin)) = next.as_ref().and_then(|n| self.classes.get(n)) {
            if let Some((_, routine)) = class.methods.iter().find(|(n, _)| n == name) {
                return Some((routine.clone(), origin.clone(), next));
            }
            next = Some(class.parent.clone());
        }
        None
    }

    /// Calls a method: the user's definition if there is one, else the built-in behaviour.
    pub(crate) fn send(&mut self, obj: &ObjRef, name: &str, args: Vec<VarRef>, above: Option<&Option<String>>) -> R<Value> {
        if let Some((routine, origin, level)) = self.find_method(obj, name, above) {
            let frame = Frame { this: Some(obj.clone()), method: Some((level, name.to_string())), ..Frame::default() };
            return self.call(routine, origin, args, frame);
        }
        let vals: Vec<Value> = args.iter().map(|a| a.borrow().clone()).collect();
        self.builtin_method(obj, name, &vals)
    }

    fn builtin_method(&mut self, obj: &ObjRef, name: &str, a: &[Value]) -> R<Value> {
        let text = |i: usize| match a.get(i) {
            Some(Value::Str(s)) => Ok(s.clone()),
            Some(_) => Err(err("Tipo de argumento no válido")),
            None => Err(err("Faltan argumentos")),
        };
        let index = |len: usize| match a.first() {
            Some(Value::Num(n)) if *n >= 1.0 && *n <= len as f64 => Ok(*n as usize - 1),
            _ => Err(err("Subíndice fuera de rango")),
        };
        match name {
            "SHOW" | "HIDE" => self.set_prop(obj, "VISIBLE", Value::Bool(name == "SHOW"))?,
            "RELEASE" => self.release(obj)?,
            "REFRESH" => {}
            "SETFOCUS" => self.focus = Some(obj.0.borrow().id),
            "ADDPROPERTY" => {
                let value = a.get(1).cloned().unwrap_or(Value::Bool(false));
                obj.0.borrow_mut().props.insert(text(0)?.to_uppercase(), value);
            }
            "ADDOBJECT" => {
                let child = self.create(&text(1)?.to_uppercase(), Some(obj), &text(0)?)?;
                // as in FoxPro, an object added at run time starts hidden
                child.0.borrow_mut().props.entry("VISIBLE".into()).and_modify(|v| *v = Value::Bool(false));
                self.init_tree(&child, a.iter().skip(2).cloned().map(var).collect())?;
            }
            "REMOVEOBJECT" => {
                let child = obj.0.borrow().props.get(&text(0)?.to_uppercase()).cloned();
                let Some(Value::Object(child)) = child else { return Err(err("Objeto no encontrado")) };
                self.release(&child)?;
            }
            "ADDITEM" => obj.0.borrow_mut().items.push(text(0)?),
            "CLEAR" => obj.0.borrow_mut().items.clear(),
            "REMOVEITEM" => {
                let mut o = obj.0.borrow_mut();
                let k = index(o.items.len())?;
                o.items.remove(k);
            }
            "LIST" => {
                let items = obj.0.borrow().list();
                return Ok(Value::Str(items[index(items.len())?].clone()));
            }
            _ => {
                // an event nobody handles does nothing
                let base = object::base(obj.0.borrow().base).unwrap();
                if !base.events.iter().any(|e| e.eq_ignore_ascii_case(name)) {
                    // an array property subscripted like a call
                    if let Ok(Value::Array(arr)) = self.get_prop(obj, name) {
                        let idx: Option<Vec<f64>> = a.iter().map(|v| if let Value::Num(n) = v { Some(*n) } else { None }).collect();
                        let k = arr.offset(&idx.ok_or_else(|| err("Se esperaba un valor numérico"))?)?;
                        return Ok(arr.data[k].clone());
                    }
                    return Err(err(format!("Método no encontrado: {name}")));
                }
            }
        }
        Ok(Value::Bool(true))
    }

    /// Runs Destroy and detaches the object from its parent, or from the open forms.
    pub(crate) fn release(&mut self, obj: &ObjRef) -> R<()> {
        self.send(obj, "DESTROY", Vec::new(), None)?;
        let parent = obj.0.borrow().parent();
        match parent {
            Some(parent) => {
                let mut p = parent.0.borrow_mut();
                let held = Value::Object(obj.clone());
                if let Some(name) = p.children.iter().find(|c| p.props.get(*c) == Some(&held)).cloned() {
                    p.children.retain(|c| *c != name);
                    p.props.remove(&name);
                }
            }
            None => self.forms.retain(|f| f != obj),
        }
        Ok(())
    }

    /// Builds an object of a built-in or user class, added objects included, without running Init.
    pub(crate) fn create(&mut self, class: &str, parent: Option<&ObjRef>, name: &str) -> R<ObjRef> {
        // the user classes, from the one asked for up to its built-in base
        let mut chain: Vec<Rc<ClassDef>> = Vec::new();
        let mut base_key = class.to_string();
        while let Some((c, _)) = self.classes.get(&base_key) {
            if chain.len() >= 64 {
                return Err(err(format!("La herencia de {class} es circular")));
            }
            chain.push(c.clone());
            base_key = c.parent.clone();
        }
        let Some(base) = object::base(&base_key) else {
            return Err(err(format!("Clase no encontrada: {base_key}")));
        };
        let mut props: HashMap<String, Value> = base.props.into_iter().map(|(k, v)| (k.to_uppercase(), v)).collect();
        for (key, value) in [("NAME", name), ("CLASS", class), ("BASECLASS", base.name), ("TAG", "")] {
            props.insert(key.into(), Value::Str(value.into()));
        }
        self.next_id += 1;
        let obj = ObjRef(Rc::new(RefCell::new(Object {
            id: self.next_id,
            base: base.key,
            class: chain.first().map(|c| c.name.clone()),
            props,
            children: Vec::new(),
            parent: parent.map(|p| Rc::downgrade(&p.0)),
            methods: HashMap::new(),
            items: Vec::new(),
        })));
        match parent {
            Some(parent) => {
                let key = name.to_uppercase();
                let mut p = parent.0.borrow_mut();
                if p.props.contains_key(&key) {
                    return Err(err(format!("Ya existe un miembro llamado {name}")));
                }
                p.props.insert(key.clone(), Value::Object(obj.clone()));
                p.children.push(key);
            }
            None if base.key == "FORM" => self.forms.push(obj.clone()),
            None => {}
        }
        // the expressions of a class definition see the new object as THIS
        self.frames.push(Frame { this: Some(obj.clone()), ..Frame::default() });
        let res = self.apply_classes(&obj, &chain);
        self.frames.pop();
        if res.is_err() {
            self.forms.retain(|f| *f != obj);
        }
        res.map(|_| obj)
    }

    /// Applies the class definitions to a new object, the base-most first.
    fn apply_classes(&mut self, obj: &ObjRef, chain: &[Rc<ClassDef>]) -> R<()> {
        for class in chain.iter().rev() {
            let origin = self.classes.get(&class.name).and_then(|(_, origin)| origin.clone());
            for t in &class.dims {
                let arr = Value::Array(Array::new(&self.nums(&t.idx)?)?);
                obj.0.borrow_mut().props.insert(t.name.clone(), arr);
            }
            for m in &class.members {
                let child = self.create(&m.class, Some(obj), &m.name)?;
                for (path, e) in &m.props {
                    let v = self.eval(e)?;
                    let (target, prop) = self.resolve(&child, path)?;
                    self.set_prop(&target, &prop, v)?;
                }
            }
            for (path, e) in &class.props {
                let v = self.eval(e)?;
                let (target, prop) = self.resolve(obj, path)?;
                // a plain `name = value` in a class declares the property
                if !path.contains('.') {
                    target.0.borrow_mut().props.entry(prop.clone()).or_insert(Value::Bool(false));
                }
                self.set_prop(&target, &prop, v)?;
            }
            // `PROCEDURE child.Event` belongs to the added object
            for (path, routine) in class.methods.iter().filter(|(path, _)| path.contains('.')) {
                let (target, method) = self.resolve(obj, path)?;
                target.0.borrow_mut().methods.insert(method, (routine.clone(), origin.clone()));
            }
        }
        Ok(())
    }

    /// Splits `CHILD.GRANDCHILD.NAME` into the object that owns the last name, and that name.
    fn resolve(&self, obj: &ObjRef, path: &str) -> R<(ObjRef, String)> {
        let mut parts: Vec<&str> = path.split('.').collect();
        let last = parts.pop().unwrap().to_string();
        let mut cur = obj.clone();
        for part in parts {
            cur = match self.get_prop(&cur, part)? {
                Value::Object(o) => o,
                _ => return Err(err(format!("{part} no es un objeto"))),
            };
        }
        Ok((cur, last))
    }

    /// Runs Init on an object's children, then on the object itself with `args`.
    pub(crate) fn init_tree(&mut self, obj: &ObjRef, args: Vec<VarRef>) -> R<()> {
        let kids = obj.0.borrow().kids();
        for kid in kids {
            self.init_tree(&kid, Vec::new())?;
        }
        self.send(obj, "INIT", args, None).map(|_| ())
    }

    /// Builds one node of a form file: `{ class, props, methods, controls }`, all optional.
    /// `label` says where the node is, for error messages.
    fn build(&mut self, node: &Json, parent: Option<&ObjRef>, label: &str) -> R<ObjRef> {
        let class = node["class"].as_str().unwrap_or("Form").to_uppercase();
        let name = node["props"]["Name"].as_str().unwrap_or(&class).to_string();
        let obj = self.create(&class, parent, &name)?;
        let label = if parent.is_some() { format!("{label}.{name}") } else { label.to_string() };
        for (key, v) in node["props"].as_object().into_iter().flatten().filter(|(key, _)| *key != "Name") {
            let v = from_json(v).ok_or_else(|| err(format!("{label}: el valor de {key} no es válido")))?;
            self.set_prop(&obj, &key.to_uppercase(), v).map_err(|e| err(format!("{label}: {}", e.msg)))?;
        }
        for (method, src) in node["methods"].as_object().into_iter().flatten() {
            let origin = format!("{label}.{method}");
            let fail = |line: usize, msg: String| Error { line, msg, file: Some(origin.clone()) };
            let prog = parse(src.as_str().unwrap_or("")).map_err(|e| fail(e.line, e.msg))?;
            if !prog.routines.is_empty() || !prog.classes.is_empty() {
                return Err(fail(1, "El código de un evento no puede definir procedimientos ni clases".into()));
            }
            if !prog.main.is_empty() {
                let routine = Rc::new(Routine { params: Vec::new(), body: prog.main });
                obj.0.borrow_mut().methods.insert(method.to_uppercase(), (routine, Some(origin)));
            }
        }
        for control in node["controls"].as_array().into_iter().flatten() {
            self.build(control, Some(&obj), &label)?;
        }
        Ok(obj)
    }

    /// Builds a form from the JSON of a form file, runs Load and the Init events, and shows it.
    fn open_form(&mut self, doc: &Json, label: &str, args: Vec<VarRef>, show: bool) -> R<ObjRef> {
        let before = self.forms.len();
        let opened = (|| {
            let form = self.build(doc, None, label)?;
            if form.0.borrow().base != "FORM" {
                return Err(err(format!("{label} no describe un formulario")));
            }
            self.send(&form, "LOAD", Vec::new(), None)?;
            self.init_tree(&form, args)?;
            if show {
                self.set_prop(&form, "VISIBLE", Value::Bool(true))?;
            }
            Ok(form)
        })();
        // a form that failed to open must not linger half built
        if opened.is_err() {
            self.forms.truncate(before);
        }
        opened
    }

    /// DO FORM: opens `name.fostform` from `dir`.
    pub(crate) fn load_form(&mut self, name: &str, args: Vec<VarRef>, show: bool) -> R<ObjRef> {
        let Some(file) = self.find_file(name, "fostform") else {
            return Err(err(format!("Formulario no encontrado: {name}")));
        };
        let src = std::fs::read_to_string(self.dir.join(&file)).map_err(|e| err(format!("No se pudo leer {file}: {e}")))?;
        let doc: Json = serde_json::from_str(&src).map_err(|e| err(format!("{file} no es un formulario válido: {e}")))?;
        self.open_form(&doc, &file, args, show)
    }

    /// Opens a form from the JSON text of a form file that is not on disk, as the designer's Run does.
    pub fn run_form(&mut self, json: &str, label: &str) -> Result<(), Error> {
        let doc: Json = serde_json::from_str(json).map_err(|e| err(format!("{label} no es un formulario válido: {e}")))?;
        self.retire();
        self.guarded(|it| it.open_form(&doc, label, Vec::new(), true).map(|_| ()))
    }

    /// A user action on a shown form: `props` are values the UI changed, `event` the method to run.
    /// The event CLOSE releases the form.
    pub fn ui_event(&mut self, id: u64, props: &Json, event: Option<&str>) -> Result<(), Error> {
        // the form may have been released while the event was on its way
        let Some(obj) = self.forms.iter().find_map(|f| find(f, id)) else { return Ok(()) };
        self.guarded(|it| {
            for (key, v) in props.as_object().into_iter().flatten() {
                let v = from_json(v).ok_or_else(|| err(format!("El valor de {key} no es válido")))?;
                obj.0.borrow_mut().props.insert(key.to_uppercase(), v);
            }
            match event.map(str::to_uppercase).as_deref() {
                Some("CLOSE") => it.release(&obj),
                Some(event) => it.send(&obj, event, Vec::new(), None).map(|_| ()),
                None => Ok(()),
            }
        })
    }

    /// The visible forms, as the tree a UI needs to draw them.
    pub fn forms_json(&self) -> Json {
        let visible = |f: &&ObjRef| f.0.borrow().props.get("VISIBLE") == Some(&Value::Bool(true));
        self.forms.iter().filter(visible).map(|f| self.snapshot(f)).collect()
    }

    fn snapshot(&self, obj: &ObjRef) -> Json {
        let o = obj.0.borrow();
        let props: serde_json::Map<String, Json> = o.props.iter().filter_map(|(k, v)| Some((k.clone(), to_json(v)?))).collect();
        // the events somebody handles: the UI only needs to report those
        let mut events: Vec<String> = o.methods.keys().cloned().collect();
        let mut next = o.class.clone();
        while let Some((class, _)) = next.as_ref().and_then(|n| self.classes.get(n)) {
            events.extend(class.methods.iter().map(|(name, _)| name.clone()));
            next = Some(class.parent.clone());
        }
        let children: Vec<Json> = o.kids().iter().map(|k| self.snapshot(k)).collect();
        json!({ "id": o.id, "base": o.base, "props": props, "items": o.list(), "events": events, "children": children })
    }
}
