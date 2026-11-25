use crate::lexer::{Line, Tok, lex};
use crate::value::Value;
use crate::{Error, R, is_kw};
use std::rc::Rc;

#[derive(Debug)]
pub enum Expr {
    Lit(Value),
    /// A name: a field of the open table if there is one called that, else a variable.
    Var(String),
    /// `m.name`: a variable, whatever the open table's fields are called.
    MemVar(String),
    /// `@name`: an argument passed by reference.
    Ref(String),
    /// `name(args)` or `name[args]`: a function call or an array element, decided at run time.
    Call(String, Vec<Expr>),
    /// `object.name`
    Member(Box<Expr>, String),
    /// `object.name(args)`: a method call, or an element of an array property.
    Method(Box<Expr>, String, Vec<Expr>),
    /// The object of the enclosing WITH block, as in `.Caption`.
    WithObj,
    Un(&'static str, Box<Expr>),
    Bin(&'static str, Box<Expr>, Box<Expr>),
}

#[derive(Debug)]
pub struct Stmt {
    pub line: usize,
    pub kind: Kind,
}

/// A name being declared, with its dimensions when it is an array.
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
    /// Targets are the expressions that can be assigned: variables, array elements, properties.
    Assign { targets: Vec<Expr>, value: Expr },
    Expr(Expr),
    Declare { scope: Scope, items: Vec<Target> },
    If { cond: Expr, then: Vec<Stmt>, els: Vec<Stmt> },
    Case { arms: Vec<(Expr, Vec<Stmt>)>, otherwise: Vec<Stmt> },
    For { var: String, from: Expr, to: Expr, step: Option<Expr>, body: Vec<Stmt> },
    ForEach { var: String, iter: Expr, body: Vec<Stmt> },
    While { cond: Expr, body: Vec<Stmt> },
    With { object: Expr, body: Vec<Stmt> },
    Loop,
    Exit,
    Do { name: String, args: Vec<Expr> },
    DoForm { name: String, args: Vec<Expr>, var: Option<String>, show: bool },
    /// MODIFY COMMAND, MODIFY FORM, CREATE FORM: ask the IDE to open a file.
    Open { form: bool, name: Option<String> },
    Return(Option<Expr>),
    Params { local: bool, names: Vec<String> },
    /// Empty means RELEASE ALL.
    Release(Vec<String>),
    Clear,
    Set(&'static str, Expr),
    /// HELP: ask the IDE to show the reference.
    Help,
    Data(DataCmd),
    Menu(MenuCmd),
    Scan { scope: RecScope, body: Vec<Stmt> },
    Nop,
}

/// A field of a table being created.
#[derive(Debug, Clone)]
pub struct FieldDef {
    pub name: String,
    /// C text, M memo, N number, I integer, L logical, D date.
    pub kind: char,
    pub width: usize,
    pub decimals: usize,
}

/// Which records a command works on: `ALL`, `NEXT n`, `REST`, `FOR cond`, `WHILE cond`.
#[derive(Debug, Default)]
pub struct RecScope {
    pub all: bool,
    pub rest: bool,
    pub next: Option<Expr>,
    pub cond: Option<Expr>,
    pub until: Option<Expr>,
}

/// The commands that work on tables. An area is given as an alias (text) or a number.
#[derive(Debug)]
pub enum DataCmd {
    Create { name: String, cursor: bool, fields: Vec<FieldDef> },
    /// `table` None closes the area.
    Use { table: Option<Expr>, alias: Option<String>, area: Option<Expr>, order: Option<String> },
    Select(Expr),
    CloseAll,
    AppendBlank,
    Insert { table: String, fields: Vec<String>, values: Vec<Expr> },
    /// Each target is an optional alias and a field.
    Replace { pairs: Vec<(Option<String>, String, Expr)>, scope: RecScope },
    Delete { recall: bool, scope: RecScope },
    Pack,
    Zap,
    GoTop,
    GoBottom,
    Go(Expr),
    Skip(Option<Expr>),
    Locate(Rc<RecScope>),
    Continue,
    Index { expr: Rc<Expr>, tag: String, descending: bool },
    SetOrder(Option<String>),
    Seek(Expr),
    Count { scope: RecScope, var: Option<String> },
    Sum { exprs: Vec<Expr>, vars: Vec<String>, scope: RecScope },
    List { exprs: Vec<Expr>, scope: RecScope, numbers: bool },
}

/// The item a menu command names: a pad of a menu, or a numbered bar of a popup.
#[derive(Debug)]
pub enum MenuItem {
    Pad(String),
    Bar(Expr),
}

#[derive(Debug)]
pub enum MenuCmd {
    /// DEFINE MENU or DEFINE POPUP: an empty one, replacing any of that name.
    Define { popup: bool, name: String },
    /// DEFINE PAD or DEFINE BAR. `skip` disables the item while it is true.
    Item { item: MenuItem, of: String, prompt: Expr, skip: Option<Rc<Expr>> },
    /// ON PAD or ON BAR: the popup the item opens. None detaches it.
    Opens { item: MenuItem, of: String, popup: Option<String> },
    /// ON SELECTION: what choosing runs. Without `item`, it is for every item of the
    /// menu or popup `of` that has no command of its own. Empty code removes it.
    Selection { item: Option<MenuItem>, popup: bool, of: String, code: Rc<Vec<Stmt>> },
    /// ACTIVATE MENU and DEACTIVATE MENU.
    Show { name: String, on: bool },
    /// RELEASE MENUS or POPUPS; no names means all of them.
    Release { popups: bool, names: Vec<String> },
    /// SET SYSMENU TO: empties the system menu.
    ResetSys,
}

#[derive(Debug)]
pub struct Routine {
    pub params: Vec<String>,
    pub body: Vec<Stmt>,
}

/// An object added to a class with ADD OBJECT.
pub struct Member {
    pub name: String,
    pub class: String,
    pub props: Vec<(String, Expr)>,
}

pub struct ClassDef {
    pub name: String,
    /// Another user class, or a built-in one such as FORM.
    pub parent: String,
    /// `name = expr` and `child.name = expr`, in source order.
    pub props: Vec<(String, Expr)>,
    pub dims: Vec<Target>,
    pub members: Vec<Member>,
    /// Keyed by `NAME`, or `CHILD.NAME` for the event of an added object.
    pub methods: Vec<(String, Rc<Routine>)>,
}

pub struct Program {
    pub main: Vec<Stmt>,
    pub routines: Vec<(String, Routine)>,
    pub classes: Vec<ClassDef>,
}

// SET options that only affect the classic FoxPro UI, safe to accept and ignore
const SET_IGNORED: &[&str] = &[
    "TALK", "ECHO", "SAFETY", "STATUS", "BELL", "CONSOLE", "ESCAPE", "NOTIFY", "HEADINGS", "CONFIRM", "SCOREBOARD",
    "CURSOR", "SYSMENU", "RESOURCE", "HELP", "DEVELOPMENT", "CLOCK", "MESSAGE",
];
const BLOCK_ENDS: &[&str] = &[
    "ENDIF", "ELSE", "ENDDO", "ENDFOR", "NEXT", "ENDCASE", "CASE", "OTHERWISE", "ENDPROC", "ENDFUNC", "ENDWITH",
    "ENDDEFINE", "ENDSCAN",
];
const ROUTINE: &[&str] = &["PROCEDURE", "FUNCTION"];

struct Parser {
    lines: Vec<Line>,
    ln: usize,
    i: usize,
    loops: usize,
}

pub fn parse(src: &str) -> R<Program> {
    let mut p = Parser { lines: lex(src)?, ln: 0, i: 0, loops: 0 };
    let main = p.block(&["PROCEDURE", "FUNCTION", "DEFINE"], None)?;
    let (mut routines, mut classes) = (Vec::new(), Vec::new());
    while !p.eof() {
        if p.starts("DEFINE") {
            classes.push(p.class()?);
        } else if ROUTINE.iter().any(|k| p.starts(k)) {
            routines.push(p.routine()?);
        } else {
            return p.fail("Después de un procedimiento o clase solo puede ir otro PROCEDURE, FUNCTION o DEFINE CLASS");
        }
    }
    Ok(Program { main, routines, classes })
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
        Err(Error::at(self.line(), msg))
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
            Some(Tok::Ident(w) | Tok::MemVar(w)) => {
                let w = w.clone();
                self.i += 1;
                Ok(w)
            }
            _ => self.fail("Se esperaba un nombre"),
        }
    }

