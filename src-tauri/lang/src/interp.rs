use crate::data::Data;
use crate::menu::Menus;
use crate::object::ObjRef;
use crate::parser::{ClassDef, Expr, Kind, Routine, Scope, Stmt, parse, parse_expr};
use crate::value::{Array, Value, binary, fmt_num, unary};
use crate::{Error, R, builtins, err};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) type VarRef = Rc<RefCell<Value>>;

pub(crate) fn var(v: Value) -> VarRef {
    Rc::new(RefCell::new(v))
}

/// What the interpreter needs from the program that hosts it.
pub trait Host {
    /// Shows MESSAGEBOX() and returns the code of the button pressed (1 OK, 2 Cancel, 6 Yes, 7 No).
    /// `buttons` is FoxPro's dialog type number. None means there is no UI to show it on.
    fn message_box(&mut self, _text: &str, _buttons: i64, _title: &str) -> Option<i64> {
        None
    }

    /// Asks INPUTBOX()'s question and returns what the user typed, empty if they cancelled.
    /// None means there is no UI to ask on.
    fn input_box(&mut self, _prompt: &str, _title: &str, _default: &str) -> Option<String> {
        None
    }

    /// Opens a web address in the user's browser. False means there is no UI to do it.
    fn open_url(&mut self, _url: &str) -> bool {
        false
    }
}

impl Host for () {}

/// A file MODIFY COMMAND, MODIFY FORM or CREATE FORM asked the IDE to open.
pub struct Open {
    pub form: bool,
    /// None for a new, untitled document. The file may not exist yet.
    pub path: Option<PathBuf>,
}

#[derive(Default)]
pub(crate) struct Frame {
    /// name -> (value, is LOCAL). Non-local variables are visible to called routines.
    pub(crate) vars: HashMap<String, (VarRef, bool)>,
    /// Names declared PRIVATE here: outer variables with these names are hidden.
    pub(crate) hidden: HashSet<String>,
    /// Arguments not yet claimed by a PARAMETERS / LPARAMETERS statement.
    pub(crate) args: Vec<VarRef>,
    /// THIS, when a method is running.
    pub this: Option<ObjRef>,
    /// The method running, for DODEFAULT(): the class that defines it (None: the object itself) and its name.
    pub method: Option<(Option<String>, String)>,
    /// Objects of the WITH blocks open in this routine.
    pub(crate) with: Vec<ObjRef>,
}

pub(crate) enum Flow {
    Next,
    Loop,
    Exit,
    Return(Value),
}

/// Something defined by a program, with where: None for the source being run now.
type Defined<T> = (Rc<T>, Option<String>);

/// Keeps variables, routines, classes and forms between calls, so it can back a command window.
pub struct Interp {
    routines: HashMap<String, Defined<Routine>>,
    pub(crate) classes: HashMap<String, Defined<ClassDef>>,
    /// Where `DO name` and `DO FORM name` look for files.
    pub dir: PathBuf,
    pub(crate) exact: bool,
    decimals: usize,
    /// frames[0] is the top level; PUBLIC variables live there.
    pub(crate) frames: Vec<Frame>,
    out: String,
    /// Set when CLEAR ran; the host should wipe what it has shown so far.
    pub clear: bool,
    /// Set from another thread to interrupt a running program.
    pub stop: Arc<AtomicBool>,
    rng: u64,
    /// Every form created and not yet released.
    // shortcut: a form stays alive until released, even when no variable refers to it any more
    pub(crate) forms: Vec<ObjRef>,
    pub(crate) next_id: u64,
    /// Id of the control SetFocus() was called on; the host should move the focus there.
    pub focus: Option<u64>,
    /// Set by MODIFY COMMAND and friends; the host should open that file.
    pub open: Option<Open>,
    /// Set by HELP; the host should show the reference.
    pub help: bool,
    pub host: Box<dyn Host>,
    /// The open tables. None until a program first touches one.
    pub(crate) data: Option<Data>,
    pub(crate) menus: Menus,
    /// ON ERROR: what runs when a statement fails outside a TRY.
    on_error: Option<Rc<Vec<Stmt>>>,
    /// The ON ERROR code is running: an error in it is not handled by itself.
    handling: bool,
    /// How many TRY blocks are open: inside one, errors go to its CATCH.
    trying: usize,
    /// The message and line of the last error a program handled, for MESSAGE() and LINENO().
    last_error: Option<(String, usize)>,
    /// What THROW threw, until the CATCH that takes it.
    thrown: Option<Value>,
}

