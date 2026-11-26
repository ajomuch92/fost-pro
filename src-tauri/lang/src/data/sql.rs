//! The SQL commands: SELECT, UPDATE and DELETE FROM, run by the interpreter over one table.

use super::*;
use crate::interp::var;
use crate::is_kw;
use crate::parser::{Column, Dest, Select};
use crate::value::Array;
use std::collections::HashSet;

/// What a column of a grouped query has gathered so far.
enum Acc {
    /// Not an aggregate: the value in the group's first record.
    Plain(Option<Value>),
    Count(f64),
    Sum(f64),
    Avg(f64, f64),
    Min(Option<Value>),
    Max(Option<Value>),
}

/// The aggregate a column of a query is, if it is one, and the expression it takes (None for COUNT(*)).
fn aggregate(e: &Expr) -> Option<(&'static str, Option<&Expr>)> {
    let Expr::Call(name, args) = e else { return None };
    let kind = ["COUNT", "CNT", "SUM", "AVG", "MIN", "MAX"].into_iter().find(|k| is_kw(name, k))?;
    let kind = if kind == "CNT" { "COUNT" } else { kind };
    match args.as_slice() {
        [] if kind == "COUNT" => Some((kind, None)),
        // MIN(a, b) and MAX(a, b) are the ordinary functions
        [arg] => Some((kind, Some(arg))),
        _ => None,
    }
}

/// The type of a computed column, from the values it got.
fn infer(values: &mut dyn Iterator<Item = &Value>) -> FieldDef {
    let (mut kind, mut width) = ('C', 1);
    for v in values {
        match v {
            Value::Str(s) => (kind, width) = ('C', width.max(s.chars().count())),
            Value::Num(_) => kind = 'N',
            Value::Bool(_) => kind = 'L',
            Value::Date(_) => kind = 'D',
            _ => continue,
        }
        if kind != 'C' {
            break;
        }
    }
    match kind {
        'C' if width > 254 => FieldDef { name: String::new(), kind: 'M', width: 0, decimals: 0 },
        'N' => FieldDef { name: String::new(), kind, width: 20, decimals: 8 },
        _ => FieldDef { name: String::new(), kind, width, decimals: 0 },
    }
}

impl Interp {
    /// Runs `f` with the table current and the numbers of its records, then puts everything back.
    /// SQL sees every record in record order, whatever order or filter the table is open with.
    fn with_table<T>(&mut self, table: &str, alias: Option<&str>, f: impl FnOnce(&mut Self, usize, Vec<i64>) -> R<T>) -> R<T> {
        let wanted = alias.unwrap_or(table);
        let d = self.db()?;
        let was = d.current;
        let open = d.areas.iter().find(|(_, a)| a.alias == wanted).map(|(n, a)| (*n, a.table == table));
        let (n, temporary) = match open {
            Some((n, true)) => (n, false),
            Some(_) => return Err(err(format!("El alias {wanted} ya está en uso"))),
            None => {
                let n = self.area_number(&Value::Num(0.0))?;
                self.open(table, alias, n, None)?;
                (n, true)
            }
        };
        let a = self.area(n);
        let saved = (a.order.take(), a.filter.take(), a.pos, a.bof);
        a.keys = None;
        self.db()?.current = n;
        let res = self.keys(n).and_then(|keys| f(self, n, keys.iter().map(|k| k.1).collect()));
        if temporary {
            self.close_area(n)?;
        } else if self.db()?.areas.contains_key(&n) {
            let a = self.area(n);
            (a.order, a.filter, a.keys) = (saved.0, saved.1, None);
            self.place(n, saved.2);
            self.area(n).bof = saved.3;
        }
        self.db()?.current = was;
        res
    }

    /// _TALLY: how many records the last SQL command took.
    fn tally(&mut self, count: usize) {
        self.frames[0].vars.insert("_TALLY".into(), (var(Value::Num(count as f64)), false));
    }