    /// `name` or `child.name`, joined with dots.
    fn dotted(&mut self) -> R<String> {
        let mut path = self.ident()?;
        while self.eat_op(".") {
            path = format!("{path}.{}", self.ident()?);
        }
        Ok(path)
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
        self.i == 0
            && self.at_kw(kw)
            && self.toks().get(1) != Some(&Tok::Op("="))
            // only DEFINE CLASS opens a block: DEFINE MENU, PAD, POPUP and BAR are plain statements
            && (kw != "DEFINE" || matches!(self.toks().get(1), Some(Tok::Ident(w)) if is_kw(w, "CLASS")))
    }

    /// Statements up to, not including, a line starting with one of `ends`.
    /// `opener` names the block being closed; None means end of file also closes it.
    fn block(&mut self, ends: &[&str], opener: Option<(&str, usize)>) -> R<Vec<Stmt>> {
        let mut out = Vec::new();
        loop {
            if self.eof() {
                return match opener {
                    Some((name, line)) => Err(Error::at(line, format!("{name} sin cerrar"))),
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

    /// `PROCEDURE name[(params)]` or `FUNCTION ...`, with its body.
    fn routine(&mut self) -> R<(String, Routine)> {
        self.i = 1;
        let name = self.dotted()?;
        let mut params = Vec::new();
        if self.eat_op("(") && !self.eat_op(")") {
            loop {
                params.push(self.ident()?);
                self.skip_as()?;
                if self.eat_op(")") {
                    break;
                }
                self.expect_op(",")?;
            }
        }
        self.skip_as()?;
        self.eol()?;
        let body = self.block(&["PROCEDURE", "FUNCTION", "ENDPROC", "ENDFUNC", "DEFINE", "ENDDEFINE"], None)?;
        if self.starts("ENDPROC") || self.starts("ENDFUNC") {
            self.skip_line();
        }
        Ok((name, Routine { params, body }))
    }

    /// `DEFINE CLASS name AS parent` up to ENDDEFINE.
    fn class(&mut self) -> R<ClassDef> {
        let line = self.line();
        self.i = 1;
        self.expect_kw("CLASS")?;
        let name = self.ident()?;
        self.expect_kw("AS")?;
        let parent = self.ident()?;
        self.skip_line();
        let mut c = ClassDef { name, parent, props: Vec::new(), dims: Vec::new(), members: Vec::new(), methods: Vec::new() };
        let prop = |p: &mut Self| {
            let path = p.dotted()?;
            p.expect_op("=")?;
            Ok((path, p.expr()?))
        };
        loop {
            if self.eof() {
                return Err(Error::at(line, "DEFINE CLASS sin cerrar"));
            }
            if self.starts("ENDDEFINE") {
                self.skip_line();
                return Ok(c);
            }
            if ROUTINE.iter().any(|k| self.starts(k)) {
                let (path, routine) = self.routine()?;
                c.methods.push((path, Rc::new(routine)));
            } else if self.starts("ADD") {
                self.i = 1;
                self.expect_kw("OBJECT")?;
                let name = self.ident()?;
                self.expect_kw("AS")?;
                let class = self.ident()?;
                self.eat_kw("NOINIT");
                let props = if self.eat_kw("WITH") { self.list(prop)? } else { Vec::new() };
                self.eol()?;
                c.members.push(Member { name, class, props });
            } else if self.starts("DIMENSION") || self.starts("DECLARE") {
                self.i = 1;
                c.dims.extend(self.list(Self::target)?);
                self.eol()?;
            } else if self.starts("PROTECTED") || self.starts("HIDDEN") {
                // shortcut: every member is public, upgrade if programs rely on access errors
                self.skip_line();
            } else {
                c.props.push(prop(self)?);
                self.eol()?;
            }
        }
    }

    /// `name type[(width[, decimals])]` in CREATE TABLE.
    fn field_def(&mut self) -> R<FieldDef> {
        let name = self.ident()?;
        let type_name = self.ident()?;
        let (kind, mut width) = match type_name.as_str() {
            "C" | "CHAR" | "CHARACTER" | "V" | "VARCHAR" => ('C', 10),
            "N" | "NUMERIC" | "NUMBER" | "F" | "FLOAT" | "B" | "DOUBLE" | "Y" | "CURRENCY" => ('N', 10),
            "I" | "INT" | "INTEGER" => ('I', 4),
            "L" | "LOGICAL" => ('L', 1),
            "D" | "DATE" => ('D', 8),
            "M" | "MEMO" => ('M', 4),
            _ => return self.fail(format!("Tipo de campo no soportado: {type_name}")),
        };
        let mut decimals = 0;
        if self.eat_op("(") {
            let size = |p: &mut Self| match p.peek().cloned() {
                Some(Tok::Num(n)) if (0.0..=254.0).contains(&n) => {
                    p.i += 1;
                    Ok(n as usize)
                }
                _ => p.fail("Se esperaba el tamaño del campo"),
            };
            width = size(self)?;
            if self.eat_op(",") {
                decimals = size(self)?;
            }
            self.expect_op(")")?;
        }
        // shortcut: NULL / NOT NULL are accepted and ignored; other column rules are refused, not dropped
        self.eat_kw("NOT");
        self.eat_kw("NULL");
        if !(self.at_op(",") || self.at_op(")")) {
            return self.fail("Esta regla de columna aún no está soportada en CREATE TABLE");
        }
        Ok(FieldDef { name, kind, width: width.max(1), decimals })
    }

    /// A work area written as an alias or a number: `clientes`, `2`, `(expr)`.
    fn area(&mut self) -> R<Expr> {
        if matches!(self.peek(), Some(Tok::Ident(_))) { Ok(Expr::Lit(Value::Str(self.ident()?))) } else { self.unary() }
    }

    /// The `ALL`, `NEXT n`, `REST`, `FOR cond` and `WHILE cond` clauses, in any order.
    fn rec_scope(&mut self) -> R<RecScope> {
        let mut scope = RecScope::default();
        loop {
            if self.eat_kw("ALL") {
                scope.all = true;
            } else if self.eat_kw("REST") {
                scope.rest = true;
            } else if self.eat_kw("NEXT") {
                scope.next = Some(self.unary()?);
            } else if self.eat_kw("FOR") {
                scope.cond = Some(self.expr()?);
            } else if self.eat_kw("WHILE") {
                scope.until = Some(self.expr()?);
            } else {
                return Ok(scope);
            }
        }
    }

    /// The table commands that fit on one line. None when `word` is not one of them.
    fn data_cmd(&mut self, word: &str) -> R<Option<DataCmd>> {
        let k = |kw: &str| is_kw(word, kw);
        let clause = |p: &Self| ["IN", "ALIAS", "AGAIN", "ORDER", "EXCLUSIVE", "SHARED", "NOUPDATE"].iter().any(|c| p.at_kw(c));
        Ok(Some(if word == "USE" {
            let mut table = None;
            if matches!(self.peek(), Some(Tok::Ident(_))) && !clause(self) {
                table = Some(Expr::Lit(Value::Str(self.ident()?)));
                // an extension, if written, is ignored: tables have no files of their own
                if self.eat_op(".") {
                    self.ident()?;
                }
            } else if self.at_op("(") {
                table = Some(self.unary()?);
            }
            let (mut alias, mut area, mut order) = (None, None, None);
            loop {
                if self.eat_kw("IN") {
                    area = Some(self.area()?);
                } else if self.eat_kw("ALIAS") {
                    alias = Some(self.ident()?);
                } else if self.eat_kw("ORDER") {
                    self.eat_kw("TAG");
                    order = Some(self.ident()?);
                } else if !(self.eat_kw("AGAIN") || self.eat_kw("EXCLUSIVE") || self.eat_kw("SHARED") || self.eat_kw("NOUPDATE")) {
                    break;
                }
            }
            DataCmd::Use { table, alias, area, order }
        } else if k("SELECT") {
            // one word after SELECT picks a work area; anything longer is SQL
            if self.toks().len() != 2 && !self.at_op("(") {
                return self.fail("SELECT de SQL aún no está soportado");
            }
            DataCmd::Select(self.area()?)
        } else if k("CLOSE") {
            if !(self.eat_kw("DATABASES") || self.eat_kw("TABLES") || self.eat_kw("ALL")) {
                return self.fail("Se esperaba CLOSE DATABASES, CLOSE TABLES o CLOSE ALL");
            }
            self.eat_kw("ALL");
            DataCmd::CloseAll
        } else if k("APPEND") {
            self.expect_kw("BLANK")?;
            DataCmd::AppendBlank
        } else if k("INSERT") {
            self.expect_kw("INTO")?;
            let table = self.ident()?;
            let mut fields = Vec::new();
            if self.eat_op("(") {
                fields = self.list(Self::ident)?;
                self.expect_op(")")?;
            }
            self.expect_kw("VALUES")?;
            self.expect_op("(")?;
            let values = self.list(Self::expr)?;
            self.expect_op(")")?;
            DataCmd::Insert { table, fields, values }
        } else if k("REPLACE") {
            let mut scope = self.rec_scope()?;
            let pairs = self.list(|p| {
                let first = p.ident()?;
                let (alias, field) = if p.eat_op(".") { (Some(first), p.ident()?) } else { (None, first) };
                p.expect_kw("WITH")?;
                Ok((alias, field, p.expr()?))
            })?;
            let after = self.rec_scope()?;
            scope = RecScope {
                all: scope.all || after.all,
                rest: scope.rest || after.rest,
                next: scope.next.or(after.next),
                cond: scope.cond.or(after.cond),
                until: scope.until.or(after.until),
            };
            DataCmd::Replace { pairs, scope }
        } else if k("DELETE") || k("RECALL") {
            if self.at_kw("FROM") {
                return self.fail("DELETE de SQL aún no está soportado");
            }
            DataCmd::Delete { recall: k("RECALL"), scope: self.rec_scope()? }
        } else if word == "PACK" {
            DataCmd::Pack
        } else if word == "ZAP" {
            DataCmd::Zap
        } else if word == "GO" || k("GOTO") {
            if self.eat_kw("TOP") {
                DataCmd::GoTop
            } else if self.eat_kw("BOTTOM") {
                DataCmd::GoBottom
            } else {
                DataCmd::Go(self.expr()?)
            }
        } else if word == "SKIP" {
            DataCmd::Skip(if self.peek().is_some() { Some(self.expr()?) } else { None })
        } else if k("LOCATE") {
            DataCmd::Locate(Rc::new(self.rec_scope()?))
        } else if k("CONTINUE") {
            DataCmd::Continue
        } else if k("INDEX") {
            self.expect_kw("ON")?;
            let expr = Rc::new(self.expr()?);
            if !(self.eat_kw("TAG") || self.eat_kw("TO")) {
                return self.fail("Se esperaba TAG nombre");
            }
            let tag = self.ident()?;
            let mut descending = false;
            loop {
                if self.eat_kw("DESCENDING") {
                    descending = true;
                } else if self.eat_kw("COLLATE") {
                    // shortcut: text is always ordered by character code, FoxPro's "MACHINE" collation
                    self.unary()?;
                } else if !self.eat_kw("ASCENDING") {
                    break;
                }
            }
            DataCmd::Index { expr, tag, descending }
        } else if word == "SEEK" {
            DataCmd::Seek(self.expr()?)
        } else if k("COUNT") {
            let mut scope = self.rec_scope()?;
            let var = if self.eat_kw("TO") { Some(self.ident()?) } else { None };
            if scope.cond.is_none() {
                scope.cond = self.rec_scope()?.cond;
            }
            DataCmd::Count { scope, var }
        } else if word == "SUM" {
            let mut exprs = Vec::new();
            if !["TO", "FOR", "ALL", "REST", "NEXT", "WHILE"].iter().any(|c| self.at_kw(c)) {
                exprs = self.list(Self::expr)?;
            }
            let mut scope = self.rec_scope()?;
            self.expect_kw("TO")?;
            let vars = self.list(Self::ident)?;
            if scope.cond.is_none() {
                scope.cond = self.rec_scope()?.cond;
            }
            DataCmd::Sum { exprs, vars, scope }
        } else if word == "LIST" || k("DISPLAY") {
            // OFF leaves out the record numbers
            let mut numbers = !self.eat_kw("OFF");
            self.eat_kw("FIELDS");
            let mut exprs = Vec::new();
            if self.peek().is_some() && !["FOR", "ALL", "REST", "NEXT", "WHILE", "OFF"].iter().any(|c| self.at_kw(c)) {
                exprs = self.list(Self::expr)?;
            }
            let scope = self.rec_scope()?;
            numbers &= !self.eat_kw("OFF");
            DataCmd::List { exprs, scope, numbers }
        } else {
            return Ok(None);
        }))
    }

    /// `name OF menu` or `number OF popup`, after the word PAD or BAR.
    fn menu_item(&mut self, bar: bool) -> R<(MenuItem, String)> {
        let item = if bar { MenuItem::Bar(self.expr()?) } else { MenuItem::Pad(self.ident()?) };
        self.expect_kw("OF")?;
        Ok((item, self.ident()?))
    }

    /// The rest of the line as a command of its own, which is what ON SELECTION takes.
    fn rest_as_code(&mut self) -> R<Rc<Vec<Stmt>>> {
        let (no, toks) = (self.line(), self.toks()[self.i..].to_vec());
        self.skip_line();
        let lines = if toks.is_empty() { Vec::new() } else { vec![Line { no, toks }] };
        Ok(Rc::new(Parser { lines, ln: 0, i: 0, loops: 0 }.block(&[], None)?))
    }

    /// The menu commands that start with `word`, whose next word is already known to be theirs.
    fn menu_cmd(&mut self, word: &str) -> R<MenuCmd> {
        let k = |kw: &str| is_kw(word, kw);
        Ok(if k("DEFINE") && (self.at_kw("MENU") || self.at_kw("POPUP")) {
            let popup = self.at_kw("POPUP");
            self.i += 1;
            let name = self.ident()?;
            // BAR, MARGIN, RELATIVE, SHADOW, COLOR...: they only style FoxPro's own menus
            self.skip_line();
            MenuCmd::Define { popup, name }
        } else if k("DEFINE") {
            let bar = self.at_kw("BAR");
            self.i += 1;
            let (item, of) = self.menu_item(bar)?;
            self.expect_kw("PROMPT")?;
            let prompt = self.expr()?;
            let mut skip = None;
            while self.peek().is_some() {
                if self.eat_kw("SKIP") {
                    let always = Expr::Lit(Value::Bool(true));
                    skip = Some(Rc::new(if self.eat_kw("FOR") { self.expr()? } else { always }));
                } else if self.eat_kw("KEY") {
                    // shortcut: `KEY CTRL+S, "Ctrl+S"` is read and ignored; upgrade when menus must answer the keyboard
                    self.ident()?;
                    while self.eat_op("+") {
                        self.ident()?;
                    }
                    if self.eat_op(",") {
                        self.expr()?;
                    }
                } else if self.eat_kw("MESSAGE") || self.eat_kw("MARK") || (self.eat_kw("COLOR") && self.eat_kw("SCHEME")) {
                    self.expr()?;
                } else {
                    return self.fail("Cláusula no soportada en DEFINE PAD o DEFINE BAR");
                }
            }
            self.skip_line();
            MenuCmd::Item { item, of, prompt, skip }
        } else if word == "ON" && self.eat_kw("SELECTION") {
            let (item, popup, of) = if self.at_kw("PAD") || self.at_kw("BAR") {
                let bar = self.at_kw("BAR");
                self.i += 1;
                let (item, of) = self.menu_item(bar)?;
                (Some(item), bar, of)
            } else {
                let popup = self.eat_kw("POPUP");
                if !popup {
                    self.expect_kw("MENU")?;
                }
                (None, popup, self.ident()?)
            };
            MenuCmd::Selection { item, popup, of, code: self.rest_as_code()? }
        } else if word == "ON" {
            let bar = self.at_kw("BAR");
            self.i += 1;
            let (item, of) = self.menu_item(bar)?;
            let popup = if self.eat_kw("ACTIVATE") {
                self.expect_kw("POPUP")?;
                Some(self.ident()?)
            } else {
                None
            };
            self.eol()?;
            MenuCmd::Opens { item, of, popup }
        } else if k("RELEASE") || k("CLEAR") {
            let popups = self.at_kw("POPUPS");
            self.i += 1;
            let names = if self.peek().is_some() && k("RELEASE") { self.list(Self::ident)? } else { Vec::new() };
            self.eat_kw("EXTENDED");
            self.eol()?;
            MenuCmd::Release { popups, names }
        } else {
            self.i += 1;
            let name = self.ident()?;
            // NOWAIT, PAD name: a menu never makes the program wait here
            self.skip_line();
            MenuCmd::Show { name, on: k("ACTIVATE") }
        })
    }

    /// A name being declared, with optional dimensions: `x`, `a[3]`, `a(2, 2)`.
    fn target(&mut self) -> R<Target> {
        let name = self.ident()?;
        Ok(Target { name, idx: self.subscript()?.unwrap_or_default() })
    }

    /// A statement that starts with something assignable: an assignment to it, or a call.
    fn place_stmt(&mut self) -> R<Kind> {
        let place = self.postfix()?;
        if self.eat_op("=") {
            let value = self.expr()?;
            self.eol()?;
            return Ok(Kind::Assign { targets: vec![place], value });
        }
        self.eol()?;
        // `THISFORM.Release` without parentheses is still a call
        Ok(Kind::Expr(match place {
            Expr::Member(object, name) => Expr::Method(object, name, Vec::new()),
            e => e,
        }))
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
            Some(Tok::Op("WITH.") | Tok::MemVar(_)) => return self.place_stmt(),
            Some(Tok::Ident(w)) => w,
            _ => return self.fail("Sentencia no válida. Para ver el valor de una expresión, escribe ? delante"),
        };
        let next = self.toks().get(1).cloned();
        if matches!(next, Some(Tok::Op("=" | "."))) {
            return self.place_stmt();
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
                    return Err(Error::at(line, "DO CASE sin cerrar"));
                } else {
                    return self.fail("Se esperaba CASE, OTHERWISE o ENDCASE");
                }
            }
            Kind::Case { arms, otherwise }
        } else if word == "DO" && self.eat_kw("WHILE") {
            let cond = self.expr()?;
            self.eol()?;
            Kind::While { cond, body: self.loop_body(&["ENDDO"], ("DO WHILE", line))? }
        } else if word == "DO" && self.at_kw("FORM") && matches!(self.toks().get(2), Some(Tok::Ident(_))) {
            self.i = 2;
            let name = self.ident()?;
            let (mut args, mut var, mut show) = (Vec::new(), None, true);
            loop {
                if self.eat_kw("WITH") {
                    args = self.list(Self::arg)?;
                } else if self.eat_kw("NAME") {
                    var = Some(self.ident()?);
                } else if self.eat_kw("NOSHOW") {
                    show = false;
                } else if !self.eat_kw("LINKED") {
                    break;
                }
            }
            self.eol()?;
            Kind::DoForm { name, args, var, show }
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
        } else if word == "WITH" {
            let object = self.expr()?;
            self.eol()?;
            let body = self.block(&["ENDWITH"], Some(("WITH", line)))?;
            self.skip_line();
            Kind::With { object, body }
        } else if word == "SCAN" {
            let scope = self.rec_scope()?;
            self.eol()?;
            Kind::Scan { scope, body: self.loop_body(&["ENDSCAN"], ("SCAN", line))? }
        } else if (k("DEFINE") && ["MENU", "POPUP", "PAD", "BAR"].iter().any(|w| self.at_kw(w)))
            || (word == "ON" && ["SELECTION", "PAD", "BAR"].iter().any(|w| self.at_kw(w)))
            || ((k("RELEASE") || k("CLEAR")) && (self.at_kw("MENUS") || self.at_kw("POPUPS")))
            || ((k("ACTIVATE") || k("DEACTIVATE")) && self.at_kw("MENU"))
        {
            Kind::Menu(self.menu_cmd(&word)?)
        } else if let Some(cmd) = self.data_cmd(&word)? {
            self.eol()?;
            Kind::Data(cmd)
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
            let targets = self.list(Self::postfix)?;
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
        } else if k("CREATE") && (self.at_kw("TABLE") || self.at_kw("CURSOR")) {
            let cursor = self.eat_kw("CURSOR");
            self.i += !cursor as usize;
            let name = self.ident()?;
            self.eat_kw("FREE");
            self.expect_op("(")?;
            let fields = self.list(Self::field_def)?;
            self.expect_op(")")?;
            self.eol()?;
            Kind::Data(DataCmd::Create { name, cursor, fields })
        } else if k("MODIFY") || k("CREATE") {
            let form = self.eat_kw("FORM");
            if !form && !(k("MODIFY") && self.eat_kw("COMMAND")) {
                return self.fail(format!("Esta variante de {word} aún no está soportada"));
            }
            let name = if self.peek().is_some() { Some(self.ident()?) } else { None };
            // an extension, if written, is ignored: the kind of file decides it
            if self.eat_op(".") {
                self.ident()?;
            }
            self.eol()?;
            Kind::Open { form, name }
        } else if word == "HELP" {
            self.skip_line();
            Kind::Help
        } else if word == "SET" {
            let option = self.ident()?;
            if is_kw(&option, "EXACT") {
                let on = self.eat_kw("ON");
                if !on && !self.eat_kw("OFF") {
                    return self.fail("Se esperaba ON u OFF");
                }
                self.eol()?;
                return Ok(Kind::Set("EXACT", Expr::Lit(Value::Bool(on))));
            }
            if is_kw(&option, "DELETED") {
                let on = self.eat_kw("ON");
                if !on && !self.eat_kw("OFF") {
                    return self.fail("Se esperaba ON u OFF");
                }
                self.eol()?;
                return Ok(Kind::Set("DELETED", Expr::Lit(Value::Bool(on))));
            }
            if is_kw(&option, "ORDER") {
                self.expect_kw("TO")?;
                self.eat_kw("TAG");
                let tag = if self.peek().is_some() { Some(self.ident()?) } else { None };
                self.eol()?;
                return Ok(Kind::Data(DataCmd::SetOrder(tag)));
            }
            if is_kw(&option, "DECIMALS") {
                self.expect_kw("TO")?;
                let n = if self.peek().is_some() { self.expr()? } else { Expr::Lit(Value::Num(10.0)) };
                self.eol()?;
                return Ok(Kind::Set("DECIMALS", n));
            }
            if is_kw(&option, "SYSMENU") && self.eat_kw("TO") {
                self.skip_line();
                return Ok(Kind::Menu(MenuCmd::ResetSys));
            }
            if !SET_IGNORED.iter().any(|o| is_kw(&option, o)) {
                return self.fail(format!("SET {option} aún no está soportado"));
            }
            self.skip_line();
            Kind::Nop
        } else if ROUTINE.iter().any(|r| k(r)) || k("DEFINE") {
            return self.fail(format!("{word} no puede ir dentro de un bloque"));
        } else if BLOCK_ENDS.iter().any(|e| k(e)) {
            return self.fail(format!("{word} sin su bloque de apertura"));
        } else if matches!(next, Some(Tok::Op("(" | "["))) {
            // `name(args)` alone is a call; followed by `=` it assigns an array element
            self.i = 0;
            return self.place_stmt();
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
        self.postfix()
    }

    /// A primary followed by any number of `.member` or `.method(args)`.
    fn postfix(&mut self) -> R<Expr> {
        let mut e = if self.at_op("WITH.") { Expr::WithObj } else { self.primary()? };
        while self.eat_op(".") || (matches!(e, Expr::WithObj) && self.eat_op("WITH.")) {
            let name = self.ident()?;
            e = match self.subscript()? {
                Some(args) => Expr::Method(Box::new(e), name, args),
                None => Expr::Member(Box::new(e), name),
            };
        }
        Ok(e)
    }

    fn primary(&mut self) -> R<Expr> {
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
            Some(Tok::Ident(name)) => {
                self.i += 1;
                return Ok(match self.subscript()? {
                    Some(args) => Expr::Call(name, args),
                    None => Expr::Var(name),
                });
            }
            Some(Tok::MemVar(name)) => {
                self.i += 1;
                return Ok(match self.subscript()? {
                    Some(args) => Expr::Call(name, args),
                    None => Expr::MemVar(name),
                });
            }
            _ => return self.fail("Expresión no válida"),
        };
        self.i += 1;
        Ok(e)
    }

    /// `(args)` or `[args]`, if one follows.
    fn subscript(&mut self) -> R<Option<Vec<Expr>>> {
        Ok(if self.eat_op("(") {
            Some(self.args(")")?)
        } else if self.eat_op("[") {
            Some(self.args("]")?)
        } else {
            None
        })
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
