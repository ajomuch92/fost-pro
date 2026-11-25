use crate::lexer::{Line, Tok, lex};
use crate::value::Value;
use crate::{Error, R, is_kw};

#[derive(Debug)]
pub enum Expr {
    Lit(Value),
    Var(String),
    /// `@name`: an argument passed by reference.
    Ref(String),
    /// `name(args)` or `name[args]`: a function call or an array element, decided at run time.
    Call(String, Vec<Expr>),
    Un(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub struct Stmt {
    pub line: usize,
    pub kind: Kind,
}

#[derive(Debug)]
pub struct Target {
    pub name: String,
    pub idx: Vec<Expr>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scope {
    Local,
    Private,
    Public,
    Dimension,
}

#[derive(Debug)]
pub enum Kind {
    Print { newline: bool, exprs: Vec<Expr> },
    Assign { targets: Vec<Target>, value: Expr },
    Expr(Expr),
    Declare { scope: Scope, items: Vec<Target> },
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    Case { arms: Vec<(Expr, Vec<Stmt>)>, otherwise: Vec<Stmt> },
    For { var: String, from: Expr, to: Expr, step: Option<Expr>, body: Vec<Stmt> },
    ForEach { var: String, iter: Expr, body: Vec<Stmt> },
    While { cond: Expr, body: Vec<Stmt> },
    Loop,
    Exit,
    Do { name: String, args: Vec<Expr> },
    Return(Option<Expr>),
    Params { local: bool, names: Vec<String> },
    /// Empty means RELEASE ALL.
    Release(Vec<String>),
    Clear,
    Nop,
}

pub struct Routine {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

pub struct Program {
    pub main: Vec<Stmt>,
    pub routines: Vec<(String, Routine)>,
}

// SET options that only affect the classic FoxPro UI, safe to accept and ignore
const SET_IGNORED: &[&str] = &[
    "TALK", "ECHO", "SAFETY", "STATUS", "BELL", "CONSOLE", "ESCAPE", "NOTIFY", "HEADINGS", "CONFIRM", "SCOREBOARD",
    "CURSOR", "SYSMENU", "RESOURCE", "HELP", "DEVELOPMENT", "CLOCK", "MESSAGE",
];
const BLOCK_ENDS: &[&str] =
    &["ENDIF", "ELSE", "ENDDO", "ENDFOR", "NEXT", "ENDCASE", "CASE", "OTHERWISE", "ENDPROC", "ENDFUNC"];
const ROUTINE: &[&str] = &["PROCEDURE", "FUNCTION"];

struct Parser {
    lines: Vec<Line>,
    ln: usize,
    i: usize,
    loops: usize,
}

pub fn parse(src: &str) -> R<Program> {
    let mut p = Parser { lines: lex(src)?, ln: 0, i: 0, loops: 0 };
    let main = p.block(ROUTINE, None)?;
    let mut routines = Vec::new();
    while !p.eof() {
        if !ROUTINE.iter().any(|k| p.starts(k)) {
            return p.fail("Después de un procedimiento solo puede ir otro PROCEDURE o FUNCTION");
        }
        p.i = 1;
        let name = p.ident()?;
        let mut params = Vec::new();
        if p.eat_op("(") && !p.eat_op(")") {
            loop {
                params.push(p.ident()?);
                p.skip_as()?;
                if p.eat_op(")") {
                    break;
                }
                p.expect_op(",")?;
            }
        }
        p.skip_as()?;
        p.eol()?;
        let body = p.block(&["PROCEDURE", "FUNCTION", "ENDPROC", "ENDFUNC"], None)?;
        if p.starts("ENDPROC") || p.starts("ENDFUNC") {
            p.skip_line();
        }
        routines.push((name, Routine { params, body }));
    }
    Ok(Program { main, routines })
}

pub fn parse_expr(src: &str) -> R<Expr> {
    let mut p = Parser { lines: lex(src)?, ln: 0, i: 0, loops: 0 };
    let e = p.expr()?;
    if p.peek().is_some() || p.lines.len() > 1 {
        return p.fail("Expresión no válida");
    }
    Ok(e)
}

impl Parser {
    fn eof(&self) -> bool {
        self.ln >= self.lines.len()
    }

    fn toks(&self) -> &[Tok] {
        self.lines.get(self.ln).map_or(&[], |l| &l.toks)
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks().get(self.i)
    }

    fn line(&self) -> usize {
        self.lines.get(self.ln).or(self.lines.last()).map_or(1, |l| l.no)
    }

    fn fail<T>(&self, msg: impl Into<String>) -> R<T> {
        Err(Error { line: self.line(), msg: msg.into() })
    }

    fn at_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == op)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        let hit = self.at_op(op);
        self.i += hit as usize;
        hit
    }

    fn expect_op(&mut self, op: &str) -> R<()> {
        if self.eat_op(op) { Ok(()) } else { self.fail(format!("Se esperaba '{op}'")) }
    }

    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Ident(w)) if is_kw(w, kw))
    }

    fn eat_kw(&mut self, kw: &str) -> bool {
        let hit = self.at_kw(kw);
        self.i += hit as usize;
        hit
    }

    fn expect_kw(&mut self, kw: &str) -> R<()> {
        if self.eat_kw(kw) { Ok(()) } else { self.fail(format!("Se esperaba {kw}")) }
    }

    fn ident(&mut self) -> R<String> {
        match self.peek() {
            Some(Tok::Ident(w)) => {
                let w = w.clone();
                self.i += 1;
                Ok(w)
            }
            _ => self.fail("Se esperaba un nombre"),
        }
    }

    /// Accepts and ignores an `AS type` clause.
    fn skip_as(&mut self) -> R<()> {
        if self.eat_kw("AS") {
            self.ident()?;
        }
        Ok(())
    }

    fn eol(&mut self) -> R<()> {
        if self.peek().is_some() {
            return self.fail("Sobra texto al final de la sentencia");
        }
        self.skip_line();
        Ok(())
    }

    /// FoxPro ignores anything written after ENDIF, ENDDO, NEXT and friends.
    fn skip_line(&mut self) {
        self.ln += 1;
        self.i = 0;
    }

    /// Whether the current line is the command `kw` (and not an assignment to a variable so named).
    fn starts(&self, kw: &str) -> bool {
        self.i == 0 && self.at_kw(kw) && self.toks().get(1) != Some(&Tok::Op("="))
    }

    /// Statements up to, not including, a line starting with one of `ends`.
    /// `opener` names the block being closed; None means end of file also closes it.
    fn block(&mut self, ends: &[&str], opener: Option<(&str, usize)>) -> R<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            if self.eof() {
                return match opener {
                    Some((name, line)) => Err(Error { line, msg: format!("{name} sin cerrar") }),
                    None => Ok(out),
                };
            }
            if ends.iter().any(|k| self.starts(k)) {
                return Ok(out);
            }
            let line = self.line();
            out.push(Stmt { line, kind: self.stmt(line)? });
        }
    }

    fn loop_body(&mut self, ends: &[&str], opener: (&str, usize)) -> R<Vec<Stmt>> {
        self.loops += 1;
        let body = self.block(ends, Some(opener));
        self.loops -= 1;
        self.skip_line();
        body
    }

    fn list<T>(&mut self, mut item: impl FnMut(&mut Self) -> R<T>) -> R<Vec<T>> {
        let mut out = vec![item(self)?];
        while self.eat_op(",") {
            out.push(item(self)?);
        }
        Ok(out)
    }

    /// A name with optional subscripts: `x`, `a[1]`, `a(1, 2)`.
    fn target(&mut self) -> R<Target> {
        let name = self.ident()?;
        let idx = if self.eat_op("(") {
            self.args(")")?
        } else if self.eat_op("[") {
            self.args("]")?
        } else {
            Vec::new()
        };
        Ok(Target { name, idx })
    }

    fn stmt(&mut self, line: usize) -> R<Kind> {
        let word = match self.peek().cloned() {
            Some(Tok::Op(op @ ("?" | "??"))) => {
                self.i += 1;
                let exprs = if self.peek().is_some() { self.list(Self::expr)? } else { Vec::new() };
                self.eol()?;
                return Ok(Kind::Print { newline: op == "?", exprs });
            }
            Some(Tok::Op("=")) => {
                self.i += 1;
                let e = self.expr()?;
                self.eol()?;
                return Ok(Kind::Expr(e));
            }
            Some(Tok::Ident(w)) => w,
            _ => return self.fail("Sentencia no válida"),
        };
        let next = self.toks().get(1).cloned();
        if next == Some(Tok::Op("=")) {
            self.i = 2;
            let value = self.expr()?;
            self.eol()?;
            return Ok(Kind::Assign { targets: vec![Target { name: word, idx: Vec::new() }], value });
        }
        let k = |kw: &str| is_kw(&word, kw);
        self.i = 1;
        let kind = if k("IF") {
            let cond = self.expr()?;
            self.eat_kw("THEN");
            self.eol()?;
            let then = self.block(&["ELSE", "ENDIF"], Some(("IF", line)))?;
            let els = if self.starts("ELSE") {
                self.skip_line();
                self.block(&["ENDIF"], Some(("IF", line)))?
            } else {
                Vec::new()
            };
            self.skip_line();
            Kind::If { cond, then, els }
        } else if word == "DO" && self.at_kw("CASE") && self.toks().len() == 2 {
            self.skip_line();
            let opener = Some(("DO CASE", line));
            let (mut arms, mut otherwise) = (Vec::new(), Vec::new());
            loop {
                if self.starts("CASE") {
                    self.i = 1;
                    let cond = self.expr()?;
                    self.eol()?;
                    arms.push((cond, self.block(&["CASE", "OTHERWISE", "ENDCASE"], opener)?));
                } else if self.starts("OTHERWISE") {
                    self.skip_line();
                    otherwise = self.block(&["ENDCASE"], opener)?;
                } else if self.starts("ENDCASE") {
                    self.skip_line();
                    break;
                } else if self.eof() {
                    return Err(Error { line, msg: "DO CASE sin cerrar".into() });
                } else {
                    return self.fail("Se esperaba CASE, OTHERWISE o ENDCASE");
                }
            }
            Kind::Case { arms, otherwise }
        } else if word == "DO" && self.eat_kw("WHILE") {
            let cond = self.expr()?;
            self.eol()?;
            Kind::While { cond, body: self.loop_body(&["ENDDO"], ("DO WHILE", line))? }
        } else if word == "DO" {
            let name = self.ident()?;
            let args = if self.eat_kw("WITH") { self.list(Self::arg)? } else { Vec::new() };
            self.eol()?;
            Kind::Do { name, args }
        } else if word == "FOR" && self.eat_kw("EACH") {
            let var = self.ident()?;
            self.expect_kw("IN")?;
            let iter = self.expr()?;
            self.eol()?;
            Kind::ForEach { var, iter, body: self.loop_body(&["ENDFOR", "NEXT"], ("FOR EACH", line))? }
        } else if word == "FOR" {
            let var = self.ident()?;
            self.expect_op("=")?;
            let from = self.expr()?;
            self.expect_kw("TO")?;
            let to = self.expr()?;
            let step = if self.eat_kw("STEP") { Some(self.expr()?) } else { None };
            self.eol()?;
            Kind::For { var, from, to, step, body: self.loop_body(&["ENDFOR", "NEXT"], ("FOR", line))? }
        } else if k("LOOP") || k("EXIT") {
            if self.loops == 0 {
                return self.fail(format!("{word} fuera de un bucle"));
            }
            self.eol()?;
            if k("LOOP") { Kind::Loop } else { Kind::Exit }
        } else if k("RETURN") {
            let e = if self.peek().is_some() { Some(self.expr()?) } else { None };
            self.eol()?;
            Kind::Return(e)
        } else if k("PARAMETERS") || k("LPARAMETERS") {
            let names = self.list(|p| {
                let name = p.ident()?;
                p.skip_as()?;
                Ok(name)
            })?;
            self.eol()?;
            Kind::Params { local: k("LPARAMETERS"), names }
        } else if k("LOCAL") || k("PRIVATE") || k("PUBLIC") || k("DIMENSION") || k("DECLARE") {
            self.eat_kw("ARRAY");
            let items = self.list(|p| {
                let t = p.target()?;
                p.skip_as()?;
                Ok(t)
            })?;
            self.eol()?;
            let scope = if k("LOCAL") {
                Scope::Local
            } else if k("PRIVATE") {
                Scope::Private
            } else if k("PUBLIC") {
                Scope::Public
            } else {
                Scope::Dimension
            };
            Kind::Declare { scope, items }
        } else if k("STORE") {
            let value = self.expr()?;
            self.expect_kw("TO")?;
            let targets = self.list(Self::target)?;
            self.eol()?;
            Kind::Assign { targets, value }
        } else if k("RELEASE") {
            let names = self.list(Self::ident)?;
            self.eol()?;
            Kind::Release(if names == ["ALL"] { Vec::new() } else { names })
        } else if k("CLEAR") {
            let kind = if self.peek().is_none() {
                Kind::Clear
            } else if self.at_kw("ALL") || self.at_kw("MEMORY") {
                Kind::Release(Vec::new())
            } else {
                return self.fail("Esta variante de CLEAR aún no está soportada");
            };
            self.skip_line();
            kind
        } else if word == "SET" {
            let option = self.ident()?;
            if !SET_IGNORED.iter().any(|o| is_kw(&option, o)) {
                return self.fail(format!("SET {option} aún no está soportado"));
            }
            self.skip_line();
            Kind::Nop
        } else if ROUTINE.iter().any(|r| k(r)) {
            return self.fail(format!("{word} no puede ir dentro de un bloque"));
        } else if BLOCK_ENDS.iter().any(|e| k(e)) {
            return self.fail(format!("{word} sin su bloque de apertura"));
        } else if matches!(next, Some(Tok::Op("(" | "["))) {
            // `name(args)` alone is a call; followed by `=` it assigns an array element
            self.i = 0;
            let t = self.target()?;
            if self.eat_op("=") {
                let value = self.expr()?;
                self.eol()?;
                Kind::Assign { targets: vec![t], value }
            } else {
                self.eol()?;
                Kind::Expr(Expr::Call(t.name, t.idx))
            }
        } else {
            return self.fail(format!("Comando no reconocido: {word}"));
        };
        Ok(kind)
    }

    // Expressions, lowest precedence first: OR, AND, NOT, comparison, + -, * / %, ^, unary sign.

    fn expr(&mut self) -> R<Expr> {
        let mut l = self.and()?;
        while self.eat_kw("OR") {
            l = Expr::Bin("OR", Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }

    fn and(&mut self) -> R<Expr> {
        let mut l = self.not()?;
        while self.eat_kw("AND") {
            l = Expr::Bin("AND", Box::new(l), Box::new(self.not()?));
        }
        Ok(l)
    }

    fn not(&mut self) -> R<Expr> {
        if self.eat_kw("NOT") || self.eat_op("!") {
            return Ok(Expr::Un("NOT", Box::new(self.not()?)));
        }
        self.binary(&["==", "<>", "!=", "<=", ">=", "=", "<", ">", "#", "$"], |p| {
            p.binary(&["+", "-"], |p| p.binary(&["*", "/", "%"], |p| p.binary(&["^", "**"], Self::unary)))
        })
    }

    fn binary(&mut self, ops: &[&'static str], next: impl Fn(&mut Self) -> R<Expr>) -> R<Expr> {
        let mut l = next(self)?;
        while let Some(op) = ops.iter().copied().find(|op| self.at_op(op)) {
            self.i += 1;
            l = Expr::Bin(op, Box::new(l), Box::new(next(self)?));
        }
        Ok(l)
    }

    fn unary(&mut self) -> R<Expr> {
        for op in ["-", "+"] {
            if self.eat_op(op) {
                return Ok(Expr::Un(op, Box::new(self.unary()?)));
            }
        }
        let e = match self.peek().cloned() {
            Some(Tok::Num(n)) => Expr::Lit(Value::Num(n)),
            Some(Tok::Str(s)) => Expr::Lit(Value::Str(s)),
            Some(Tok::Bool(b)) => Expr::Lit(Value::Bool(b)),
            Some(Tok::Date(d)) => Expr::Lit(Value::Date(d)),
            Some(Tok::Null) => Expr::Lit(Value::Null),
            Some(Tok::Op("(")) => {
                self.i += 1;
                let e = self.expr()?;
                self.expect_op(")")?;
                return Ok(e);
            }
            Some(Tok::Ident(_)) => {
                let t = self.target()?;
                let called = matches!(self.toks().get(self.i - 1), Some(Tok::Op(")" | "]")));
                return Ok(if called { Expr::Call(t.name, t.idx) } else { Expr::Var(t.name) });
            }
            _ => return self.fail("Expresión no válida"),
        };
        self.i += 1;
        Ok(e)
    }

    fn arg(&mut self) -> R<Expr> {
        if self.eat_op("@") { Ok(Expr::Ref(self.ident()?)) } else { self.expr() }
    }

    fn args(&mut self, close: &str) -> R<Vec<Expr>> {
        if self.eat_op(close) {
            return Ok(Vec::new());
        }
        let args = self.list(Self::arg)?;
        self.expect_op(close)?;
        Ok(args)
    }
}