impl Default for Interp {
    fn default() -> Self {
        Interp {
            routines: HashMap::new(),
            classes: HashMap::new(),
            dir: PathBuf::from("."),
            exact: false,
            decimals: 10,
            frames: vec![Frame::default()],
            out: String::new(),
            clear: false,
            stop: Arc::default(),
            rng: 0x2545_F491_4F6C_DD1D,
            forms: Vec::new(),
            next_id: 0,
            focus: None,
            open: None,
            help: false,
            host: Box::new(()),
            data: None,
            menus: Menus::default(),
            on_error: None,
            handling: false,
            trying: 0,
            last_error: None,
            thrown: None,
        }
    }
}

impl Interp {
    pub fn run(&mut self, src: &str) -> Result<(), Error> {
        let prog = parse(src)?;
        self.retire();
        for (name, routine) in prog.routines {
            self.routines.insert(name, (Rc::new(routine), None));
        }
        for class in prog.classes {
            self.classes.insert(class.name.clone(), (Rc::new(class), None));
        }
        self.guarded(|it| it.block(&prog.main).map(|_| ()))
    }

    /// Text printed since the last call. `?` writes a newline *before* its values, as in FoxPro.
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    /// Line numbers of routines and classes kept from earlier runs no longer refer to the source being run.
    pub(crate) fn retire(&mut self) {
        let origins = self.routines.values_mut().map(|(_, o)| o).chain(self.classes.values_mut().map(|(_, o)| o));
        for origin in origins {
            origin.get_or_insert_with(|| "un comando anterior".into());
        }
    }

    /// Runs one request from the host, leaving the interpreter ready for the next whatever happens.
    pub(crate) fn guarded(&mut self, f: impl FnOnce(&mut Self) -> R<()>) -> R<()> {
        self.stop.store(false, Ordering::Relaxed);
        (self.handling, self.trying) = (false, 0);
        let res = f(self);
        self.frames.truncate(1);
        self.frames[0].with.clear();
        res
    }