    pub(super) fn sql_select(&mut self, q: &Select) -> R<()> {
        let grouped = !q.group.is_empty() || q.items.iter().any(|(e, _)| aggregate(e).is_some());
        if grouped && q.items.is_empty() {
            return Err(err("Una consulta con GROUP BY debe nombrar sus columnas"));
        }
        let fresh = || -> Vec<Acc> {
            let acc = |(e, _): &(Expr, Option<String>)| match aggregate(e).map(|a| a.0) {
                Some("COUNT") => Acc::Count(0.0),
                Some("SUM") => Acc::Sum(0.0),
                Some("AVG") => Acc::Avg(0.0, 0.0),
                Some("MIN") => Acc::Min(None),
                Some(_) => Acc::Max(None),
                None => Acc::Plain(None),
            };
            q.items.iter().map(acc).collect()
        };
        let (fields, mut rows) = self.with_table(&q.table, q.alias.as_deref(), |it, n, ids| {
            let fields = it.area(n).fields.clone();
            let mut rows: Vec<Vec<Value>> = Vec::new();
            let mut groups: Vec<Vec<Acc>> = Vec::new();
            let mut index: HashMap<String, usize> = HashMap::new();
            for id in ids {
                it.place(n, Some(id));
                if q.cond.as_ref().map(|c| it.truthy(c)).transpose()? == Some(false) {
                    continue;
                }
                if !grouped {
                    rows.push(if q.items.is_empty() {
                        let row = it.row(n)?;
                        fields.iter().map(|f| row[&f.name].clone()).collect()
                    } else {
                        q.items.iter().map(|(e, _)| it.eval(e)).collect::<R<_>>()?
                    });
                    continue;
                }
                let mut key = String::new();
                for g in &q.group {
                    key.push_str(&it.eval(g)?.display());
                    key.push('\u{1}');
                }
                let at = *index.entry(key).or_insert_with(|| {
                    groups.push(fresh());
                    groups.len() - 1
                });
                for (acc, (e, _)) in groups[at].iter_mut().zip(&q.items) {
                    let arg = aggregate(e).and_then(|a| a.1);
                    match (acc, arg) {
                        (Acc::Plain(v), _) if v.is_none() => *v = Some(it.eval(e)?),
                        (Acc::Count(c), None) => *c += 1.0,
                        (Acc::Count(c), Some(arg)) => *c += (it.eval(arg)? != Value::Null) as u8 as f64,
                        (Acc::Sum(s), Some(arg)) => *s += it.num(arg)?,
                        (Acc::Avg(s, c), Some(arg)) => (*s, *c) = (*s + it.num(arg)?, *c + 1.0),
                        (Acc::Min(m) | Acc::Max(m), Some(arg)) => {
                            let v = it.eval(arg)?;
                            let wanted = if aggregate(e).unwrap().0 == "MIN" { Ordering::Less } else { Ordering::Greater };
                            if m.as_ref().is_none_or(|old| compare(&v, old) == Some(wanted)) {
                                *m = Some(v);
                            }
                        }
                        _ => {}
                    }
                }
            }
            if grouped {
                // totals over no records are still one row
                if groups.is_empty() && q.group.is_empty() {
                    groups.push(fresh());
                }
                let value = |acc: Acc| match acc {
                    Acc::Plain(v) | Acc::Min(v) | Acc::Max(v) => v.unwrap_or(Value::Null),
                    Acc::Count(n) | Acc::Sum(n) => Value::Num(n),
                    Acc::Avg(sum, count) => if count > 0.0 { Value::Num(sum / count) } else { Value::Null },
                };
                rows = groups.into_iter().map(|g| g.into_iter().map(value).collect()).collect();
            }
            Ok((fields, rows))
        })?;

        let mut defs: Vec<FieldDef> = fields.to_vec();
        if !q.items.is_empty() {
            defs.clear();
            for (i, (e, name)) in q.items.iter().enumerate() {
                let (agg, inner) = match aggregate(e) {
                    Some((kind, arg)) => (Some(kind), arg),
                    None => (None, Some(e)),
                };
                let source = match inner {
                    Some(Expr::Var(f) | Expr::Member(_, f)) => fields.iter().find(|x| x.name == *f),
                    _ => None,
                };
                let auto = match (agg, source) {
                    (None, Some(f)) => f.name.clone(),
                    (Some("COUNT"), _) => "CNT".into(),
                    (Some(kind), Some(f)) => format!("{kind}_{}", f.name),
                    _ => format!("EXP_{}", i + 1),
                };
                // a field, or the least or greatest of one, keeps the field's type; the rest take it from their values
                let template = if matches!(agg, None | Some("MIN" | "MAX")) { source.cloned() } else { None };
                let mut def = template.unwrap_or_else(|| infer(&mut rows.iter().map(|r| &r[i])));
                def.name = name.clone().unwrap_or(auto);
                if defs.iter().any(|d| d.name == def.name) {
                    def.name = format!("{}_{}", def.name, i + 1);
                }
                defs.push(def);
            }
        }

        if q.distinct {
            let mut seen = HashSet::new();
            rows.retain(|r| seen.insert(r.iter().map(|v| v.display()).collect::<Vec<_>>().join("\u{1}")));
        }
        let mut order = Vec::new();
        for (column, descending) in &q.order {
            let at = match column {
                Column::Pos(p) => Some(p.wrapping_sub(1)).filter(|p| *p < defs.len()),
                Column::Name(name) => defs.iter().position(|d| d.name == *name),
            };
            order.push((at.ok_or_else(|| err("ORDER BY debe nombrar una columna del resultado"))?, *descending));
        }
        rows.sort_by(|a, b| {
            let by = |(i, descending): &(usize, bool)| {
                let o = compare(&a[*i], &b[*i]).unwrap_or(Ordering::Equal);
                if *descending { o.reverse() } else { o }
            };
            order.iter().map(by).find(|o| *o != Ordering::Equal).unwrap_or(Ordering::Equal)
        });
        if let Some(top) = &q.top {
            rows.truncate(self.num(top)?.max(0.0) as usize);
        }
        self.tally(rows.len());

        let (name, cursor) = match &q.into {
            Dest::Array(name) => {
                // as in FoxPro, a query that finds nothing leaves the array alone
                if !rows.is_empty() {
                    let mut array = Array::new(&[rows.len() as f64, defs.len() as f64])?;
                    array.data = rows.into_iter().flatten().collect();
                    self.set_var(name, Value::Array(array));
                }
                return Ok(());
            }
            Dest::Cursor(name) => (name.clone(), true),
            Dest::Table(name) => (name.clone(), false),
            Dest::Screen => ("QUERY".to_string(), true),
        };
        self.exec_data(&DataCmd::Create { name, cursor, fields: defs.clone() })?;
        let n = self.current_area()?;
        for row in rows {
            self.append(n, defs.iter().map(|d| d.name.clone()).zip(row).collect())?;
        }
        self.go(n, 0)?;
        if matches!(q.into, Dest::Screen) {
            self.browse()?;
        }
        Ok(())
    }

    pub(super) fn sql_update(&mut self, table: &str, pairs: &[(String, Expr)], cond: Option<&Expr>) -> R<()> {
        let count = self.with_table(table, None, |it, n, ids| {
            let mut count = 0;
            for id in ids {
                it.place(n, Some(id));
                if cond.map(|c| it.truthy(c)).transpose()? == Some(false) {
                    continue;
                }
                for (field, e) in pairs {
                    let v = it.eval(e)?;
                    it.replace(n, field, v)?;
                }
                count += 1;
            }
            Ok(count)
        })?;
        self.tally(count);
        Ok(())
    }

    /// DELETE FROM: marks the records, as DELETE does. PACK removes them.
    pub(super) fn sql_delete(&mut self, table: &str, cond: Option<&Expr>) -> R<()> {
        let count = self.with_table(table, None, |it, n, ids| {
            let mut count = 0;
            for id in ids {
                it.place(n, Some(id));
                if cond.map(|c| it.truthy(c)).transpose()? == Some(false) {
                    continue;
                }
                it.mark(n, true)?;
                count += 1;
            }
            Ok(count)
        })?;
        self.tally(count);
        Ok(())
    }
}
