use crate::parser::{Expr, Kind, Routine, Scope, Stmt, Target, parse, parse_expr};
use crate::value::{Array, Value, binary, unary};
use crate::{Error, R, builtins, err};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

type VarRef = Rc<RefCell<Value>>;

fn var(v: Value) -> VarRef {
    Rc::new(RefCell::new(v))
}

#[derive(Default)]
struct Frame {
    /// name -> (value, is LOCAL). Non-local variables are visible to called routines.
    vars: HashMap<String, (VarRef, bool)>,
    /// Names declared PRIVATE here: outer variables with these names are hidden.
    hidden: HashSet<String>,
    /// Arguments not yet claimed by a PARAMETERS / LPARAMETERS statement.
    args: Vec<VarRef>,
}

enum Flow {
    Next,
    Loop,
    Exit,
    Return(Value),
}

/// Keeps variables and routines between `run` calls, so it can back a command window.
pub struct Interp {
    routines: HashMap<String, Rc<Routine>>,
    /// frames[0] is the top level; PUBLIC variables live there.
    frames: Vec<Frame>,
    out: String,
    /// Set when CLEAR ran; the host should wipe what it has shown so far.
    pub clear: bool,
    /// Set from another thread to interrupt a running program.
    pub stop: Arc<AtomicBool>,
    rng: u64,
}

impl Default for Interp {
    fn default() -> Self {
        Interp {
            routines: HashMap::new(),
            frames: vec![Frame::default()],
            out: String::new(),
            clear: false,
            stop: Arc::default(),
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }
}

impl Interp {
    pub fn run(&mut self, src: &str) -> Result<(), Error> {
        let prog = parse(src)?;
        for (name, routine) in prog.routines {
            self.routines.insert(name, Rc::new(routine));
        }
        self.stop.store(false, Ordering::Relaxed);
        let res = self.block(&prog.main);
        self.frames.truncate(1);
        res.map(|_| ())
    }

    /// Text printed since the last call. `?` writes a newline *before* its values, as in FoxPro.
    pub fn take_output(&mut self) -> String {
        std::mem::take(&mut self.out)
    }

    fn top(&mut self) -> &mut Frame {
        self.frames.last_mut().unwrap()
    }

    fn lookup(&self, name: &str) -> Option<VarRef> {
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

    fn truthy(&mut self, e: &Expr) -> R<bool> {
        match self.eval(e)? {
            Value::Bool(b) => Ok(b),
            Value::Null => Ok(false),
            _ => Err(err("Se esperaba un valor lógico")),
        }
    }

    fn num(&mut self, e: &Expr) -> R<f64> {
        match self.eval(e)? {
            Value::Num(n) => Ok(n),
            _ => Err(err("Se esperaba un valor numérico")),
        }
    }

    fn nums(&mut self, exprs: &[Expr]) -> R<Vec<f64>> {
        exprs.iter().map(|e| self.num(e)).collect()
    }

    fn block(&mut self, stmts: &[Stmt]) -> R<Flow> {
        for s in stmts {
            let flow = self.exec(s).map_err(|mut e| {
                if e.line == 0 {
                    e.line = s.line;
                }
                e
            })?;
            if !matches!(flow, Flow::Next) {
                return Ok(flow);
            }
        }
        Ok(Flow::Next)
    }

    /// Runs a loop body; Some(flow) means the loop must stop and hand `flow` to its caller.
    fn iteration(&mut self, body: &[Stmt]) -> R<Option<Flow>> {
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
                    parts.push(self.eval(e)?.display());
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
                let counter = Target { name: name.clone(), idx: Vec::new() };
                self.assign(&counter, Value::Num(from))?;
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
                    self.assign(&counter, Value::Num(i + step))?;
                }
            }
            Kind::ForEach { var: name, iter, body } => {
                let Value::Array(items) = self.eval(iter)? else {
                    return Err(err("FOR EACH necesita un arreglo"));
                };
                let item = Target { name: name.clone(), idx: Vec::new() };
                for v in items.data {
                    self.assign(&item, v)?;
                    if let Some(flow) = self.iteration(body)? {
                        return Ok(flow);
                    }
                }
            }
            Kind::Loop => return Ok(Flow::Loop),
            Kind::Exit => return Ok(Flow::Exit),
            Kind::Do { name, args } => {
                let Some(routine) = self.routines.get(name).cloned() else {
                    return Err(err(format!("Procedimiento no encontrado: {name}")));
                };
                // DO ... WITH passes plain variables by reference
                let mut refs = Vec::new();
                for a in args {
                    refs.push(match a {
                        Expr::Var(n) | Expr::Ref(n) => self.get(n)?,
                        _ => var(self.eval(a)?),
                    });
                }
                self.call(routine, refs)?;
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
            Kind::Nop => {}
        }
        Ok(Flow::Next)
    }

    fn assign(&mut self, t: &Target, v: Value) -> R<()> {
        if !t.idx.is_empty() {
            let idx = self.nums(&t.idx)?;
            let r = self.get(&t.name)?;
            let mut cur = r.borrow_mut();
            let Value::Array(a) = &mut *cur else {
                return Err(err(format!("{} no es un arreglo", t.name)));
            };
            let k = a.offset(&idx)?;
            a.data[k] = v;
            return Ok(());
        }
        match self.lookup(&t.name) {
            Some(r) => match (&mut *r.borrow_mut(), v) {
                (cur, v @ Value::Array(_)) => *cur = v,
                // assigning a plain value to an array fills every element
                (Value::Array(a), v) => a.data.fill(v),
                (cur, v) => *cur = v,
            },
            None => {
                self.top().vars.insert(t.name.clone(), (var(v), false));
            }
        }
        Ok(())
    }

    fn call(&mut self, routine: Rc<Routine>, args: Vec<VarRef>) -> R<Value> {
        // 128 nested DO levels is also FoxPro's limit
        if self.frames.len() >= 128 {
            return Err(err("Demasiadas llamadas anidadas (máximo 128)"));
        }
        self.tick()?;
        let mut frame = Frame::default();
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
        Ok(match res? {
            Flow::Return(v) => v,
            _ => Value::Bool(true),
        })
    }

    fn eval(&mut self, e: &Expr) -> R<Value> {
        Ok(match e {
            Expr::Lit(v) => v.clone(),
            Expr::Var(n) => self.get(n)?.borrow().clone(),
            Expr::Ref(_) => return Err(err("'@' solo se permite en argumentos de una llamada")),
            Expr::Un(op, x) => unary(op, self.eval(x)?)?,
            Expr::Bin(op @ ("AND" | "OR"), l, r) => {
                // short-circuit, as FoxPro does
                let stop_on = *op == "OR";
                Value::Bool(if self.truthy(l)? == stop_on { stop_on } else { self.truthy(r)? })
            }
            Expr::Bin(op, l, r) => {
                let a = self.eval(l)?;
                binary(op, a, self.eval(r)?)?
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
        if let Some(routine) = self.routines.get(name).cloned() {
            let mut refs = Vec::new();
            for a in args {
                refs.push(match a {
                    Expr::Ref(n) => self.get(n)?,
                    _ => var(self.eval(a)?),
                });
            }
            return self.call(routine, refs);
        }
        let Some(full) = builtins::resolve(name) else {
            return Err(err(format!("Función no encontrada: {name}")));
        };
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
            "RAND" => {
                // xorshift64: enough for RAND(), not for anything security related
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
                Value::Num((self.rng >> 11) as f64 / (1u64 << 53) as f64)
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