    pub(crate) fn top(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<VarRef> {
        let top = self.frames.len() - 1;
        for (depth, f) in self.frames.iter().enumerate().rev() {
            if let Some((v, local)) = f.vars.get(name) {
                if !local || depth == top {
                    return Some(v.clone());
                }
            }
            if f.hidden.contains(name) {
                return None;
            }
        }
        None
    }

    fn get(&self, name: &str) -> R<VarRef> {
        self.lookup(name).ok_or_else(|| err(format!("Variable no encontrada: {name}")))
    }

    fn tick(&self) -> R<()> {
        if self.stop.load(Ordering::Relaxed) { Err(err("Ejecución interrumpida")) } else { Ok(()) }
    }

    pub(crate) fn truthy(&mut self, e: &Expr) -> R<bool> {
        match self.eval(e)? {
            Value::Bool(b) => Ok(b),
            Value::Null => Ok(false),
            _ => Err(err("Se esperaba un valor lógico")),
        }
    }

    pub(crate) fn num(&mut self, e: &Expr) -> R<f64> {
        match self.eval(e)? {
            Value::Num(n) => Ok(n),
            _ => Err(err("Se esperaba un valor numérico")),
        }
    }

    pub(crate) fn nums(&mut self, exprs: &[Expr]) -> R<Vec<f64>> {
        exprs.iter().map(|e| self.num(e)).collect()
    }

    /// Arguments for a call: `@name` shares the variable, anything else is passed by value.
    pub(crate) fn arg_refs(&mut self, args: &[Expr]) -> R<Vec<VarRef>> {
        let mut refs = Vec::new();
        for a in args {
            refs.push(match a {
                Expr::Ref(n) => self.get(n)?,
                _ => var(self.eval(a)?),
            });
        }
        Ok(refs)
    }

    /// Arguments for DO ... WITH, which also passes plain variables by reference.
    fn do_refs(&mut self, args: &[Expr]) -> R<Vec<VarRef>> {
        let mut refs = Vec::new();
        for a in args {
            refs.push(match a {
                // a name that is a field of the open table is a value like any other
                Expr::Var(n) if self.field(n)?.is_none() => self.get(n)?,
                Expr::MemVar(n) | Expr::Ref(n) => self.get(n)?,
                _ => var(self.eval(a)?),
            });
        }
        Ok(refs)
    }

    pub(crate) fn block(&mut self, stmts: &[Stmt]) -> R<Flow> {
        for s in stmts {
            let flow = match self.exec(s) {
                Ok(flow) => flow,
                Err(mut e) => {
                    if e.line == 0 {
                        e.line = s.line;
                    }
                    // ON ERROR takes it, and the program goes on with the next statement
                    let handler = self.on_error.clone().filter(|_| !self.handling && self.trying == 0 && self.tick().is_ok());
                    let Some(handler) = handler else { return Err(e) };
                    self.last_error = Some((e.msg, e.line));
                    self.handling = true;
                    let handled = self.block(&handler);
                    self.handling = false;
                    handled?;
                    Flow::Next
                }
            };
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    /// Runs a loop body; Some(flow) means the loop must stop and hand `flow` to its caller.
    pub(crate) fn iteration(&mut self, body: &[Stmt]) -> R<Option<Flow>> {
        self.tick()?;
        Ok(match self.block(body)? {
            Flow::Exit => Some(Flow::Next),
            f @ Flow::Return(_) => Some(f),
            Flow::Next | Flow::Loop => None,
        })
    }

    fn exec(&mut self, s: &Stmt) -> R<Flow> {
        match &s.kind {
            Kind::Print { newline, exprs } => {
                let mut parts = Vec::new();
                for e in exprs {
                    parts.push(match self.eval(e)? {
                        Value::Num(n) => fmt_num(n, self.decimals),
                        v => v.display(),
                    });
                }
                if *newline {
                    self.out.push('\n');
                }
                self.out.push_str(&parts.join(" "));
            }
            Kind::Assign { targets, value } => {
                let v = self.eval(value)?;
                for t in targets {
                    self.assign(t, v.clone())?;
                }
            }
            Kind::Expr(e) => {
                self.eval(e)?;
            }
            Kind::Declare { scope, items } => {
                for t in items {
                    let init = if t.idx.is_empty() {
                        Value::Bool(false)
                    } else {
                        Value::Array(Array::new(&self.nums(&t.idx)?)?)
                    };
                    let name = t.name.clone();
                    match scope {
                        Scope::Local => {
                            self.top().vars.insert(name, (var(init), true));
                        }
                        Scope::Public => {
                            self.frames[0].vars.entry(name).or_insert((var(init), false));
                        }
                        Scope::Private if t.idx.is_empty() => {
                            self.top().hidden.insert(name);
                        }
                        // shortcut: redimensioning discards the contents, upgrade if programs rely on them surviving
                        _ => match self.lookup(&name).filter(|_| *scope == Scope::Dimension) {
                            Some(existing) => *existing.borrow_mut() = init,
                            None => {
                                self.top().vars.insert(name, (var(init), false));
                            }
                        },
                    }
                }
            }
            Kind::If { cond, then, els } => {
                return if self.truthy(cond)? { self.block(then) } else { self.block(els) };
            }
            Kind::Case { arms, otherwise } => {
                for (cond, body) in arms {
                    if self.truthy(cond)? {
                        return self.block(body);
                    }
                }
                return self.block(otherwise);
            }
            Kind::While { cond, body } => {
                while self.truthy(cond)? {
                    if let Some(flow) = self.iteration(body)? {
                        return Ok(flow);
                    }
                }
            }
            Kind::For { var: name, from, to, step, body } => {
                let (from, to) = (self.num(from)?, self.num(to)?);
                let step = match step {
                    Some(e) => self.num(e)?,
                    None => 1.0,
                };
                self.set_var(name, Value::Num(from));
                // the counter is re-read each pass: the body may change it
                loop {
                    let Value::Num(i) = *self.get(name)?.borrow() else {
                        return Err(err("El contador del FOR debe ser numérico"));
                    };
                    if (step >= 0.0 && i > to) || (step < 0.0 && i < to) {
                        break;
                    }
                    if let Some(flow) = self.iteration(body)? {
                        return Ok(flow);
                    }
                    let Value::Num(i) = *self.get(name)?.borrow() else {
                        return Err(err("El contador del FOR debe ser numérico"));
                    };
                    self.set_var(name, Value::Num(i + step));
                }
            }
            Kind::ForEach { var: name, iter, body } => {
                let Value::Array(items) = self.eval(iter)? else {
                    return Err(err("FOR EACH necesita un arreglo"));
                };
                for v in items.data {
                    self.set_var(name, v);
                    if let Some(flow) = self.iteration(body)? {
                        return Ok(flow);
                    }
                }
            }
            Kind::With { object, body } => {
                let obj = self.object(object)?;
                self.top().with.push(obj);
                let res = self.block(body);
                self.top().with.pop();
                return res;
            }
            Kind::Loop => return Ok(Flow::Loop),
            Kind::Exit => return Ok(Flow::Exit),
            Kind::Do { name, args } => {
                let (routine, origin) = match self.routines.get(name).cloned() {
                    Some(found) => found,
                    None => self.load(name)?,
                };
                let refs = self.do_refs(args)?;
                self.call(routine, origin, refs, Frame::default())?;
            }
            Kind::DoForm { name, args, var: holder, show } => {
                let refs = self.do_refs(args)?;
                let form = self.load_form(name, refs, *show)?;
                if let Some(holder) = holder {
                    self.set_var(holder, Value::Object(form));
                }
            }
            Kind::Open { form, name } => {
                let ext = if *form { "fostform" } else { "fost" };
                let file = |it: &Self, n: &String| it.find_file(n, ext).unwrap_or(format!("{}.{ext}", n.to_lowercase()));
                self.open = Some(Open { form: *form, path: name.as_ref().map(|n| self.dir.join(file(self, n))) });
            }
            Kind::Return(e) => {
                return Ok(Flow::Return(match e {
                    Some(e) => self.eval(e)?,
                    None => Value::Bool(true),
                }));
            }
            Kind::Params { local, names } => {
                let args = std::mem::take(&mut self.top().args);
                if args.len() > names.len() {
                    return Err(err("Se pasaron más argumentos de los declarados"));
                }
                let mut args = args.into_iter();
                for n in names {
                    let v = args.next().unwrap_or_else(|| var(Value::Bool(false)));
                    self.top().vars.insert(n.clone(), (v, *local));
                }
            }
            Kind::Release(names) => {
                if names.is_empty() {
                    self.top().vars.clear();
                }
                for n in names {
                    for f in self.frames.iter_mut().rev() {
                        if f.vars.remove(n).is_some() {
                            break;
                        }
                    }
                }
            }
            Kind::Clear => {
                self.out.clear();
                self.clear = true;
            }
            Kind::Set(option, value) => match *option {
                "EXACT" => self.exact = self.truthy(value)?,
                "DELETED" => {
                    let on = self.truthy(value)?;
                    self.set_deleted(on)?;
                }
                _ => {
                    let n = self.num(value)?;
                    if !(0.0..=18.0).contains(&n) {
                        return Err(err("SET DECIMALS admite de 0 a 18"));
                    }
                    self.decimals = n as usize;
                }
            },
            // shortcut: procedures are never taken away, so with or without ADDITIVE the files add up,
            // and SET PROCEDURE TO alone does nothing; upgrade if a program relies on one file hiding another
            Kind::Procedures(files) => {
                for file in files {
                    // its procedures and classes are kept; its main code is not run
                    self.load(file)?;
                }
            }
            Kind::Help => self.help = true,
            Kind::Try { body, var, catch, finally } => {
                self.trying += 1;
                let mut res = self.block(body);
                self.trying -= 1;
                // a program the user stopped is not an error to catch
                if let (Err(e), Some(catch), true) = (&res, catch, self.tick().is_ok()) {
                    self.last_error = Some((e.msg.clone(), e.line));
                    let thrown = self.thrown.take();
                    if let Some(var) = var {
                        let exception = self.create("CUSTOM", None, "Exception")?;
                        let text = |s: &str| Value::Str(s.to_string());
                        let props = [
                            ("MESSAGE", text(&e.msg)),
                            ("LINENO", Value::Num(e.line as f64)),
                            ("PROCEDURE", text(e.file.as_deref().unwrap_or(""))),
                            // shortcut: errors have no FoxPro numbers; 2071 is the one of THROW, 1 any other
                            ("ERRORNO", Value::Num(if thrown.is_some() { 2071.0 } else { 1.0 })),
                            ("USERVALUE", thrown.unwrap_or(Value::Str(String::new()))),
                        ];
                        exception.0.borrow_mut().props.extend(props.map(|(k, v)| (k.to_string(), v)));
                        self.set_var(var, Value::Object(exception));
                    }
                    res = self.block(catch);
                }
                // FINALLY runs whatever happened, and what it returns wins
                let last = self.block(finally)?;
                return if matches!(last, Flow::Next) { res } else { Ok(last) };
            }
            Kind::Throw(e) => {
                let v = match e {
                    Some(e) => self.eval(e)?,
                    None => Value::Str("Error lanzado por el programa".into()),
                };
                let msg = v.display();
                self.thrown = Some(v);
                return Err(err(msg));
            }
            Kind::OnError(code) => self.on_error = Some(code.clone()).filter(|c| !c.is_empty()),
            Kind::Text { var, additive, merge, show, text } => {
                let mut text = text.clone();
                if *merge {
                    // <<expression>> is replaced by its value
                    let mut out = String::new();
                    let mut rest = text.as_str();
                    while let Some((before, after)) = rest.split_once("<<") {
                        let Some((inside, after)) = after.split_once(">>") else { break };
                        out.push_str(before);
                        out.push_str(&self.eval(&parse_expr(inside)?)?.display());
                        rest = after;
                    }
                    text = out + rest;
                }
                match var {
                    Some(var) => {
                        let old = match self.lookup(var).filter(|_| *additive).map(|v| v.borrow().clone()) {
                            Some(Value::Str(s)) => s,
                            _ => String::new(),
                        };
                        self.set_var(var, Value::Str(old + &text));
                    }
                    None if *show => self.print(&text),
                    None => {}
                }
            }
            Kind::Macro(src) => {
                let text = self.expand(src)?;
                // the line of an error in it is the line of the statement itself
                let unplaced = |mut e: Error| {
                    e.line = 0;
                    e
                };
                let prog = parse(&text).map_err(unplaced)?;
                if !prog.routines.is_empty() || !prog.classes.is_empty() {
                    return Err(err("Una macro no puede definir procedimientos ni clases"));
                }
                return self.block(&prog.main).map_err(unplaced);
            }
            Kind::Data(cmd) => self.exec_data(cmd)?,
            Kind::Menu(cmd) => self.exec_menu(cmd)?,
            Kind::Scan { scope, body } => return self.scan(scope, body),
            Kind::Nop => {}
        }
        Ok(Flow::Next)
    }

    /// A statement with each `&name` replaced by the text that variable holds. A dot ends the name: `&campo.1`.
    // shortcut: a macro is a whole statement of one line, so `IF &cond` cannot open a block, and
    // `&name` inside a string is replaced too; upgrade when programs need either
    fn expand(&self, src: &str) -> R<String> {
        let c: Vec<char> = src.chars().collect();
        let (mut out, mut i) = (String::new(), 0);
        while i < c.len() {
            let next = c.get(i + 1).copied().unwrap_or(' ');
            if c[i] == '&' && next == '&' {
                break;
            }
            if c[i] == '&' && (next.is_alphabetic() || next == '_') {
                let end = (i + 1..c.len()).find(|&j| !(c[j].is_alphanumeric() || c[j] == '_')).unwrap_or(c.len());
                let name: String = c[i + 1..end].iter().collect::<String>().to_uppercase();
                let Value::Str(text) = self.get(&name)?.borrow().clone() else {
                    return Err(err(format!("La variable de la macro &{name} debe contener texto")));
                };
                out.push_str(&text);
                i = end + (c.get(end) == Some(&'.')) as usize;
                continue;
            }
            out.push(c[i]);
            i += 1;
        }
        Ok(out)
    }

    /// The file `name.ext` in `dir`, whatever its case: the lexer upper-cased the name.
    pub(crate) fn find_file(&self, name: &str, ext: &str) -> Option<String> {
        let wanted = format!("{name}.{ext}").to_uppercase();
        let entries = std::fs::read_dir(&self.dir).ok().into_iter().flatten().flatten();
        entries.map(|e| e.file_name().to_string_lossy().into_owned()).find(|f| f.to_uppercase() == wanted)
    }

    /// Reads `name.fost` from `dir` for DO. Its procedures and classes become available to everyone.
    // shortcut: bare names in one directory only; upgrade when SET PATH or `DO path\file` is needed
    fn load(&mut self, name: &str) -> R<Defined<Routine>> {
        let Some(file) = self.find_file(name, "fost") else {
            return Err(err(format!("Procedimiento o programa no encontrado: {name}")));
        };
        let tag = |mut e: Error| {
            e.file.get_or_insert_with(|| file.clone());
            e
        };
        let src = std::fs::read_to_string(self.dir.join(&file)).map_err(|e| err(format!("No se pudo leer {file}: {e}")))?;
        let prog = parse(&src).map_err(tag)?;
        for (name, routine) in prog.routines {
            self.routines.insert(name, (Rc::new(routine), Some(file.clone())));
        }
        for class in prog.classes {
            self.classes.insert(class.name.clone(), (Rc::new(class), Some(file.clone())));
        }
        Ok((Rc::new(Routine { params: Vec::new(), body: prog.main }), Some(file)))
    }

    /// A plain value assigned to an array fills every element; anything else replaces the slot.
    fn store(slot: &mut Value, v: Value) {
        match (slot, v) {
            (slot, v @ Value::Array(_)) => *slot = v,
            (Value::Array(a), v) => a.data.fill(v),
            (slot, v) => *slot = v,
        }
    }

    fn store_element(slot: &mut Value, idx: &[f64], v: Value) -> R<()> {
        let Value::Array(a) = slot else { return Err(err("No es un arreglo")) };
        let k = a.offset(idx)?;
        a.data[k] = v;
        Ok(())
    }

    pub(crate) fn set_var(&mut self, name: &str, v: Value) {
        match self.lookup(name) {
            Some(r) => Self::store(&mut r.borrow_mut(), v),
            None => {
                self.top().vars.insert(name.to_string(), (var(v), false));
            }
        }
    }

    /// Adds a line to the output, as `?` does.
    pub(crate) fn print(&mut self, text: &str) {
        self.out.push('\n');
        self.out.push_str(text);
    }

    pub(crate) fn set_prop(&mut self, obj: &ObjRef, name: &str, v: Value) -> R<()> {
        if matches!(name, "PAGECOUNT" | "BUTTONCOUNT" | "COLUMNCOUNT") {
            return self.set_count(obj, v);
        }
        match obj.0.borrow_mut().props.get_mut(name) {
            Some(slot) => Self::store(slot, v),
            None => return Err(err(format!("Propiedad no encontrada: {name}"))),
        }
        match name {
            "VALUE" => self.push_bound(obj)?,
            // a new source is loaded before anything is written to it
            "CONTROLSOURCE" => obj.0.borrow_mut().seen = None,
            _ => {}
        }
        Ok(())
    }

    fn assign(&mut self, place: &Expr, v: Value) -> R<()> {
        match place {
            Expr::Var(name) | Expr::MemVar(name) => self.set_var(name, v),
            Expr::Call(name, idx) => {
                let idx = self.nums(idx)?;
                Self::store_element(&mut self.get(name)?.borrow_mut(), &idx, v)?;
            }
            Expr::Member(object, name) => {
                let obj = self.object(object)?;
                self.set_prop(&obj, name, v)?;
            }
            Expr::Method(object, name, idx) => {
                let obj = self.object(object)?;
                let idx = self.nums(idx)?;
                match obj.0.borrow_mut().props.get_mut(name) {
                    Some(slot) => Self::store_element(slot, &idx, v)?,
                    None => return Err(err(format!("Propiedad no encontrada: {name}"))),
                }
            }
            _ => return Err(err("No se puede asignar a esta expresión")),
        }
        Ok(())
    }

    /// Runs a routine in `frame`, which may already carry THIS for a method.
    pub(crate) fn call(&mut self, routine: Rc<Routine>, origin: Option<String>, args: Vec<VarRef>, mut frame: Frame) -> R<Value> {
        // 128 nested DO levels is also FoxPro's limit
        if self.frames.len() >= 128 {
            return Err(err("Demasiadas llamadas anidadas (máximo 128)"));
        }
        self.tick()?;
        if routine.params.is_empty() {
            frame.args = args;
        } else {
            if args.len() > routine.params.len() {
                return Err(err("Se pasaron más argumentos de los declarados"));
            }
            let mut args = args.into_iter();
            for p in &routine.params {
                let v = args.next().unwrap_or_else(|| var(Value::Bool(false)));
                frame.vars.insert(p.clone(), (v, true));
            }
        }
        self.frames.push(frame);
        let res = self.block(&routine.body);
        self.frames.pop();
        let res = res.map_err(|mut e| {
            if e.file.is_none() {
                e.file = origin;
            }
            e
        });
        Ok(match res? {
            Flow::Return(v) => v,
            _ => Value::Bool(true),
        })
    }

    pub(crate) fn eval(&mut self, e: &Expr) -> R<Value> {
        Ok(match e {
            Expr::Lit(v) => v.clone(),
            Expr::Var(n) if n == "THIS" || n == "THISFORM" => Value::Object(self.this(n == "THISFORM")?),
            // as in FoxPro, a field of the open table wins over a variable of the same name
            Expr::Var(n) => match self.field(n)? {
                Some(v) => v,
                None => self.get(n)?.borrow().clone(),
            },
            Expr::MemVar(n) => self.get(n)?.borrow().clone(),
            Expr::Ref(_) => return Err(err("'@' solo se permite en argumentos de una llamada")),
            Expr::WithObj => match self.top().with.last() {
                Some(obj) => Value::Object(obj.clone()),
                None => return Err(err("'.nombre' solo puede usarse dentro de WITH")),
            },
            Expr::Member(object, name) => {
                // `alias.field`, unless a variable has that name
                if let Expr::Var(alias) = &**object {
                    if self.lookup(alias).is_none() {
                        if let Some(v) = self.alias_field(alias, name)? {
                            return Ok(v);
                        }
                    }
                }
                let obj = self.object(object)?;
                self.get_prop(&obj, name)?
            }
            Expr::Method(object, name, args) => {
                let obj = self.object(object)?;
                let refs = self.arg_refs(args)?;
                self.send(&obj, name, refs, None)?
            }
            Expr::Un(op, x) => unary(op, self.eval(x)?)?,
            Expr::Bin(op @ ("AND" | "OR"), l, r) => {
                // short-circuit, as FoxPro does
                let stop_on = *op == "OR";
                Value::Bool(if self.truthy(l)? == stop_on { stop_on } else { self.truthy(r)? })
            }
            Expr::Bin(op, l, r) => {
                let a = self.eval(l)?;
                binary(op, a, self.eval(r)?, self.exact)?
            }
            Expr::Call(name, args) => self.call_expr(name, args)?,
        })
    }

    fn call_expr(&mut self, name: &str, args: &[Expr]) -> R<Value> {
        if let Some(r) = self.lookup(name) {
            if matches!(*r.borrow(), Value::Array(_)) {
                let idx = self.nums(args)?;
                let cur = r.borrow();
                let Value::Array(a) = &*cur else { unreachable!() };
                return Ok(a.data[a.offset(&idx)?].clone());
            }
        }
        if let Some((routine, origin)) = self.routines.get(name).cloned() {
            let refs = self.arg_refs(args)?;
            return self.call(routine, origin, refs, Frame::default());
        }
        let Some(full) = builtins::resolve(name) else {
            return Err(err(format!("Función no encontrada: {name}")));
        };
        if let Some(v) = self.menus.function(full) {
            return Ok(v);
        }
        if let Some(v) = self.data_function(full, args)? {
            return Ok(v);
        }
        let arg = |i: usize| args.get(i).ok_or_else(|| err("Faltan argumentos"));
        let text = |it: &mut Self, i: usize| match it.eval(arg(i)?)? {
            Value::Str(s) => Ok(s),
            _ => Err(err("Tipo de argumento no válido")),
        };
        // the functions that need the interpreter itself, or lazy arguments
        Ok(match full {
            "IIF" => {
                let branch = if self.truthy(arg(0)?)? { 1 } else { 2 };
                self.eval(arg(branch)?)?
            }
            "EVALUATE" => {
                let src = text(self, 0)?;
                self.eval(&parse_expr(&src)?)?
            }
            "TYPE" => {
                let src = text(self, 0)?;
                let t = parse_expr(&src).and_then(|e| self.eval(&e)).map_or('U', |v| v.type_char());
                Value::Str(t.into())
            }
            "VARTYPE" => Value::Str(self.eval(arg(0)?).map_or('U', |v| v.type_char()).into()),
            "MESSAGE" => Value::Str(self.last_error.as_ref().map_or(String::new(), |e| e.0.clone())),
            "LINENO" => Value::Num(self.last_error.as_ref().map_or(0, |e| e.1) as f64),
            // shortcut: errors have no FoxPro numbers, so this only says whether there was one
            "ERROR" => Value::Num(self.last_error.is_some() as u8 as f64),
            "RAND" => {
                // xorshift64: enough for RAND(), not for anything security related
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
                Value::Num((self.rng >> 11) as f64 / (1u64 << 53) as f64)
            }
            "CREATEOBJECT" => {
                let class = text(self, 0)?.to_uppercase();
                let refs = self.arg_refs(&args[1..])?;
                let obj = self.create(&class, None, &class)?;
                self.init_tree(&obj, refs)?;
                Value::Object(obj)
            }
            "DODEFAULT" => {
                let frame = self.top();
                let (Some(this), Some((level, method))) = (frame.this.clone(), frame.method.clone()) else {
                    return Err(err("DODEFAULT solo puede usarse dentro de un método"));
                };
                let refs = self.arg_refs(args)?;
                self.send(&this, &method, refs, Some(&level))?
            }
            "MESSAGEBOX" => {
                let message = self.eval(arg(0)?)?.display();
                // the second argument is the dialog type, or the title when it is text
                let (mut buttons, mut title) = (0, String::new());
                for a in args.iter().skip(1).take(2) {
                    match self.eval(a)? {
                        Value::Num(n) => buttons = n as i64,
                        v => title = v.display(),
                    }
                }
                let answer = self.host.message_box(&message, buttons, &title).unwrap_or_else(|| {
                    // no UI: print it and answer OK
                    self.print(&message);
                    1
                });
                Value::Num(answer as f64)
            }
            "INPUTBOX" => {
                let mut texts = Vec::new();
                for a in args.iter().take(3) {
                    texts.push(self.eval(a)?.display());
                }
                texts.resize(3, String::new());
                // no UI: answer with the default
                let answer = self.host.input_box(&texts[0], &texts[1], &texts[2]).unwrap_or_else(|| texts[2].clone());
                Value::Str(answer)
            }
            _ => {
                let mut vals = Vec::new();
                for a in args {
                    vals.push(self.eval(a)?);
                }
                builtins::call(full, &vals)?
            }
        })
    }
}
