//! Tables: FoxPro's record-oriented commands, stored in SQLite.
//!
//! A folder's tables live in one SQLite file, `datos.fostdb`; cursors live in memory. Each table has two
//! hidden columns: `_recno`, the record number, and `_deleted`, FoxPro's deletion mark. On top of that
//! sit the things SQLite does not have: work areas, a record pointer, and orders by an expression.

use crate::interp::{Flow, Interp};
use crate::parser::{DataCmd, Expr, FieldDef, RecScope, Stmt, parse_expr};
use crate::object::{ObjRef, to_json};
use crate::value::{Value, compare, parse_date};
use crate::{R, err};
use chrono::NaiveDate;
use rusqlite::Connection;
use rusqlite::types::Value as Sql;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use serde_json::{Value as Json, json};
use std::rc::Rc;

const FILE: &str = "datos.fostdb";
/// Where each table's fields are described: SQLite's own column types are not enough to rebuild them.
const META: &str = "_fost_fields (tbl TEXT, pos INTEGER, name TEXT, kind TEXT, width INTEGER, decimals INTEGER)";

mod sql;

/// The orders of each table of the folder: the expression is kept as text and read again when the table is opened.
const TAGS: &str = "_fost_tags (tbl TEXT, name TEXT, expr TEXT, descending INTEGER)";

fn sql<T>(res: rusqlite::Result<T>) -> R<T> {
    res.map_err(|e| err(format!("Error de la base de datos: {e}")))
}

/// An order of a table: its records sorted by an expression.
struct Tag {
    name: String,
    expr: Rc<Expr>,
    descending: bool,
}

/// A table open in a work area.
struct Area {
    /// "main" for a cursor, "disk" for a table of the folder.
    schema: &'static str,
    table: String,
    alias: String,
    fields: Rc<Vec<FieldDef>>,
    order: Option<String>,
    /// The current record; None is past the last one (EOF).
    pos: Option<i64>,
    bof: bool,
    found: bool,
    /// Where `pos` was in `keys` when last looked up, for when the record itself drops out of them.
    hint: usize,
    /// The current record's fields, read once.
    row: Option<Rc<HashMap<String, Value>>>,
    /// Every visible record in the current order, with its key. Rebuilt after any change.
    // shortcut: rebuilt from scratch, fine for thousands of records; upgrade to incremental upkeep for more
    keys: Option<Rc<Vec<(Value, i64)>>>,
    /// The search of the last LOCATE, for CONTINUE.
    locate: Option<Rc<RecScope>>,
    /// SET FILTER TO: only the records that pass are in `keys`.
    filter: Option<Rc<Expr>>,
    /// With buffering on: each record changed since the last TABLEUPDATE(), as it was before
    /// (None for one that did not exist), so TABLEREVERT() can put it back.
    // shortcut: changes are written at once and only undone on request, so other areas see them early;
    // upgrade to holding them back when programs rely on nobody seeing an edit before TABLEUPDATE()
    buffer: Option<HashMap<i64, Option<Vec<Sql>>>>,
}

impl Area {
    fn sql_name(&self) -> String {
        format!("{}.\"{}\"", self.schema, self.table)
    }

    fn tag_key(&self) -> String {
        format!("{}.{}", self.schema, self.table)
    }
}

pub(crate) struct Data {
    conn: Connection,
    /// The folder database attached as `disk`, if any.
    disk: Option<PathBuf>,
    areas: BTreeMap<usize, Area>,
    current: usize,
    /// SET DELETED ON: records marked for deletion are skipped.
    hide_deleted: bool,
    /// Orders by table.
    tags: HashMap<String, Vec<Tag>>,
}

/// The tables saved in the folder `dir`, each with its fields, for a project panel.
/// Only reads: a folder with no database gets none, and none is created.
pub(crate) fn tables(dir: &std::path::Path) -> Vec<Json> {
    let read = || -> rusqlite::Result<Vec<(String, Json)>> {
        let conn = Connection::open_with_flags(dir.join(FILE), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let mut stmt = conn.prepare("SELECT tbl, name, kind, width, decimals FROM _fost_fields ORDER BY tbl, pos")?;
        let rows = stmt.query_map([], |r| {
            let field = json!({ "name": r.get::<_, String>(1)?, "kind": r.get::<_, String>(2)?, "width": r.get::<_, i64>(3)?, "decimals": r.get::<_, i64>(4)? });
            Ok((r.get(0)?, field))
        })?;
        rows.collect()
    };
    let mut out: Vec<Json> = Vec::new();
    for (table, field) in read().unwrap_or_default() {
        match out.last_mut().filter(|t| t["name"] == table.as_str()) {
            Some(last) => last["fields"].as_array_mut().unwrap().push(field),
            None => out.push(json!({ "name": table, "fields": [field] })),
        }
    }
    out
}

fn blank(f: &FieldDef) -> Value {
    match f.kind {
        'C' => Value::Str(" ".repeat(f.width)),
        'M' => Value::Str(String::new()),
        'L' => Value::Bool(false),
        'D' => Value::Date(None),
        _ => Value::Num(0.0),
    }
}

fn from_sql(f: &FieldDef, v: Sql) -> Value {
    match (f.kind, v) {
        // text fields are as wide as declared, padded with spaces
        ('C', Sql::Text(s)) => Value::Str(format!("{s:<width$}", width = f.width)),
        ('M', Sql::Text(s)) => Value::Str(s),
        ('L', Sql::Integer(n)) => Value::Bool(n != 0),
        ('D', Sql::Text(s)) => Value::Date(NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok()),
        ('D', Sql::Null) => Value::Date(None),
        (_, Sql::Integer(n)) => Value::Num(n as f64),
        (_, Sql::Real(n)) => Value::Num(n),
        _ => Value::Null,
    }
}

fn to_sql(f: &FieldDef, v: Value) -> R<Sql> {
    Ok(match (f.kind, v) {
        (_, Value::Null) => Sql::Null,
        ('C', Value::Str(s)) => Sql::Text(s.chars().take(f.width).collect::<String>().trim_end_matches(' ').into()),
        ('M', Value::Str(s)) => Sql::Text(s),
        ('L', Value::Bool(b)) => Sql::Integer(b as i64),
        ('D', Value::Date(d)) => d.map_or(Sql::Null, |d| Sql::Text(d.format("%Y-%m-%d").to_string())),
        ('I', Value::Num(n)) => Sql::Integer(n.round() as i64),
        ('N', Value::Num(n)) => Sql::Real(crate::value::round(n, f.decimals as i32)),
        _ => return Err(err(format!("El valor no es del tipo del campo {}", f.name))),
    })
}

impl Interp {
    fn db(&mut self) -> R<&mut Data> {
        if self.data.is_none() {
            let conn = sql(Connection::open_in_memory())?;
            sql(conn.execute_batch(&format!("CREATE TABLE main.{META}")))?;
            self.data = Some(Data { conn, disk: None, areas: BTreeMap::new(), current: 1, hide_deleted: false, tags: HashMap::new() });
        }
        Ok(self.data.as_mut().unwrap())
    }

    /// Makes the tables of `dir` reachable as the schema `disk`. False when the folder has no database
    /// and `create` does not ask for one: merely looking for a table must not leave a file behind.
    fn attach(&mut self, create: bool) -> R<bool> {
        let path = self.dir.join(FILE);
        let d = self.db()?;
        if d.disk.as_ref() == Some(&path) {
            return Ok(true);
        }
        // the folder changed: what was open from the old one goes with it
        if d.disk.take().is_some() {
            d.areas.retain(|_, a| a.schema != "disk");
            d.tags.retain(|key, _| !key.starts_with("disk."));
            sql(d.conn.execute_batch("DETACH DATABASE disk"))?;
        }
        if !create && !path.exists() {
            return Ok(false);
        }
        sql(d.conn.execute("ATTACH DATABASE ?1 AS disk", [path.to_string_lossy()]))?;
        // WAL: a loop of single-record changes does not wait on the disk after each one
        let setup = format!(
            "PRAGMA disk.journal_mode = WAL; PRAGMA disk.synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS disk.{META}; CREATE TABLE IF NOT EXISTS disk.{TAGS}"
        );
        sql(d.conn.execute_batch(&setup))?;
        d.disk = Some(path);
        Ok(true)
    }

    /// Where a table is and what its fields are: a cursor first, then the folder's tables.
    fn find_table(&mut self, name: &str) -> R<Option<(&'static str, Vec<FieldDef>)>> {
        for schema in ["main", "disk"] {
            if schema == "disk" && !self.attach(false)? {
                break;
            }
            let d = self.db()?;
            let query = format!("SELECT name, kind, width, decimals FROM {schema}._fost_fields WHERE tbl = ?1 ORDER BY pos");
            let mut stmt = sql(d.conn.prepare(&query))?;
            let field = |r: &rusqlite::Row| {
                let kind: String = r.get(1)?;
                Ok(FieldDef { name: r.get(0)?, kind: kind.chars().next().unwrap_or('C'), width: r.get::<_, u16>(2)? as usize, decimals: r.get::<_, u16>(3)? as usize })
            };
            let fields: Vec<FieldDef> = sql(sql(stmt.query_map([name], field))?.collect())?;
            if !fields.is_empty() {
                return Ok(Some((schema, fields)));
            }
        }
        Ok(None)
    }

    fn current_area(&mut self) -> R<usize> {
        match &self.data {
            Some(d) if d.areas.contains_key(&d.current) => Ok(d.current),
            _ => Err(err("No hay ninguna tabla abierta en esta área de trabajo")),
        }
    }

    /// The area `n`, which the caller knows is open.
    fn area(&mut self, n: usize) -> &mut Area {
        self.data.as_mut().unwrap().areas.get_mut(&n).unwrap()
    }

    /// The number of a work area given as a number or an alias.
    fn area_number(&mut self, v: &Value) -> R<usize> {
        let d = self.db()?;
        match v {
            Value::Num(n) if *n >= 1.0 => Ok(*n as usize),
            // 0 asks for the lowest area with nothing open
            Value::Num(_) => Ok((1..).find(|n| !d.areas.contains_key(n)).unwrap()),
            Value::Str(alias) => {
                let alias = alias.trim().to_uppercase();
                let found = d.areas.iter().find(|(_, a)| a.alias == alias).map(|(n, _)| *n);
                found.ok_or_else(|| err(format!("Alias no encontrado: {alias}")))
            }
            _ => Err(err("Un área de trabajo se indica con un número o un alias")),
        }
    }

    fn close_area(&mut self, n: usize) -> R<()> {
        let d = self.db()?;
        let Some(closed) = d.areas.remove(&n) else { return Ok(()) };
        // a cursor exists only while it is open somewhere
        if closed.schema == "main" && !d.areas.values().any(|a| a.schema == "main" && a.table == closed.table) {
            sql(d.conn.execute_batch(&format!("DROP TABLE IF EXISTS {}", closed.sql_name())))?;
            sql(d.conn.execute("DELETE FROM main._fost_fields WHERE tbl = ?1", [&closed.table]))?;
            d.tags.remove(&closed.tag_key());
        }
        Ok(())
    }

    fn open(&mut self, table: &str, alias: Option<&str>, n: usize, order: Option<&str>) -> R<()> {
        let Some((schema, fields)) = self.find_table(table)? else {
            return Err(err(format!("Tabla no encontrada: {table}")));
        };
        let alias = alias.unwrap_or(table).to_string();
        if self.db()?.areas.iter().any(|(other, a)| *other != n && a.alias == alias) {
            return Err(err(format!("El alias {alias} ya está en uso")));
        }
        let mut area = Area {
            schema,
            table: table.to_string(),
            alias,
            fields: Rc::new(fields),
            order: None,
            pos: None,
            bof: false,
            found: false,
            hint: 0,
            row: None,
            keys: None,
            locate: None,
            filter: None,
            buffer: None,
        };
        if schema == "disk" {
            self.load_tags(table)?;
        }
        if let Some(tag) = order {
            if !self.db()?.tags.get(&area.tag_key()).is_some_and(|tags| tags.iter().any(|t| t.name == tag)) {
                return Err(err(format!("Orden no encontrado: {tag}")));
            }
            area.order = Some(tag.to_string());
        }
        // a cursor being reopened in its own area must survive the close of that area
        let d = self.db()?;
        match d.areas.insert(n, area) {
            Some(old) if old.schema == "main" && old.table != table => {
                d.areas.insert(usize::MAX, old);
                self.close_area(usize::MAX)?;
            }
            _ => {}
        }
        self.go(n, 0)
    }

    /// Forgets what was read from a table, in every area that has it open, after it changed.
    fn touch(&mut self, n: usize) {
        let d = self.data.as_mut().unwrap();
        let (schema, table) = (d.areas[&n].schema, d.areas[&n].table.clone());
        for a in d.areas.values_mut().filter(|a| a.schema == schema && a.table == table) {
            a.keys = None;
            a.row = None;
        }
    }

    fn place(&mut self, n: usize, pos: Option<i64>) {
        let a = self.area(n);
        a.pos = pos;
        a.row = None;
        a.bof = false;
    }

    /// The visible records of an area in its current order, each with its key in that order.
    fn keys(&mut self, n: usize) -> R<Rc<Vec<(Value, i64)>>> {
        if let Some(keys) = &self.area(n).keys {
            return Ok(keys.clone());
        }
        let d = self.data.as_ref().unwrap();
        let a = &d.areas[&n];
        let visible = if d.hide_deleted { " WHERE _deleted = 0" } else { "" };
        let query = format!("SELECT _recno FROM {}{visible} ORDER BY _recno", a.sql_name());
        let tag = a.order.as_ref().and_then(|o| d.tags.get(&a.tag_key())?.iter().find(|t| t.name == *o));
        let tag = tag.map(|t| (t.expr.clone(), t.descending));
        let filter = a.filter.clone();
        let ids: Vec<i64> = {
            let mut stmt = sql(d.conn.prepare(&query))?;
            sql(sql(stmt.query_map([], |r| r.get(0)))?.collect())?
        };
        let mut keys: Vec<(Value, i64)> = Vec::with_capacity(ids.len());
        if tag.is_none() && filter.is_none() {
            keys.extend(ids.into_iter().map(|id| (Value::Null, id)));
        } else {
            // the filter and the key expression are evaluated with each record current, in this area
            let d = self.data.as_mut().unwrap();
            let saved = (d.current, d.areas[&n].pos, d.areas[&n].bof);
            d.current = n;
            let mut filled = Ok(());
            for id in ids {
                self.place(n, Some(id));
                let key = match filter.as_ref().map(|f| self.truthy(f)).transpose() {
                    Ok(Some(false)) => continue,
                    Ok(_) => tag.as_ref().map_or(Ok(Value::Null), |(expr, _)| self.eval(expr)),
                    Err(e) => Err(e),
                };
                match key {
                    Ok(key) => keys.push((key, id)),
                    Err(e) => {
                        filled = Err(e);
                        break;
                    }
                }
            }
            self.place(n, saved.1);
            self.area(n).bof = saved.2;
            self.data.as_mut().unwrap().current = saved.0;
            filled?;
            if let Some((_, descending)) = tag {
                // stable: records with equal keys stay in record-number order
                keys.sort_by(|a, b| compare(&a.0, &b.0).unwrap_or(Ordering::Equal));
                if descending {
                    keys.reverse();
                }
            }
        }
        let keys = Rc::new(keys);
        self.area(n).keys = Some(keys.clone());
        Ok(keys)
    }

    /// Reads the orders kept with a table of the folder, the first time it is opened.
    fn load_tags(&mut self, table: &str) -> R<()> {
        let key = format!("disk.{table}");
        let d = self.db()?;
        if d.tags.contains_key(&key) {
            return Ok(());
        }
        let saved: Vec<(String, String, bool)> = {
            let mut stmt = sql(d.conn.prepare("SELECT name, expr, descending FROM disk._fost_tags WHERE tbl = ?1"))?;
            sql(sql(stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))))?.collect())?
        };
        // an expression that no longer parses is an order that is gone
        let tag = |(name, text, descending): (String, String, bool)| Some(Tag { name, expr: Rc::new(parse_expr(&text).ok()?), descending });
        d.tags.insert(key, saved.into_iter().filter_map(tag).collect());
        Ok(())
    }

    /// With buffering on, keeps the record as it is now, before its first change.
    fn remember(&mut self, n: usize, id: i64) -> R<()> {
        let a = self.area(n);
        if !a.buffer.as_ref().is_some_and(|log| !log.contains_key(&id)) {
            return Ok(());
        }
        let query = format!("SELECT * FROM {} WHERE _recno = ?1", a.sql_name());
        let conn = &self.data.as_ref().unwrap().conn;
        let row: Vec<Sql> = sql(conn.query_row(&query, [id], |r| (0..r.as_ref().column_count()).map(|i| r.get(i)).collect()))?;
        self.area(n).buffer.as_mut().unwrap().insert(id, Some(row));
        Ok(())
    }

    /// TABLEREVERT(): undoes what was changed since buffering started or the last TABLEUPDATE(). Returns how many records.
    fn revert(&mut self, n: usize) -> R<usize> {
        let Some(log) = self.area(n).buffer.as_mut().map(std::mem::take) else { return Ok(0) };
        let name = self.area(n).sql_name();
        let conn = &self.data.as_ref().unwrap().conn;
        for (id, before) in &log {
            match before {
                None => sql(conn.execute(&format!("DELETE FROM {name} WHERE _recno = ?1"), [id]))?,
                Some(row) => {
                    let marks = vec!["?"; row.len()].join(", ");
                    sql(conn.execute(&format!("INSERT OR REPLACE INTO {name} VALUES ({marks})"), rusqlite::params_from_iter(row)))?
                }
            };
        }
        self.touch(n);
        // the current record may be one that no longer exists
        if self.area(n).pos.is_some_and(|id| log.get(&id) == Some(&None)) {
            self.go(n, 0)?;
        }
        Ok(log.len())
    }

    /// Sets or clears the deletion mark of the current record of an area.
    fn mark(&mut self, n: usize, deleted: bool) -> R<()> {
        let Some(id) = self.area(n).pos else { return Ok(()) };
        self.remember(n, id)?;
        let query = format!("UPDATE {} SET _deleted = ?1 WHERE _recno = ?2", self.area(n).sql_name());
        sql(self.data.as_ref().unwrap().conn.execute(&query, rusqlite::params![deleted as i64, id]))?;
        self.touch(n);
        Ok(())
    }

    /// TABLEUPDATE(), TABLEREVERT() and CURSORSETPROP("Buffering", n).
    fn buffer_function(&mut self, name: &str, args: &[Expr]) -> R<Value> {
        let vals = args.iter().map(|a| self.eval(a)).collect::<R<Vec<Value>>>()?;
        // the area, a number or an alias, is the last argument each of them takes
        let n = match vals.get(if name == "TABLEREVERT" { 1 } else { 2 }) {
            Some(v) => self.area_number(v)?,
            None => self.current_area()?,
        };
        if !self.db()?.areas.contains_key(&n) {
            return Err(err("No hay ninguna tabla abierta en esa área de trabajo"));
        }
        Ok(match name {
            "TABLEREVERT" => Value::Num(self.revert(n)? as f64),
            "TABLEUPDATE" => {
                if let Some(log) = self.area(n).buffer.as_mut() {
                    log.clear();
                }
                Value::Bool(true)
            }
            _ => {
                // 1 is no buffering; FoxPro's row and table modes (2 to 5) all buffer the whole table here
                if let [Value::Str(prop), Value::Num(mode), ..] = vals.as_slice() {
                    if prop.eq_ignore_ascii_case("BUFFERING") {
                        let a = self.area(n);
                        a.buffer = if *mode > 1.0 { Some(a.buffer.take().unwrap_or_default()) } else { None };
                    }
                }
                Value::Bool(true)
            }
        })
    }

    /// One field of every record an area shows, in its order, as text: what a list filled from a table holds.
    /// `source` names the field as a ControlSource does. Empty when it names none.
    pub(crate) fn column(&mut self, source: &str) -> R<Vec<String>> {
        let Some((n, field)) = self.source_field(source) else { return Ok(Vec::new()) };
        let keys = self.keys(n)?;
        let d = self.data.as_ref().unwrap();
        let query = format!("SELECT _recno, \"{}\" FROM {}", field.name, d.areas[&n].sql_name());
        let mut stmt = sql(d.conn.prepare(&query))?;
        let values: HashMap<i64, Value> = sql(sql(stmt.query_map([], |r| Ok((r.get(0)?, from_sql(&field, r.get(1)?)))))?.collect())?;
        Ok(keys.iter().filter_map(|k| values.get(&k.1)).map(|v| v.display().trim_end().to_string()).collect())
    }

    /// The first field of the table open as `alias`, named as a ControlSource names it.
    pub(crate) fn first_field(&self, alias: &str) -> Option<String> {
        let n = self.alias_area(alias)?;
        let a = &self.data.as_ref()?.areas[&n];
        Some(format!("{}.{}", a.alias, a.fields.first()?.name))
    }

    /// Moves the record pointer `step` records from the top (GO TOP is `go(n, 0)`).
    fn go(&mut self, n: usize, step: usize) -> R<()> {
        let keys = self.keys(n)?;
        self.place(n, keys.get(step).map(|k| k.1));
        self.area(n).hint = step.min(keys.len());
        Ok(())
    }

    /// Where the current record is in the order, or where it would be if it were still there.
    fn index(&mut self, n: usize) -> R<usize> {
        let keys = self.keys(n)?;
        let a = self.area(n);
        let found = match a.pos {
            None => Some(keys.len()),
            Some(id) => keys.iter().position(|k| k.1 == id),
        };
        if let Some(i) = found {
            a.hint = i;
        }
        Ok(a.hint.min(keys.len()))
    }

    fn skip(&mut self, n: usize, by: i64) -> R<()> {
        let keys = self.keys(n)?;
        let here = self.index(n)? as i64;
        // a current record that dropped out of the order no longer takes a place in it
        let gone = self.area(n).pos.is_some_and(|id| !keys.iter().any(|k| k.1 == id));
        let target = here + by - (gone && by > 0) as i64;
        if target < 0 {
            // FoxPro stays on the first record and raises BOF
            self.place(n, keys.first().map(|k| k.1));
            self.area(n).bof = true;
            self.area(n).hint = 0;
        } else {
            self.go(n, target as usize)?;
        }
        Ok(())
    }

    /// The current record's fields by name, plus `_DELETED`. Blanks past the last record.
    fn row(&mut self, n: usize) -> R<Rc<HashMap<String, Value>>> {
        if let Some(row) = &self.area(n).row {
            return Ok(row.clone());
        }
        let d = self.data.as_ref().unwrap();
        let a = &d.areas[&n];
        let mut row: HashMap<String, Value> = a.fields.iter().map(|f| (f.name.clone(), blank(f))).collect();
        row.insert("_DELETED".into(), Value::Bool(false));
        if let Some(id) = a.pos {
            let columns: Vec<String> = a.fields.iter().map(|f| format!("\"{}\"", f.name)).collect();
            let query = format!("SELECT _deleted, {} FROM {} WHERE _recno = ?1", columns.join(", "), a.sql_name());
            let mut stmt = sql(d.conn.prepare(&query))?;
            let mut rows = sql(stmt.query([id]))?;
            if let Some(r) = sql(rows.next())? {
                row.insert("_DELETED".into(), Value::Bool(sql(r.get::<_, i64>(0))? != 0));
                for (i, f) in a.fields.iter().enumerate() {
                    row.insert(f.name.clone(), from_sql(f, sql(r.get(i + 1))?));
                }
            }
        }
        let row = Rc::new(row);
        self.area(n).row = Some(row.clone());
        Ok(row)
    }

    /// The value of a field of the current area's table, if it has a field called `name`.
    pub(crate) fn field(&mut self, name: &str) -> R<Option<Value>> {
        let Some(d) = &self.data else { return Ok(None) };
        let n = d.current;
        match d.areas.get(&n) {
            Some(a) if a.fields.iter().any(|f| f.name == name) => Ok(self.row(n)?.get(name).cloned()),
            _ => Ok(None),
        }
    }

    /// `alias.name`, if `alias` is an open table with that field.
    pub(crate) fn alias_field(&mut self, alias: &str, name: &str) -> R<Option<Value>> {
        let Some(d) = &self.data else { return Ok(None) };
        let found = d.areas.iter().find(|(_, a)| a.alias == alias && a.fields.iter().any(|f| f.name == name));
        match found.map(|(n, _)| *n) {
            Some(n) => Ok(self.row(n)?.get(name).cloned()),
            None => Ok(None),
        }
    }

    /// The area of the table open as `alias`; the current area when it is empty.
    fn alias_area(&self, alias: &str) -> Option<usize> {
        let d = self.data.as_ref()?;
        let alias = alias.trim().to_uppercase();
        if alias.is_empty() {
            return d.areas.contains_key(&d.current).then_some(d.current);
        }
        d.areas.iter().find(|(_, a)| a.alias == alias).map(|(n, _)| *n)
    }

    /// The area and field a ControlSource names, `field` or `alias.field`, when it is a field of an open table.
    fn source_field(&self, source: &str) -> Option<(usize, FieldDef)> {
        let source = source.trim().to_uppercase();
        let (alias, field) = source.split_once('.').unwrap_or(("", &source));
        let n = self.alias_area(alias)?;
        let field = self.data.as_ref()?.areas[&n].fields.iter().find(|f| f.name == field)?;
        Some((n, field.clone()))
    }

    /// What a ControlSource holds now: a field of the current record, or a variable. None when it names neither.
    pub(crate) fn read_source(&mut self, source: &str) -> R<Option<Value>> {
        if let Some((n, field)) = self.source_field(source) {
            return Ok(self.row(n)?.get(&field.name).cloned());
        }
        let name = source.trim().to_uppercase();
        Ok(self.lookup(name.strip_prefix("M.").unwrap_or(&name)).map(|v| v.borrow().clone()))
    }

    /// Stores what a bound control holds. Text typed for a number or a date is converted;
    /// while it is not one yet (a date half typed) nothing is stored.
    pub(crate) fn write_source(&mut self, source: &str, v: Value) -> R<()> {
        let Some((n, field)) = self.source_field(source) else {
            let name = source.trim().to_uppercase();
            let name = name.strip_prefix("M.").unwrap_or(&name);
            if self.lookup(name).is_some() {
                self.set_var(name, v);
            }
            return Ok(());
        };
        let v = match (field.kind, v) {
            ('N' | 'I', Value::Str(s)) => match s.trim().parse() {
                Ok(n) => Value::Num(n),
                Err(_) => return Ok(()),
            },
            ('D', Value::Str(s)) if s.trim_matches([' ', '/']).is_empty() => Value::Date(None),
            ('D', Value::Str(s)) => match parse_date(&s) {
                Some(d) => Value::Date(Some(d)),
                None => return Ok(()),
            },
            // a CheckBox holds 0 or 1 until it learns its field is logical
            ('L', Value::Num(n)) => Value::Bool(n != 0.0),
            (_, v) => v,
        };
        self.replace(n, &field.name, v)
    }

    /// The columns of a Grid: the field each shows, its title and its width (None: as wide as it needs).
    /// With no Column objects, every field of the table.
    fn grid_columns(&mut self, grid: &ObjRef, fields: &[FieldDef]) -> Vec<(FieldDef, String, Option<Value>)> {
        let columns: Vec<ObjRef> = grid.0.borrow().kids().into_iter().filter(|k| k.0.borrow().base == "COLUMN").collect();
        if columns.is_empty() {
            return fields.iter().map(|f| (f.clone(), f.name.clone(), None)).collect();
        }
        let mut out = Vec::new();
        for (i, column) in columns.iter().enumerate() {
            let c = column.0.borrow();
            let text = |v: Option<&Value>| v.map_or(String::new(), |v| v.display());
            // `alias.field` or `field`; empty takes the field in the column's own position
            let source = text(c.props.get("CONTROLSOURCE")).trim().to_uppercase();
            let name = source.rsplit('.').next().unwrap_or_default();
            let field = if name.is_empty() { fields.get(i) } else { fields.iter().find(|f| f.name == name) };
            let Some(field) = field else { continue };
            let title = match c.props.get("HEADER1") {
                Some(Value::Object(h)) => Some(text(h.0.borrow().props.get("CAPTION"))).filter(|t| t != "Header1"),
                _ => None,
            };
            out.push((field.clone(), title.unwrap_or_else(|| field.name.clone()), c.props.get("WIDTH").cloned()));
        }
        out
    }

    /// What a Grid shows of its RecordSource: its columns, its records in the current order, and which is current.
    // shortcut: the first 1000 records are sent whole on every change; upgrade to sending the visible window when tables grow
    pub(crate) fn grid_json(&mut self, grid: &ObjRef) -> Json {
        let empty = json!({ "fields": [], "titles": [], "widths": [], "rows": [], "recno": null });
        let alias = grid.0.borrow().props.get("RECORDSOURCE").map_or(String::new(), |v| v.display());
        let Some(n) = self.alias_area(&alias) else { return empty };
        let Ok(keys) = self.keys(n) else { return empty };
        let fields = self.area(n).fields.clone();
        let columns = self.grid_columns(grid, &fields);
        let d = self.data.as_ref().unwrap();
        let a = &d.areas[&n];
        let names: Vec<String> = columns.iter().map(|(f, ..)| format!(", \"{}\"", f.name)).collect();
        let query = format!("SELECT _recno{} FROM {}", names.concat(), a.sql_name());
        let read = || -> rusqlite::Result<HashMap<i64, Vec<Json>>> {
            let mut stmt = d.conn.prepare(&query)?;
            let rows = stmt.query_map([], |r| {
                let cells = columns.iter().enumerate().map(|(i, (f, ..))| {
                    Ok(match from_sql(f, r.get(i + 1)?) {
                        Value::Str(s) => json!(s.trim_end()),
                        v => to_json(&v).unwrap_or(Json::Null),
                    })
                });
                Ok((r.get(0)?, cells.collect::<rusqlite::Result<_>>()?))
            })?;
            rows.collect()
        };
        let Ok(mut cells) = read() else { return empty };
        let rows: Vec<Json> = keys.iter().take(1000).map(|k| json!({ "recno": k.1, "cells": cells.remove(&k.1) })).collect();
        let fields: Vec<&str> = columns.iter().map(|(f, ..)| f.name.as_str()).collect();
        let titles: Vec<&str> = columns.iter().map(|(_, title, _)| title.as_str()).collect();
        let widths: Vec<Json> = columns.iter().map(|(.., w)| w.as_ref().and_then(to_json).unwrap_or(Json::Null)).collect();
        json!({ "fields": fields, "titles": titles, "widths": widths, "rows": rows, "recno": a.pos })
    }

    /// The user clicked a row of a Grid (`RecNo`), and perhaps changed a cell of it (`Field` and `Value`).
    pub(crate) fn grid_event(&mut self, grid: &ObjRef, props: &Json) -> R<()> {
        let prop = |name: &str| grid.0.borrow().props.get(name).cloned();
        let alias = prop("RECORDSOURCE").map_or(String::new(), |v| v.display());
        let (Some(n), Some(recno)) = (self.alias_area(&alias), props["RecNo"].as_i64()) else { return Ok(()) };
        // the record may have been deleted or filtered out since the grid was drawn
        if !self.keys(n)?.iter().any(|k| k.1 == recno) {
            return Ok(());
        }
        self.place(n, Some(recno));
        let edited = props["Field"].as_str().zip(props.get("Value").and_then(crate::object::from_json));
        if let Some((field, value)) = edited.filter(|_| prop("READONLY") != Some(Value::Bool(true))) {
            let source = format!("{}.{field}", self.area(n).alias);
            self.write_source(&source, value)?;
        }
        Ok(())
    }

    pub(crate) fn set_deleted(&mut self, hide: bool) -> R<()> {
        let d = self.db()?;
        d.hide_deleted = hide;
        for a in d.areas.values_mut() {
            a.keys = None;
        }
        Ok(())
    }

    /// Adds a record with the given values, blank where there is none, and makes it current.
    fn append(&mut self, n: usize, values: HashMap<String, Value>) -> R<()> {
        let a = self.area(n);
        let fields = a.fields.clone();
        let query = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            a.sql_name(),
            fields.iter().map(|f| format!("\"{}\"", f.name)).collect::<Vec<_>>().join(", "),
            vec!["?"; fields.len()].join(", ")
        );
        let mut params = Vec::new();
        for f in fields.iter() {
            params.push(to_sql(f, values.get(&f.name).cloned().unwrap_or_else(|| blank(f)))?);
        }
        let conn = &self.data.as_ref().unwrap().conn;
        sql(conn.execute(&query, rusqlite::params_from_iter(params)))?;
        let id = conn.last_insert_rowid();
        if let Some(log) = self.area(n).buffer.as_mut() {
            log.entry(id).or_insert(None);
        }
        self.touch(n);
        self.place(n, Some(id));
        Ok(())
    }

    /// Writes one field of the current record of an area. Past the last record there is nothing to write.
    fn replace(&mut self, n: usize, field: &str, v: Value) -> R<()> {
        let a = self.area(n);
        let Some(f) = a.fields.iter().find(|f| f.name == field) else {
            return Err(err(format!("Campo no encontrado: {field}")));
        };
        let Some(id) = a.pos else { return Ok(()) };
        let query = format!("UPDATE {} SET \"{}\" = ?1 WHERE _recno = ?2", a.sql_name(), f.name);
        let value = to_sql(f, v)?;
        self.remember(n, id)?;
        sql(self.data.as_ref().unwrap().conn.execute(&query, rusqlite::params![value, id]))?;
        self.touch(n);
        Ok(())
    }

    /// Visits the records a scope covers, in the current area, with each one current in turn.
    /// Without ALL, NEXT, REST or WHILE the scope is every record if `default_all` or there is a FOR, else just the current one.
    fn each(&mut self, scope: &RecScope, default_all: bool, mut visit: impl FnMut(&mut Self, usize) -> R<()>) -> R<()> {
        let n = self.current_area()?;
        let bounded = scope.rest || scope.next.is_some() || scope.until.is_some();
        let all = scope.all || (!bounded && (default_all || scope.cond.is_some()));
        let limit = match &scope.next {
            Some(count) => Some(self.num(count)?.max(0.0) as usize),
            None if !all && !bounded => Some(1),
            None => None,
        };
        // the records are listed first: the visit may change what the order contains
        let keys = self.keys(n)?;
        let from = if all { 0 } else { self.index(n)? };
        let ids: Vec<i64> = keys.iter().skip(from).take(limit.unwrap_or(usize::MAX)).map(|k| k.1).collect();
        for id in ids {
            self.place(n, Some(id));
            if let Some(until) = &scope.until {
                // WHILE stops at the first record that fails, and stays on it
                if !self.truthy(until)? {
                    return Ok(());
                }
            }
            if let Some(cond) = &scope.cond {
                if !self.truthy(cond)? {
                    continue;
                }
            }
            visit(self, n)?;
        }
        // a scope that runs to the end leaves the pointer past the last record
        if limit.is_none() {
            self.place(n, None);
        }
        Ok(())
    }

    /// Finds the first record from `from` on that passes the scope's FOR, and stops there.
    fn locate(&mut self, n: usize, scope: &RecScope, from: usize) -> R<()> {
        let keys = self.keys(n)?;
        let limit = match &scope.next {
            Some(count) => self.num(count)?.max(0.0) as usize,
            None => usize::MAX,
        };
        for (_, id) in keys.iter().skip(from).take(limit) {
            self.place(n, Some(*id));
            if scope.until.as_ref().map(|w| self.truthy(w)).transpose()? == Some(false) {
                break;
            }
            if scope.cond.as_ref().map(|c| self.truthy(c)).transpose()? != Some(false) {
                self.area(n).found = true;
                return Ok(());
            }
        }
        self.place(n, None);
        self.area(n).found = false;
        Ok(())
    }

    /// SEEK: the first record whose key in the current order matches. Text matches by its beginning
    /// unless SET EXACT is ON, so SEEK "GAR" finds "GARCIA".
    fn seek(&mut self, n: usize, wanted: &Value) -> R<bool> {
        if self.area(n).order.is_none() {
            return Err(err("SEEK necesita un orden activo: use INDEX ON o SET ORDER TO"));
        }
        let exact = self.exact;
        let matches = |key: &Value| match (key, wanted) {
            (Value::Str(k), Value::Str(w)) if !exact => k.starts_with(w.as_str()),
            (Value::Str(k), Value::Str(w)) => k.trim_end_matches(' ') == w.trim_end_matches(' '),
            _ => compare(key, wanted) == Some(Ordering::Equal),
        };
        let keys = self.keys(n)?;
        let hit = keys.iter().find(|k| matches(&k.0)).map(|k| k.1);
        self.place(n, hit);
        self.area(n).found = hit.is_some();
        Ok(hit.is_some())
    }

    pub(crate) fn exec_data(&mut self, cmd: &DataCmd) -> R<()> {
        match cmd {
            DataCmd::Create { name, cursor, fields } => {
                let schema = if *cursor { "main" } else { "disk" };
                if !*cursor {
                    self.attach(true)?;
                }
                // a cursor is replaced by a new one of the same name; a table is never overwritten silently
                let open: Vec<usize> = self.db()?.areas.iter().filter(|(_, a)| a.schema == schema && a.table == *name).map(|(n, _)| *n).collect();
                let exists: bool = {
                    let d = self.db()?;
                    let query = format!("SELECT COUNT(*) FROM {schema}._fost_fields WHERE tbl = ?1");
                    sql(d.conn.query_row(&query, [name], |r| r.get::<_, i64>(0)))? > 0
                };
                if exists && !*cursor {
                    return Err(err(format!("La tabla {name} ya existe")));
                }
                for n in open {
                    self.close_area(n)?;
                }
                let columns: Vec<String> = fields.iter().map(|f| format!("\"{}\"", f.name)).collect();
                let d = self.db()?;
                sql(d.conn.execute_batch(&format!(
                    "CREATE TABLE {schema}.\"{name}\" (_recno INTEGER PRIMARY KEY, _deleted INTEGER NOT NULL DEFAULT 0, {})",
                    columns.join(", ")
                )))?;
                for (pos, f) in fields.iter().enumerate() {
                    let insert = format!("INSERT INTO {schema}._fost_fields VALUES (?1, ?2, ?3, ?4, ?5, ?6)");
                    sql(d.conn.execute(&insert, rusqlite::params![name, pos as i64, f.name, f.kind.to_string(), f.width as i64, f.decimals as i64]))?;
                }
                // as in FoxPro, the new table is opened in a free area, which becomes the current one
                let n = self.area_number(&Value::Num(0.0))?;
                self.open(name, None, n, None)?;
                self.db()?.current = n;
            }
            DataCmd::Use { table, alias, area, order } => {
                let n = match area {
                    Some(e) => {
                        let v = self.eval(e)?;
                        self.area_number(&v)?
                    }
                    None => self.db()?.current,
                };
                match table {
                    None => self.close_area(n)?,
                    Some(e) => {
                        let Value::Str(name) = self.eval(e)? else {
                            return Err(err("El nombre de la tabla debe ser texto"));
                        };
                        let name = name.trim().to_uppercase();
                        self.open(&name, alias.as_deref(), n, order.as_deref())?;
                    }
                }
            }
            DataCmd::Select(e) => {
                let v = self.eval(e)?;
                let n = self.area_number(&v)?;
                self.db()?.current = n;
            }
            DataCmd::CloseAll => {
                let open: Vec<usize> = self.db()?.areas.keys().copied().collect();
                for n in open {
                    self.close_area(n)?;
                }
                self.db()?.current = 1;
            }
            DataCmd::AppendBlank => {
                let n = self.current_area()?;
                self.append(n, HashMap::new())?;
            }
            DataCmd::Insert { table, fields, values } => {
                // the table is used where it is open, or opened in a free area; the current area does not change
                let open = self.db()?.areas.iter().find(|(_, a)| a.alias == *table || a.table == *table).map(|(n, _)| *n);
                let n = match open {
                    Some(n) => n,
                    None => {
                        let n = self.area_number(&Value::Num(0.0))?;
                        self.open(table, None, n, None)?;
                        n
                    }
                };
                let names: Vec<String> = if fields.is_empty() { self.area(n).fields.iter().map(|f| f.name.clone()).collect() } else { fields.clone() };
                if names.len() != values.len() {
                    return Err(err("INSERT: el número de valores no coincide con el de campos"));
                }
                let mut row = HashMap::new();
                for (name, e) in names.iter().zip(values) {
                    if !self.area(n).fields.iter().any(|f| f.name == *name) {
                        return Err(err(format!("Campo no encontrado: {name}")));
                    }
                    row.insert(name.clone(), self.eval(e)?);
                }
                self.append(n, row)?;
            }
            DataCmd::Replace { pairs, scope } => self.each(scope, false, |it, n| {
                for (alias, field, e) in pairs {
                    let v = it.eval(e)?;
                    let target = match alias {
                        Some(alias) => it.area_number(&Value::Str(alias.clone()))?,
                        None => n,
                    };
                    it.replace(target, field, v)?;
                }
                Ok(())
            })?,
            DataCmd::Delete { recall, scope } => {
                // RECALL must reach the very records SET DELETED ON hides
                let hidden = self.db()?.hide_deleted;
                if *recall && hidden {
                    self.set_deleted(false)?;
                }
                let done = self.each(scope, false, |it, n| it.mark(n, !*recall));
                self.set_deleted(hidden)?;
                done?
            }
            DataCmd::Pack | DataCmd::Zap => {
                let n = self.current_area()?;
                let name = self.area(n).sql_name();
                let conn = &self.data.as_ref().unwrap().conn;
                if matches!(cmd, DataCmd::Zap) {
                    sql(conn.execute_batch(&format!("DELETE FROM {name}")))?;
                } else {
                    // the deleted records go, and the rest are renumbered from 1; negatives keep the
                    // renumbering from colliding with numbers still in use
                    sql(conn.execute_batch(&format!(
                        "BEGIN;
                         DELETE FROM {name} WHERE _deleted = 1;
                         CREATE TEMP TABLE _fost_pack AS SELECT _recno AS old, ROW_NUMBER() OVER (ORDER BY _recno) AS new FROM {name};
                         UPDATE {name} SET _recno = -(SELECT new FROM _fost_pack WHERE old = _recno);
                         UPDATE {name} SET _recno = -_recno;
                         DROP TABLE _fost_pack;
                         COMMIT;"
                    )))?;
                }
                // what is gone cannot be put back
                if let Some(log) = self.area(n).buffer.as_mut() {
                    log.clear();
                }
                self.touch(n);
                self.go(n, 0)?;
            }
            DataCmd::SetFilter(cond) => {
                let n = self.current_area()?;
                let a = self.area(n);
                a.filter = cond.clone();
                a.keys = None;
            }
            DataCmd::Browse => self.browse()?,
            DataCmd::SqlSelect(q) => self.sql_select(q)?,
            DataCmd::SqlUpdate { table, pairs, cond } => self.sql_update(table, pairs, cond.as_ref())?,
            DataCmd::SqlDelete { table, cond } => self.sql_delete(table, cond.as_ref())?,
            DataCmd::GoTop => {
                let n = self.current_area()?;
                self.go(n, 0)?;
            }
            DataCmd::GoBottom => {
                let n = self.current_area()?;
                let last = self.keys(n)?.len().saturating_sub(1);
                self.go(n, last)?;
            }
            DataCmd::Go(e) => {
                let n = self.current_area()?;
                let id = self.num(e)? as i64;
                let a = self.area(n);
                let query = format!("SELECT COUNT(*) FROM {} WHERE _recno = ?1", a.sql_name());
                let exists: i64 = sql(self.data.as_ref().unwrap().conn.query_row(&query, [id], |r| r.get(0)))?;
                if exists == 0 {
                    return Err(err(format!("El registro {id} está fuera de rango")));
                }
                self.place(n, Some(id));
            }
            DataCmd::Skip(by) => {
                let n = self.current_area()?;
                let by = match by {
                    Some(e) => self.num(e)? as i64,
                    None => 1,
                };
                self.skip(n, by)?;
            }
            DataCmd::Locate(scope) => {
                let n = self.current_area()?;
                let bounded = scope.rest || scope.next.is_some() || scope.until.is_some();
                let from = if bounded && !scope.all { self.index(n)? } else { 0 };
                self.area(n).locate = Some(scope.clone());
                self.locate(n, scope, from)?;
            }
            DataCmd::Continue => {
                let n = self.current_area()?;
                let Some(scope) = self.area(n).locate.clone() else {
                    return Err(err("CONTINUE necesita un LOCATE anterior"));
                };
                let from = self.index(n)? + 1;
                self.locate(n, &scope, from)?;
            }
            DataCmd::Index { expr, text, tag, descending } => {
                let n = self.current_area()?;
                let key = self.area(n).tag_key();
                // the order of a table of the folder is kept with it, for the next time it is opened
                if self.area(n).schema == "disk" {
                    let table = self.area(n).table.clone();
                    let conn = &self.data.as_ref().unwrap().conn;
                    sql(conn.execute("DELETE FROM disk._fost_tags WHERE tbl = ?1 AND name = ?2", [&table, tag]))?;
                    sql(conn.execute("INSERT INTO disk._fost_tags VALUES (?1, ?2, ?3, ?4)", rusqlite::params![table, tag, text, descending]))?;
                }
                let tags = self.data.as_mut().unwrap().tags.entry(key).or_default();
                tags.retain(|t| t.name != *tag);
                tags.push(Tag { name: tag.clone(), expr: expr.clone(), descending: *descending });
                // INDEX ON also makes the new order the current one
                let a = self.area(n);
                a.order = Some(tag.clone());
                a.keys = None;
                self.go(n, 0)?;
            }
            DataCmd::SetOrder(tag) => {
                let n = self.current_area()?;
                if let Some(tag) = tag {
                    let key = self.area(n).tag_key();
                    if !self.data.as_ref().unwrap().tags.get(&key).is_some_and(|tags| tags.iter().any(|t| t.name == *tag)) {
                        return Err(err(format!("Orden no encontrado: {tag}")));
                    }
                }
                // the pointer stays on its record; only the neighbours change
                let a = self.area(n);
                a.order = tag.clone();
                a.keys = None;
            }
            DataCmd::Seek(e) => {
                let n = self.current_area()?;
                let wanted = self.eval(e)?;
                self.seek(n, &wanted)?;
            }
            DataCmd::Count { scope, var } => {
                let mut count = 0.0;
                self.each(scope, true, |_, _| {
                    count += 1.0;
                    Ok(())
                })?;
                match var {
                    Some(var) => self.set_var(var, Value::Num(count)),
                    None => self.print(&Value::Num(count).display()),
                }
            }
            DataCmd::Sum { exprs, vars, scope } => {
                if exprs.len() != vars.len() {
                    return Err(err("SUM necesita una variable por cada expresión"));
                }
                let mut totals = vec![0.0; exprs.len()];
                self.each(scope, true, |it, _| {
                    for (total, e) in totals.iter_mut().zip(exprs) {
                        *total += it.num(e)?;
                    }
                    Ok(())
                })?;
                for (var, total) in vars.iter().zip(totals) {
                    self.set_var(var, Value::Num(total));
                }
            }
            DataCmd::List { exprs, scope, numbers } => {
                let n = self.current_area()?;
                let fields = self.area(n).fields.clone();
                if exprs.is_empty() {
                    let names: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
                    self.print(&format!("{}{}", if *numbers { "Registro  " } else { "" }, names.join("  ")));
                }
                self.each(scope, true, |it, n| {
                    let row = it.row(n)?;
                    let mut cells = Vec::new();
                    if exprs.is_empty() {
                        cells.extend(fields.iter().map(|f| row[&f.name].display()));
                    }
                    for e in exprs {
                        cells.push(it.eval(e)?.display());
                    }
                    let mark = if row["_DELETED"] == Value::Bool(true) { "*" } else { " " };
                    let id = it.area(n).pos.unwrap_or(0);
                    let number = if *numbers { format!("{id:>7} {mark} ") } else { String::new() };
                    it.print(&format!("{number}{}", cells.join("  ")));
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    /// SCAN ... ENDSCAN: runs the body once per record. The body may move the pointer or change area:
    /// each pass comes back to the scan's area and steps on from wherever its pointer is.
    pub(crate) fn scan(&mut self, scope: &RecScope, body: &[Stmt]) -> R<Flow> {
        let n = self.current_area()?;
        let bounded = scope.rest || scope.next.is_some() || scope.until.is_some();
        if scope.all || !bounded {
            self.go(n, 0)?;
        }
        let mut left = match &scope.next {
            Some(count) => self.num(count)?.max(0.0) as usize,
            None => usize::MAX,
        };
        loop {
            let d = self.data.as_mut().unwrap();
            // the body closed the table: there is nothing left to scan
            let Some(a) = d.areas.get(&n) else { break };
            if a.pos.is_none() || left == 0 {
                break;
            }
            d.current = n;
            if scope.until.as_ref().map(|w| self.truthy(w)).transpose()? == Some(false) {
                break;
            }
            if scope.cond.as_ref().map(|c| self.truthy(c)).transpose()? != Some(false) {
                if let Some(flow) = self.iteration(body)? {
                    return Ok(flow);
                }
            }
            left -= 1;
            if self.data.as_ref().unwrap().areas.contains_key(&n) {
                self.data.as_mut().unwrap().current = n;
                self.skip(n, 1)?;
            }
        }
        Ok(Flow::Next)
    }

    /// The functions that ask about tables. None when `name` is not one of them.
    pub(crate) fn data_function(&mut self, name: &str, args: &[Expr]) -> R<Option<Value>> {
        if let "TABLEUPDATE" | "TABLEREVERT" | "CURSORSETPROP" = name {
            return self.buffer_function(name, args).map(Some);
        }
        if !["RECNO", "RECCOUNT", "EOF", "BOF", "FOUND", "DELETED", "ALIAS", "SELECT", "USED", "FCOUNT", "FIELD", "SEEK"].contains(&name) {
            return Ok(None);
        }
        // most take an optional area, as a number or an alias, and default to the current one
        let area_arg = match (name, args) {
            ("FIELD", [_, a]) | ("SEEK", [_, a]) => Some(a),
            ("FIELD" | "SEEK", _) => None,
            (_, [a, ..]) => Some(a),
            _ => None,
        };
        let asked = match area_arg {
            Some(e) => Some(self.eval(e)?),
            None => None,
        };
        // an alias that is not open is not an error for these: USED("x") and SELECT("x") ask exactly that
        let n = match &asked {
            Some(Value::Num(n)) if *n == 0.0 && name == "SELECT" => self.data.as_ref().map(|d| d.current),
            Some(v) => self.area_number(v).ok(),
            None => self.data.as_ref().map(|d| d.current),
        };
        let n = n.filter(|n| self.data.as_ref().is_some_and(|d| d.areas.contains_key(n)));
        Ok(Some(match name {
            "SELECT" => Value::Num(match &asked {
                Some(Value::Str(_)) => n.unwrap_or(0) as f64,
                _ => self.data.as_ref().map_or(1, |d| d.current) as f64,
            }),
            "USED" => Value::Bool(n.is_some()),
            "ALIAS" => Value::Str(n.map_or(String::new(), |n| self.area(n).alias.clone())),
            "FOUND" => Value::Bool(n.is_some_and(|n| self.area(n).found)),
            "BOF" => Value::Bool(n.is_some_and(|n| self.area(n).bof)),
            "EOF" => Value::Bool(n.is_some_and(|n| self.area(n).pos.is_none())),
            "FCOUNT" => Value::Num(n.map_or(0, |n| self.area(n).fields.len()) as f64),
            "FIELD" => {
                let index = match args.first() {
                    Some(e) => self.num(e)? as usize,
                    None => return Err(err("Faltan argumentos")),
                };
                let name = n.and_then(|n| self.area(n).fields.get(index.wrapping_sub(1)).map(|f| f.name.clone()));
                Value::Str(name.unwrap_or_default())
            }
            "DELETED" => match n {
                Some(n) => self.row(n)?["_DELETED"].clone(),
                None => Value::Bool(false),
            },
            "RECNO" | "RECCOUNT" => match n {
                None => Value::Num(0.0),
                Some(n) => {
                    let a = self.area(n);
                    let (pos, table) = (a.pos, a.sql_name());
                    let conn = &self.data.as_ref().unwrap().conn;
                    let count: i64 = sql(conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)))?;
                    let last: i64 = sql(conn.query_row(&format!("SELECT COALESCE(MAX(_recno), 0) FROM {table}"), [], |r| r.get(0)))?;
                    // past the last record, RECNO() is one more than the highest number
                    Value::Num(if name == "RECCOUNT" { count } else { pos.unwrap_or(last + 1) } as f64)
                }
            },
            _ => {
                let wanted = match args.first() {
                    Some(e) => self.eval(e)?,
                    None => return Err(err("Faltan argumentos")),
                };
                let Some(n) = n else { return Err(err("No hay ninguna tabla abierta en esta área de trabajo")) };
                Value::Bool(self.seek(n, &wanted)?)
            }
        }))
    }
}
