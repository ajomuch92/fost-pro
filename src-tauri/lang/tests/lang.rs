use fost_lang::Interp;

/// Runs a program and returns what it printed, one `?` per line.
fn run(src: &str) -> String {
    let mut it = Interp::default();
    let res = it.run(src);
    let out = it.take_output();
    let out = out.strip_prefix('\n').unwrap_or(&out).to_string();
    match res {
        Ok(()) => out,
        Err(e) => format!("{out}!{e}"),
    }
}

#[test]
fn expressions_and_precedence() {
    assert_eq!(run("? 1 + 2 * 3, (1 + 2) * 3, 2 ^ 3 ^ 2, -2 ^ 2, 7 % 3, -7 % 3"), "7 9 64 4 1 2");
    assert_eq!(run("? 10 / 4, 1 / 3, 0.1 + 0.2, 1.5e3"), "2.5 0.3333333333 0.3 1500");
    assert_eq!(run("? 1 < 2 AND NOT 2 < 1, .F. .OR. .T., !.T., 1 = 1 OR 1 / 0 = 1"), ".T. .T. .F. .T.");
    assert_eq!(run("? 1 / 0"), "!Línea 1: División entre cero");
    assert_eq!(run("? 1 + 'a'"), "!Línea 1: Los tipos de datos no coinciden con el operador");
}

#[test]
fn strings() {
    assert_eq!(run("? 'ab' + \"cd\" + [ef], 'ab  ' - 'cd' + '|'"), "abcdef abcd  |");
    // `=` stops at the end of the right side; `==` is exact
    assert_eq!(run("? 'abc' = 'ab', 'abc' == 'ab', 'ab' = 'abc', 'abc' <> 'ab', 'b' $ 'abc'"), ".T. .F. .F. .F. .T.");
    assert_eq!(run("? UPPER('año'), LEN('año'), SUBSTR('abcdef', 2, 3), LEFT('abc', 2), RIGHT('abc', 2)"), "AÑO 3 bcd ab bc");
    assert_eq!(run("? '[' + ALLTRIM('  a b  ') + ']', AT('b', 'abcabc'), AT('b', 'abcabc', 2), RAT('b', 'abcabc')"), "[a b] 2 5 5");
    assert_eq!(run("? PADL(7, 3, '0'), PADR('ab', 4) + '|', PADC('ab', 6, '*'), STR(3.14159, 6, 2), STR(2.5)"), "007 ab  | **ab**   3.14          3");
    assert_eq!(run("? VAL('12.5abc'), VAL('x'), STRTRAN('a-b-c', '-', '+'), STUFF('abcdef', 2, 3, 'X'), PROPER('juan PEREZ')"), "12.5 0 a+b+c aXef Juan Perez");
    assert_eq!(run("? CHRTRAN('abc', 'ac', 'x'), OCCURS('a', 'banana'), REPLICATE('ab', 2), ASC('A'), CHR(66)"), "xb 3 abab 65 B");
}

#[test]
fn abbreviations_and_comments() {
    let src = "
* a comment line ;
  continued here
SET TALK OFF
x = 10 && trailing comment
IF x > 5 ;
   AND x < 20
    ? ALLT('  ok  '), SUBS('abcd', 2, 2)
ENDI anything after ENDIF is ignored
NOTE = 3
? m.note, m.x
";
    assert_eq!(run(src), "ok bc\n3 10");
}

#[test]
fn control_flow() {
    let src = "
total = 0
FOR i = 1 TO 10
    IF i % 2 = 0
        LOOP
    ENDIF
    IF i > 7
        EXIT
    ENDIF
    total = total + i
ENDFOR
? total, i
FOR i = 3 TO 1 STEP -1
    ?? i
NEXT
n = 0
DO WHILE .T.
    n = n + 1
    DO CASE
    CASE n = 1
        ?? ' uno'
    CASE n = 2
        ?? ' dos'
    OTHERWISE
        EXIT
    ENDCASE
ENDDO
";
    assert_eq!(run(src), "16 9321 uno dos");
}

#[test]
fn routines_and_scope() {
    let src = "
x = 1
DO bump WITH x
? x, twice(x), fact(5)
? twice(@x), x
priv = 'outer'
DO shadow
? priv, VARTYPE(loc), TYPE('glob')

PROCEDURE bump
PARAMETERS n
    n = n + 1
ENDPROC

FUNCTION twice(n)
    n = n * 2
    RETURN n
ENDFUNC

FUNCTION fact
LPARAMETERS n
    RETURN IIF(n <= 1, 1, n * fact(n - 1))

PROCEDURE shadow
    PRIVATE priv
    LOCAL loc
    PUBLIC glob
    priv = 'inner'
    loc = 1
    glob = 1
    DO peek

PROCEDURE peek
    ? priv, VARTYPE(loc)
";
    assert_eq!(run(src), "2 4 120\n4 4\ninner U\nouter U N");
}

#[test]
fn arrays() {
    let src = "
DIMENSION a(3), grid[2, 2]
a = 0
a[2] = 5
a(3) = a[2] * 2
grid[2, 1] = 'x'
? a[1], a(2), a[3], grid[2, 1], ALEN(a), ALEN(grid, 1), ALEN(grid, 2)
sum = 0
FOR EACH item IN a
    sum = sum + item
ENDFOR
? sum
? a[4]
";
    assert_eq!(run(src), "0 5 10 x 3 2 2\n15!Línea 13: Subíndice fuera de rango");
}

#[test]
fn dates_and_types() {
    assert_eq!(run("d = {^2024-01-31}\n? d + 1, d - {^2024-01-01}, YEAR(d), DOW(d), CDOW(d), DTOS(GOMONTH(d, 1))"), "02/01/2024 30 2024 4 Wednesday 20240229");
    assert_eq!(run("? CTOD('12/25/2024') > {^2024-01-01}, EMPTY({}), EMPTY(CTOD('x')), DTOC({}) + '|'"), ".T. .T. .T.   /  /    |");
    assert_eq!(run("STORE .NULL. TO n1, n2\n? ISNULL(n1 + 1), NVL(n2, 'x'), EMPTY(''), EMPTY(0), EVALUATE('1 + 2'), TYPE('nope')"), ".T. x .T. .T. 3 U");
    assert_eq!(run("? BETWEEN(5, 1, 10), INLIST('b', 'a', 'b'), MAX(1, 7, 3), MIN('b', 'a'), ROUND(2.5, 0), INT(-2.7), MOD(-7, 3)"), ".T. .T. 7 a 3 -2 2");
}

#[test]
fn errors_point_at_the_line() {
    assert_eq!(run("x = 1\n? y"), "!Línea 2: Variable no encontrada: Y");
    assert_eq!(run("IF .T.\n? 1"), "!Línea 1: IF sin cerrar");
    assert_eq!(run("? 1\nENDIF"), "!Línea 2: ENDIF sin su bloque de apertura");
    assert_eq!(run("REINDEX"), "!Línea 1: Comando no reconocido: REINDEX");
    assert_eq!(run("SET DATE DMY"), "!Línea 1: SET DATE aún no está soportado");
    assert_eq!(run("? 'a'\n? nope(1)"), "a!Línea 2: Función no encontrada: NOPE");
}

#[test]
fn runaway_recursion_is_an_error_not_a_crash() {
    let deep = || run("? deep(1)\nFUNCTION deep(n)\nRETURN deep(n + 1)");
    let out = std::thread::Builder::new().stack_size(fost_lang::STACK_SIZE).spawn(deep).unwrap().join().unwrap();
    assert_eq!(out, "!Línea 3: Demasiadas llamadas anidadas (máximo 128)");
}

#[test]
fn state_survives_between_runs_like_a_command_window() {
    let mut it = Interp::default();
    it.run("x = 5").unwrap();
    it.run("FUNCTION dbl(n)\nRETURN n * 2").unwrap();
    assert!(it.run("? nope").is_err());
    it.run("? dbl(x)").unwrap();
    assert_eq!(it.take_output(), "\n10");
    it.run("CLEAR").unwrap();
    assert!(it.clear);
}

#[test]
fn stop_flag_interrupts_a_loop() {
    let mut it = Interp::default();
    let stop = it.stop.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        stop.store(true, std::sync::atomic::Ordering::Relaxed);
    });
    assert_eq!(it.run("DO WHILE .T.\nENDDO").unwrap_err().msg, "Ejecución interrumpida");
}

#[test]
fn define_and_set() {
    assert_eq!(run("#DEFINE TAB CHR(65)\n#DEFINE TWICE TAB + TAB\n? TWICE"), "AA");
    assert_eq!(run("#IF .T."), "!Línea 1: Directiva de preprocesador aún no soportada");
    assert_eq!(run("? 'abc' = 'ab', 'ab ' = 'ab'\nSET EXACT ON\n? 'abc' = 'ab', 'ab ' = 'ab', 'abc' <> 'ab'"), ".T. .T.\n.F. .T. .T.");
    assert_eq!(run("SET DECIMALS TO 2\n? 1 / 3, 2 / 3, 10 / 4, 5\nSET DECI TO 0\n? 2.5, 50"), "0.33 0.67 2.5 5\n3 50");
    assert_eq!(run("2 + 2"), "!Línea 1: Sentencia no válida. Para ver el valor de una expresión, escribe ? delante");
}

#[test]
fn do_runs_a_program_file_and_errors_name_their_origin() {
    let dir = std::env::temp_dir().join(format!("fost-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Saluda.fost"), "PARAMETERS quien\n? 'Hola ' + quien\nDO helper\n\nPROCEDURE helper\n? 1 / 0\n").unwrap();
    std::fs::write(dir.join("roto.fost"), "IF .T.\n").unwrap();
    let mut it = Interp::default();
    it.dir = dir.clone();

    let e = it.run("x = 1\nDO saluda WITH 'Ana'").unwrap_err();
    assert_eq!(it.take_output(), "\nHola Ana");
    assert_eq!(e.to_string(), "Saluda.fost, línea 6: División entre cero");
    assert_eq!(it.run("DO roto").unwrap_err().to_string(), "roto.fost, línea 1: IF sin cerrar");
    assert_eq!(it.run("DO nada").unwrap_err().to_string(), "Línea 1: Procedimiento o programa no encontrado: NADA");

    // a routine kept from an earlier run must not report its lines as if they were in the new source
    it.run("FUNCTION boom\nRETURN 1 / 0").unwrap();
    assert_eq!(it.run("? 1\n? boom()").unwrap_err().to_string(), "un comando anterior, línea 2: División entre cero");
    assert_eq!(it.run("? nope").unwrap_err().file, None);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn classes_and_objects() {
    let src = "
o = CREATEOBJECT('Counter', 10)
o.Add(5)
o.Add()
? o.Total, o.Describe(), VARTYPE(o), o.Class, o.BaseClass
WITH o
    .Total = .Total * 2
    ? .Total, .History[1] + .History(2)
ENDWITH
other = o
other.Total = 1
? o.Total, o = other, o = CREATEOBJECT('Counter', 0)
? o.Nope

DEFINE CLASS Base AS Custom
    Total = 0
    PROCEDURE Init(start)
        THIS.Total = start
    PROCEDURE Describe
        RETURN 'total ' + STR(THIS.Total, 2)
ENDDEFINE

DEFINE CLASS Counter AS Base
    DIMENSION History[2]
    Step = 1
    PROCEDURE Init(start)
        DODEFAULT(start)
        THIS.History = 0
    ENDPROC
    PROCEDURE Add(n)
        THIS.Total = THIS.Total + IIF(VARTYPE(n) = 'N', n, THIS.Step)
        THIS.History[1] = THIS.Total
        THIS.History[2] = 100
    ENDPROC
    FUNCTION Describe
        RETURN 'counter, ' + DODEFAULT()
    ENDFUNC
ENDDEFINE
";
    assert_eq!(
        run(src),
        "16 counter, total 16 O COUNTER Custom\n32 116\n1 .T. .F.!Línea 13: Propiedad no encontrada: NOPE"
    );
    assert_eq!(run("? THIS.x"), "!Línea 1: THIS y THISFORM solo pueden usarse dentro de un método");
    assert_eq!(run("x = CREATEOBJECT('Nope')"), "!Línea 1: Clase no encontrada: NOPE");
    assert_eq!(run("x = 1\n? x.y"), "!Línea 2: Se esperaba un objeto");
    // a member named like a logical literal is still a member
    assert_eq!(run("o = CREATEOBJECT('Custom')\no.AddProperty('t', 7)\n? o.t + 1, .T..AND..T."), "8 .T.");
}

#[test]
fn forms_built_from_classes() {
    let src = "
f = CREATEOBJECT('Login')
? f.txtUser.Value, f.cmdOk.Caption, f.ControlCount, f.Visible
f.cmdOk.Click()
FOR EACH c IN f.Controls
    ?? c.Name + ' '
ENDFOR
f.AddObject('lblNew', 'Label')
? f.lblNew.Visible, f.lblNew.Parent.Caption
f.Release()

DEFINE CLASS Login AS Form
    Caption = 'Entrar'
    ADD OBJECT txtUser AS TextBox WITH Value = 'ana', Left = 10
    ADD OBJECT cmdOk AS CommandButton WITH Caption = 'OK'
    cmdOk.Top = 40
    PROCEDURE cmdOk.Click
        ? 'click', THIS.Top, THISFORM.Caption, THIS.Parent.txtUser.Value
        THISFORM.Hide
    PROCEDURE Destroy
        ? 'bye'
ENDDEFINE
";
    assert_eq!(run(src), "ana OK 2 .F.\nclick 40 Entrar anaTXTUSER CMDOK \n.F. Entrar\nbye");
}

const FORM: &str = r#"{
  "class": "Form",
  "props": { "Name": "frmDemo", "Caption": "Demo", "Width": 300 },
  "methods": { "Init": "LPARAMETERS who\nTHIS.Caption = 'Hola ' + IIF(VARTYPE(who) = 'C', who, 'nadie')" },
  "controls": [
    { "class": "TextBox", "props": { "Name": "txtName", "Left": 10 } },
    { "class": "ComboBox", "props": { "Name": "cboColor", "RowSourceType": 1, "RowSource": "rojo, verde" } },
    { "class": "CommandButton", "props": { "Name": "cmdGo", "Caption": "Ir" },
      "methods": { "Click": "THISFORM.Caption = THISFORM.txtName.Value\n? MESSAGEBOX('hecho')\nTHISFORM.txtName.SetFocus()", "Boom": "x = 1\n? 1 / 0" } }
  ]
}"#;

#[test]
fn forms_from_a_form_file_react_to_ui_events() {
    let mut it = Interp::default();
    it.run_form(FORM, "demo").unwrap();
    let forms = it.forms_json();
    let form = &forms[0];
    assert_eq!(form["props"]["CAPTION"], "Hola nadie");
    assert_eq!(form["props"]["WIDTH"], 300.0);
    assert_eq!(form["children"][1]["items"], serde_json::json!(["rojo", "verde"]));
    let (text, button) = (&form["children"][0], &form["children"][2]);
    assert_eq!(button["base"], "COMMANDBUTTON");
    assert!(button["events"].as_array().unwrap().contains(&"CLICK".into()));

    // the user types, then clicks: the value travels first, the handler sees it
    it.ui_event(text["id"].as_u64().unwrap(), &serde_json::json!({ "Value": "Ana" }), None).unwrap();
    let button_id = button["id"].as_u64().unwrap();
    it.ui_event(button_id, &serde_json::Value::Null, Some("Click")).unwrap();
    assert_eq!(it.forms_json()[0]["props"]["CAPTION"], "Ana");
    // with no UI, MESSAGEBOX prints its text and answers OK
    assert_eq!(it.take_output(), "\nhecho\n1");
    assert_eq!(it.focus, text["id"].as_u64());

    let e = it.ui_event(button_id, &serde_json::Value::Null, Some("Boom")).unwrap_err();
    assert_eq!(e.to_string(), "demo.cmdGo.Boom, línea 2: División entre cero");

    it.ui_event(form["id"].as_u64().unwrap(), &serde_json::Value::Null, Some("close")).unwrap();
    assert_eq!(it.forms_json(), serde_json::json!([]));
    // an event for a form that is gone is ignored
    it.ui_event(button_id, &serde_json::Value::Null, Some("Click")).unwrap();
}

#[test]
fn do_form_and_modify_commands() {
    let dir = std::env::temp_dir().join(format!("fost-forms-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Demo.fostform"), FORM).unwrap();
    std::fs::write(dir.join("roto.fostform"), r#"{ "props": { "Nope": 1 } }"#).unwrap();
    let mut it = Interp::default();
    it.dir = dir.clone();

    it.run("DO FORM demo WITH 'Eva' NAME f\n? f.Caption, f.cboColor.ListCount, f.cboColor.List(2)").unwrap();
    assert_eq!(it.take_output(), "\nHola Eva 2 verde");
    assert_eq!(it.forms_json().as_array().unwrap().len(), 1);
    it.run("DO FORM demo NOSHOW").unwrap();
    assert_eq!(it.forms_json().as_array().unwrap().len(), 1);

    assert_eq!(it.run("DO FORM roto").unwrap_err().to_string(), "Línea 1: roto.fostform: Propiedad no encontrada: NOPE");
    assert_eq!(it.run("DO FORM nada").unwrap_err().to_string(), "Línea 1: Formulario no encontrado: NADA");
    assert_eq!(it.run_form(r#"{ "methods": { "Init": "IF .T." } }"#, "x").unwrap_err().to_string(), "x.Init, línea 1: IF sin cerrar");
    // none of the failed forms was left behind
    it.run("f.Release()").unwrap();
    assert_eq!(it.forms_json().as_array().unwrap().len(), 0);

    it.run("MODIFY FORM demo").unwrap();
    let open = it.open.take().unwrap();
    assert!(open.form);
    assert_eq!(open.path, Some(dir.join("Demo.fostform")));
    it.run("MODI COMM NuevoProg").unwrap();
    assert_eq!(it.open.take().unwrap().path, Some(dir.join("nuevoprog.fost")));
    it.run("MODIFY COMMAND").unwrap();
    assert!(it.open.take().unwrap().path.is_none());
    assert_eq!(it.run("CREATE VIEW x").unwrap_err().msg, "Esta variante de CREATE aún no está soportada");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn help_and_inputbox() {
    let mut it = Interp::default();
    // with no UI, INPUTBOX answers its default
    it.run("? INPUTBOX('Nombre', 'Datos', 'Ana') + '|' + INPUTBOX('x')\nHELP").unwrap();
    assert_eq!(it.take_output(), "\nAna|");
    assert!(it.help);
    let reference = fost_lang::reference();
    assert!(reference["commands"].as_array().unwrap().len() > 30);
    assert_eq!(reference["functions"][0]["syntax"], "LEN(texto)");
    it.run("f = CREATEOBJECT('Form')\n? f.MinButton, f.MaxButton, f.WindowState").unwrap();
    assert_eq!(it.take_output(), "\n.T. .T. 0");
    it.run("f.BorderRadius = 12\nf.AddObject('pgf', 'PageFrame')\nf.pgf.Caption = 'Datos'\n? f.BorderRadius, f.pgf.Caption").unwrap();
    assert_eq!(it.take_output(), "\n12 Datos");
}

#[test]
fn containers_and_the_other_controls() {
    let src = "
f = CREATEOBJECT('Panel')
? f.pgf.PageCount, f.pgf.Page2.Caption, f.pgf.Pages(1).Name, f.pgf.Page1.txt.Value, f.pgf.ActivePage
? f.opt.ButtonCount, f.opt.Option3.Top > f.opt.Option2.Top, f.opt.Buttons(2).Caption, f.opt.Value
f.pgf.PageCount = 1
f.opt.ButtonCount = 4
? f.pgf.PageCount, f.opt.ButtonCount, VARTYPE(f.opt.Option4)
f.pgf.Page1.txt.Check()
? f.box.spn.Value + f.box.spn.Increment, f.tmr.Interval, f.lnk.Url
f.lnk.NavigateTo('https://example.com')

DEFINE CLASS Panel AS Form
    ADD OBJECT pgf AS PageFrame WITH PageCount = 2
    ADD OBJECT opt AS OptionGroup WITH ButtonCount = 3, Value = 2
    ADD OBJECT box AS Container
    ADD OBJECT tmr AS Timer WITH Interval = 500
    ADD OBJECT lnk AS HyperLink WITH Url = 'https://fost.example'
    PROCEDURE Init
        THIS.pgf.Page1.AddObject('txt', 'Field')
        THIS.box.AddObject('spn', 'Spinner')
        THIS.box.AddObject('img', 'Image')
        THIS.box.AddObject('shp', 'Shape')
        THIS.box.AddObject('lin', 'Line')
        THIS.box.AddObject('cmg', 'CommandGroup')
ENDDEFINE

DEFINE CLASS Field AS TextBox
    Value = 'dentro'
    PROCEDURE Check
        * THISFORM is found from any depth
        ? THISFORM.Class, THIS.Parent.Name, THIS.Parent.Parent.Name
ENDDEFINE
";
    assert_eq!(
        run(src),
        "2 Page2 Page1 dentro 1\n3 .T. Option2 2\n1 4 O\nPANEL Page1 PGF\n1 500 https://fost.example\nhttps://example.com"
    );
    assert_eq!(run("f = CREATEOBJECT('Form')\nf.PageCount = 2").split('!').nth(1), Some("Línea 2: PageCount, ButtonCount y ColumnCount necesitan un número, en un PageFrame, un grupo o un Grid"));
    // every built-in class can be created, and the designer is told about all of them
    let classes = fost_lang::class_info();
    assert_eq!(classes.as_array().unwrap().len(), 24);
    for class in classes.as_array().unwrap() {
        assert_eq!(run(&format!("o = CREATEOBJECT('{}')\n? o.BaseClass", class["name"].as_str().unwrap())), class["name"].as_str().unwrap());
    }
}

#[test]
fn nested_controls_in_a_form_file() {
    let form = r#"{ "controls": [
      { "class": "PageFrame", "props": { "Name": "pgf" }, "controls": [
        { "class": "Page", "props": { "Name": "Page1", "Caption": "Datos" }, "controls": [
          { "class": "Container", "props": { "Name": "box" }, "controls": [
            { "class": "CommandButton", "props": { "Name": "cmd" }, "methods": { "Click": "THISFORM.Caption = THIS.Parent.Parent.Caption" } } ] } ] },
        { "class": "Page", "props": { "Name": "Page2" }, "methods": { "Activate": "? 'page 2'" } } ] },
      { "class": "OptionGroup", "props": { "Name": "opt" }, "controls": [
        { "class": "OptionButton", "props": { "Name": "Option1" } }, { "class": "OptionButton", "props": { "Name": "Option2" } } ],
        "methods": { "InteractiveChange": "? 'elegida', THIS.Value" } } ] }"#;
    let mut it = Interp::default();
    it.run_form(form, "anidado").unwrap();
    let forms = it.forms_json();
    let (frame, group) = (&forms[0]["children"][0], &forms[0]["children"][1]);
    let button = &frame["children"][0]["children"][0]["children"][0];
    assert_eq!(button["base"], "COMMANDBUTTON");

    // a click three levels down still reaches its handler, and the handler reaches the form
    it.ui_event(button["id"].as_u64().unwrap(), &serde_json::Value::Null, Some("Click")).unwrap();
    assert_eq!(it.forms_json()[0]["props"]["CAPTION"], "Datos");
    // what the UI sends when the user picks the second option, then the second tab
    it.ui_event(group["id"].as_u64().unwrap(), &serde_json::json!({ "Value": 2 }), Some("InteractiveChange")).unwrap();
    it.ui_event(frame["id"].as_u64().unwrap(), &serde_json::json!({ "ActivePage": 2 }), None).unwrap();
    it.ui_event(frame["children"][1]["id"].as_u64().unwrap(), &serde_json::Value::Null, Some("Activate")).unwrap();
    assert_eq!(it.take_output(), "\nelegida 2\npage 2");
    assert_eq!(it.forms_json()[0]["children"][0]["props"]["ACTIVEPAGE"], 2.0);
}

/// Runs a program whose tables live in a fresh folder, removed afterwards.
fn run_in_folder(tag: &str, src: &str) -> String {
    let dir = std::env::temp_dir().join(format!("fost-data-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut it = Interp::default();
    it.dir = dir.clone();
    let res = it.run(src);
    let out = it.take_output();
    drop(it);
    std::fs::remove_dir_all(dir).unwrap();
    let out = out.strip_prefix('\n').unwrap_or(&out).to_string();
    match res {
        Ok(()) => out,
        Err(e) => format!("{out}!{e}"),
    }
}

const PEOPLE: &str = "
CREATE CURSOR gente (nombre C(8), edad N(3), activo L, alta D, nota M)
INSERT INTO gente VALUES ('Carla', 30, .T., {^2024-03-01}, 'uno')
INSERT INTO gente (nombre, edad) VALUES ('Ana', 25)
INSERT INTO gente (nombre, edad) VALUES ('Beto', 41)
APPEND BLANK
REPLACE nombre WITH 'Dora', edad WITH 25
";

#[test]
fn tables_records_and_the_pointer() {
    let src = format!("{PEOPLE}
? RECCOUNT(), RECNO(), '[' + nombre + ']', LEN(nombre), ALLTRIM(nombre), edad + 1, activo, EMPTY(alta), ALIAS(), FCOUNT(), FIELD(2)
GO TOP
? nombre, activo, alta, nota, BOF(), EOF()
SKIP -1
? RECNO(), BOF()
GO BOTTOM
SKIP
? EOF(), RECNO(), '[' + nombre + ']', edad
SKIP -2
? nombre
GO 2
? nombre, gente.nombre, gente.edad
GO 9
");
    assert_eq!(
        run(&src),
        "4 4 [Dora    ] 8 Dora 26 .F. .T. GENTE 5 EDAD\nCarla    .T. 03/01/2024 uno .F. .F.\n1 .T.\n.T. 5 [        ] 0\nBeto    \nAna      Ana      25!Línea 21: El registro 9 está fuera de rango"
    );
    assert_eq!(run("? RECNO(), EOF(), USED(), ALIAS() + '|', SELECT()"), "0 .F. .F. | 1");
    assert_eq!(run("GO TOP"), "!Línea 1: No hay ninguna tabla abierta en esta área de trabajo");
    assert_eq!(run("USE nada"), "!Línea 1: Tabla no encontrada: NADA");
    assert_eq!(run(&format!("{PEOPLE}REPLACE edad WITH 'x'")), "!Línea 8: El valor no es del tipo del campo EDAD");
    assert_eq!(run(&format!("{PEOPLE}REPLACE nada WITH 1")), "!Línea 8: Campo no encontrado: NADA");
}

#[test]
fn tables_scopes_search_and_deletion() {
    let src = format!("{PEOPLE}
COUNT TO todos
COUNT FOR edad = 25 TO jovenes
SUM edad, edad * 2 TO suma, doble FOR edad > 25
? todos, jovenes, suma, doble, EOF()
LOCATE FOR edad = 25
? FOUND(), nombre
CONTINUE
? FOUND(), nombre
CONTINUE
? FOUND(), EOF()
REPLACE ALL edad WITH edad + 1 FOR edad < 30
GO 2
REPLACE edad WITH 99
? edad, RECNO()
SCAN FOR edad > 40
    ?? ALLTRIM(nombre) + ' '
ENDSCAN
? EOF()
* deleted records stay visible until SET DELETED ON, and stay in the table until PACK
GO 2
DELETE
? DELETED(), RECCOUNT()
SET DELETED ON
COUNT TO visibles
GO TOP
SKIP
? visibles, nombre, RECNO()
RECALL ALL
COUNT TO visibles
DELETE FOR edad = 99
PACK
? visibles, RECCOUNT(), RECNO(), nombre
GO BOTTOM
? RECNO(), nombre
ZAP
? RECCOUNT(), EOF()
");
    assert_eq!(
        run(&src),
        "4 2 71 142 .T.\n.T. Ana     \n.T. Dora    \n.F. .T.\n99 2Ana Beto \n.T.\n.T. 4\n3 Beto     3\n4 3 1 Carla   \n3 Dora    \n0 .T."
    );
}

#[test]
fn tables_orders_and_seek() {
    let src = format!("{PEOPLE}
INDEX ON UPPER(nombre) TAG nom
? nombre, RECNO()
SCAN
    ?? ALLTRIM(nombre) + ' '
ENDSCAN
INDEX ON edad TAG ed DESCENDING
GO TOP
? nombre
* a descending order runs the ascending one backwards, equal keys included; a new record finds its place
INSERT INTO gente (nombre, edad) VALUES ('Eva', 35)
GO TOP
SKIP
? nombre
SEEK 25
? FOUND(), nombre, SEEK(41), nombre, SEEK(7), EOF()
SET ORDER TO nom
SEEK 'BE'
? FOUND(), nombre
SET EXACT ON
? SEEK('BE'), SEEK('BETO')
SET ORDER TO
GO TOP
? nombre
SEEK 'x'
");
    assert_eq!(
        run(&src),
        "Ana      2Ana Beto Carla Dora \nBeto    \nEva     \n.T. Dora     .T. Beto     .F. .T.\n.T. Beto    \n.F. .T.\nCarla   !Línea 32: SEEK necesita un orden activo: use INDEX ON o SET ORDER TO"
    );
}

#[test]
fn tables_work_areas_and_names() {
    let src = "
CREATE CURSOR uno (id N(3), nombre C(5))
INSERT INTO uno VALUES (1, 'a')
CREATE CURSOR dos (id N(3), ref N(3))
INSERT INTO dos VALUES (7, 1)
? ALIAS(), SELECT(), SELECT('uno'), SELECT('nada'), USED('uno'), USED('nada')
nombre = 'variable'
id = 100
* a field hides a variable of the same name; m. reaches the variable
? id, m.id, nombre, uno.nombre, uno.id + dos.id
SELECT uno
? id, m.id, nombre, m.nombre
REPLACE dos.ref WITH 55
? dos.ref
USE IN dos
? USED('dos'), ALIAS()
USE
? USED(), nombre
CLOSE DATABASES ALL
? SELECT()
";
    assert_eq!(run(src), "DOS 2 1 0 .T. .F.\n7 100 variable a     8\n1 100 a     variable\n55\n.F. UNO\n.F. variable\n1");
    assert_eq!(
        run("CREATE CURSOR c (n N(2), t C(3))\nINSERT INTO c VALUES (1, 'a')\nINSERT INTO c VALUES (2, 'b')\nDELETE\nINDEX ON t TAG t COLLATE 'Machine'\nLIST\nLIST OFF FIELDS n * 2, t FOR n > 1"),
        "Registro  N  T\n      1   1  a  \n      2 * 2  b  \n4  b  "
    );
    // a cursor is gone once closed
    assert_eq!(run("CREATE CURSOR c (x N(1))\nUSE\nUSE c"), "!Línea 3: Tabla no encontrada: C");
}

#[test]
fn tables_on_disk_persist_and_cursors_leave_no_file() {
    let dir = std::env::temp_dir().join(format!("fost-disk-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("datos.fostdb");
    {
        let mut it = Interp::default();
        it.dir = dir.clone();
        it.run("CREATE CURSOR c (x N(1))\nUSE nada").unwrap_err();
        assert!(!file.exists(), "neither a cursor nor a failed USE creates the database");
        it.run("CREATE TABLE clientes (nombre C(10), saldo N(8,2))\nINSERT INTO clientes VALUES ('Ana', 10.555)\nAPPEND BLANK\nDELETE").unwrap();
        assert!(file.exists());
        assert_eq!(it.run("CREATE TABLE clientes (x N(1))").unwrap_err().msg, "La tabla CLIENTES ya existe");
    }
    // a new interpreter, as after restarting the app
    let mut it = Interp::default();
    it.dir = dir.clone();
    it.run("USE clientes ALIAS cli\n? ALIAS(), RECCOUNT(), nombre, saldo\nGO BOTTOM\n? DELETED()").unwrap();
    assert_eq!(it.take_output(), "\nCLI 2 Ana        10.56\n.T.");
    drop(it);
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(run_in_folder("sql", "SELECT * FROM x"), "!Línea 1: Tabla no encontrada: X");
}

#[test]
fn menus() {
    let mut it = Interp::default();
    // the shape of the code FoxPro's menu designer generates, plus a menu of its own
    let src = r#"
SET SYSMENU TO
bloqueado = .T.
DEFINE PAD archivo OF _MSYSMENU PROMPT "\<Archivo" COLOR SCHEME 3 KEY ALT+A, ""
DEFINE PAD salir OF _MSYSMENU PROMPT "Salir"
ON PAD archivo OF _MSYSMENU ACTIVATE POPUP parchivo
ON SELECTION PAD salir OF _MSYSMENU ? "adiós"

DEFINE POPUP parchivo MARGIN RELATIVE SHADOW COLOR SCHEME 4
DEFINE BAR 1 OF parchivo PROMPT "Nuevo" KEY CTRL+N, "Ctrl+N"
DEFINE BAR 2 OF parchivo PROMPT "\-"
DEFINE BAR 3 OF parchivo PROMPT "Guardar" SKIP FOR bloqueado MESSAGE "Guarda"
DEFINE BAR 4 OF parchivo PROMPT "Exportar"
ON SELECTION BAR 1 OF parchivo DO nuevo WITH "x"
ON SELECTION POPUP parchivo ? "elegido", BAR(), PROMPT(), POPUP(), PAD(), MENU()
ON BAR 4 OF parchivo ACTIVATE POPUP pexportar
DEFINE POPUP pexportar
DEFINE BAR 1 OF pexportar PROMPT "PDF"

DEFINE MENU otro BAR
DEFINE PAD uno OF otro PROMPT "Uno"
? BAR(), "[" + PROMPT() + "]"

PROCEDURE nuevo
    LPARAMETERS que
    ? "nuevo", que
ENDPROC
"#;
    it.run(src).unwrap();
    assert_eq!(it.take_output(), "\n0 []");
    let menus = it.menus_json();
    // a menu is not drawn until it is activated; the system one always is
    assert_eq!(menus.as_array().unwrap().len(), 1);
    assert_eq!(menus[0]["name"], "_MSYSMENU");
    let bars = &menus[0]["items"][0]["items"];
    assert_eq!(menus[0]["items"][0]["popup"], "PARCHIVO");
    assert_eq!(bars[2], serde_json::json!({ "key": "3", "prompt": "Guardar", "disabled": true, "popup": null, "items": null }));
    assert_eq!(bars[3]["items"][0]["prompt"], "PDF");

    it.menu_event("_MSYSMENU", "ARCHIVO", Some(("PARCHIVO", "1"))).unwrap();
    assert_eq!(it.take_output(), "\nnuevo x");
    // a bar without a command of its own runs the popup's
    it.menu_event("_MSYSMENU", "ARCHIVO", Some(("PARCHIVO", "3"))).unwrap();
    assert_eq!(it.take_output(), "\nelegido 3 Guardar PARCHIVO ARCHIVO _MSYSMENU");
    it.menu_event("_MSYSMENU", "SALIR", None).unwrap();
    assert_eq!(it.take_output(), "\nadiós");
    // nothing to run, and an item that is gone, are not errors
    it.menu_event("_MSYSMENU", "ARCHIVO", None).unwrap();
    it.menu_event("_MSYSMENU", "NADIE", None).unwrap();

    it.run("bloqueado = .F.\nACTIVATE MENU otro\nON SELECTION PAD uno OF otro ? 1 / 0").unwrap();
    let menus = it.menus_json();
    assert_eq!(menus[0]["items"][0]["items"][2]["disabled"], false);
    assert_eq!(menus[1]["name"], "OTRO");
    assert_eq!(it.menu_event("OTRO", "UNO", None).unwrap_err().to_string(), "ON SELECTION, línea 3: División entre cero");

    // defining again replaces instead of piling up
    it.run("DEFINE PAD salir OF _MSYSMENU PROMPT 'Cerrar'\nDEACTIVATE MENU otro").unwrap();
    let menus = it.menus_json();
    assert_eq!((menus.as_array().unwrap().len(), menus[0]["items"].as_array().unwrap().len()), (1, 2));
    assert_eq!(menus[0]["items"][1]["prompt"], "Cerrar");
    it.run("ACTIVATE MENU otro\nRELEASE MENUS otro\nRELEASE POPUPS\nSET SYSMENU TO DEFAULT").unwrap();
    assert_eq!(it.menus_json(), serde_json::json!([]));

    assert_eq!(it.run("DEFINE PAD x OF nada PROMPT 'x'").unwrap_err().msg, "Menú no definido: NADA");
    assert_eq!(it.run("ON SELECTION BAR 9 OF _MSYSMENU ? 1").unwrap_err().msg, "Desplegable no definido: _MSYSMENU");
    assert_eq!(it.run("DEFINE BAR 1 OF p PROMPT 'x' BEFORE y").unwrap_err().msg, "Cláusula no soportada en DEFINE PAD o DEFINE BAR");
    // classes still work next to menu definitions
    assert_eq!(run("DEFINE MENU m\no = CREATEOBJECT('C')\n? o.n\nDEFINE CLASS C AS Custom\nn = 5\nENDDEFINE"), "5");
}

#[test]
fn forms_bound_to_data() {
    use serde_json::json;
    let mut it = Interp::default();
    it.run(PEOPLE).unwrap();
    it.run("GO TOP\nPUBLIC titulo\ntitulo = 'hola'").unwrap();
    let control = |class: &str, name: &str, source: &str| json!({ "class": class, "props": { "Name": name, "ControlSource": source } });
    let form = json!({ "controls": [
        control("TextBox", "txtNombre", "gente.nombre"),
        control("TextBox", "txtEdad", "edad"),
        control("CheckBox", "chkActivo", "activo"),
        control("TextBox", "txtAlta", "alta"),
        control("TextBox", "txtTitulo", "titulo"),
        control("TextBox", "txtNada", "no.existe"),
        { "class": "Grid", "props": { "Name": "grd", "RecordSource": "gente" } },
        { "class": "CommandButton", "props": { "Name": "cmd" }, "methods": {
            "Click": "SKIP\n? THISFORM.txtNombre.Value\nTHISFORM.Refresh()\n? THISFORM.txtNombre.Value\nTHISFORM.txtEdad.Value = 26" } },
    ] });
    it.run_form(&form.to_string(), "datos").unwrap();
    let forms = it.forms_json();
    let kids = forms[0]["children"].as_array().unwrap();
    let value = |forms: &serde_json::Value, i: usize| forms[0]["children"][i]["props"]["VALUE"].clone();
    let id = |i: usize| kids[i]["id"].as_u64().unwrap();
    // each control shows its field of the current record; text comes without its padding
    assert_eq!((0..6).map(|i| value(&forms, i)).collect::<Vec<_>>(), [json!("Carla"), json!(30.0), json!(true), json!("03/01/2024"), json!("hola"), json!("")]);
    assert_eq!(kids[6]["grid"]["fields"], json!(["NOMBRE", "EDAD", "ACTIVO", "ALTA", "NOTA"]));
    assert_eq!(kids[6]["grid"]["rows"][1], json!({ "recno": 2, "cells": ["Ana", 25.0, false, "  /  /    ", ""] }));
    assert_eq!(kids[6]["grid"]["recno"], 1);

    // what the user enters goes to the field, converted to its type
    it.ui_event(id(0), &json!({ "Value": "Carlota" }), None).unwrap();
    it.ui_event(id(1), &json!({ "Value": 31 }), None).unwrap();
    it.ui_event(id(2), &json!({ "Value": false }), None).unwrap();
    it.ui_event(id(4), &json!({ "Value": "chao" }), None).unwrap();
    // half a date is not stored, and does not wipe what is being typed
    it.ui_event(id(3), &json!({ "Value": "04/1" }), None).unwrap();
    assert_eq!(value(&it.forms_json(), 3), "04/1");
    it.run("? nombre + '|', edad, activo, alta, titulo").unwrap();
    assert_eq!(it.take_output(), "\nCarlota | 31 .F. 03/01/2024 chao");
    it.ui_event(id(3), &json!({ "Value": "04/15/2024" }), None).unwrap();
    it.run("? alta").unwrap();
    assert_eq!(it.take_output(), "\n04/15/2024");

    // a click on a row of the grid moves the record pointer, and the controls follow
    it.ui_event(id(6), &json!({ "RecNo": 3 }), Some("AfterRowColChange")).unwrap();
    let forms = it.forms_json();
    assert_eq!((value(&forms, 0), value(&forms, 1), forms[0]["children"][6]["grid"]["recno"].clone()), (json!("Beto"), json!(41.0), json!(3)));

    // inside a method the control keeps its value until Refresh; assigning Value writes the field
    it.ui_event(id(7), &serde_json::Value::Null, Some("Click")).unwrap();
    assert_eq!(it.take_output(), "\nBeto\nDora");
    it.run("? edad\nGO 2").unwrap();
    assert_eq!(it.take_output(), "\n26");
    assert_eq!(value(&it.forms_json(), 0), "Ana");

    // with the table closed the controls keep what they had, and typing is not an error
    it.run("USE").unwrap();
    it.ui_event(id(0), &json!({ "Value": "x" }), None).unwrap();
    let forms = it.forms_json();
    assert_eq!((value(&forms, 0), forms[0]["children"][6]["grid"]["rows"].clone()), (json!("x"), json!([])));
}

#[test]
fn a_project_lists_what_its_folder_holds() {
    let dir = std::env::temp_dir().join(format!("fost-project-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // an empty folder is an empty project, and looking at it creates nothing
    let empty = fost_lang::project(&dir);
    assert_eq!(empty, serde_json::json!({ "file": null, "main": null, "programs": [], "forms": [], "classes": [], "tables": [], "assets": [] }));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);

    std::fs::write(dir.join("Main.fost"), "? 1\nDEFINE CLASS Ficha AS Form\nENDDEFINE").unwrap();
    std::fs::write(dir.join("alta.fostform"), "{}").unwrap();
    std::fs::write(dir.join("notas.txt"), "").unwrap();
    std::fs::write(dir.join("tienda.fostproj"), r#"{ "main": "Main.fost" }"#).unwrap();
    std::fs::create_dir_all(dir.join("assets/sub")).unwrap();
    std::fs::write(dir.join("assets/logo.png"), "").unwrap();
    let mut it = Interp::default();
    it.dir = dir.clone();
    it.run("CREATE TABLE clientes (nombre C(20), saldo N(10, 2))\nCREATE CURSOR temporal (x L)").unwrap();
    // read while the interpreter still has the database open, as the IDE does
    let project = fost_lang::project(&dir);
    drop(it);
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(project["programs"], serde_json::json!(["Main.fost"]));
    assert_eq!((&project["file"], &project["main"]), (&serde_json::json!("tienda.fostproj"), &serde_json::json!("Main.fost")));
    // folders inside assets are not assets
    assert_eq!(project["assets"], serde_json::json!(["assets/logo.png"]));
    assert_eq!(project["forms"], serde_json::json!(["alta.fostform"]));
    assert_eq!(project["classes"], serde_json::json!([{ "name": "Ficha", "parent": "Form", "file": "Main.fost", "line": 2 }]));
    // cursors live in memory: they are not part of the project
    assert_eq!(project["tables"].as_array().unwrap().len(), 1);
    assert_eq!(project["tables"][0]["name"], "CLIENTES");
    assert_eq!(project["tables"][0]["fields"][1], serde_json::json!({ "name": "SALDO", "kind": "N", "width": 10, "decimals": 2 }));
}

#[test]
fn set_procedure_shares_routines_between_programs() {
    let dir = std::env::temp_dir().join(format!("fost-setproc-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("Utilidades.fost"), "? 'no se ejecuta'\nFUNCTION doble(n)\nRETURN n * 2\nDEFINE CLASS Caja AS Custom\nlado = 3\nENDDEFINE").unwrap();
    std::fs::write(dir.join("textos.fost"), "PROCEDURE saluda\n? 'hola'\nENDPROC\nPROCEDURE falla\n? 1 / 0").unwrap();
    let mut it = Interp::default();
    it.dir = dir.clone();
    let mut run = |src: &str| {
        let res = it.run(src);
        let out = it.take_output();
        res.map(|_| out).map_err(|e| e.to_string())
    };
    // the file's own code does not run; its functions and classes are there afterwards
    assert_eq!(run("SET PROCEDURE TO utilidades\n? doble(21)\no = CREATEOBJECT('Caja')\n? o.lado").unwrap(), "\n42\n3");
    // several files, quoted with path and extension as old programs write them, and ADDITIVE
    assert_eq!(run("SET PROC TO \"libs\\TEXTOS.PRG\", utilidades.prg ADDITIVE\nDO saluda\n? doble(1)").unwrap(), "\nhola\n2");
    assert_eq!(run("SET PROCEDURE TO\n? doble(2)").unwrap(), "\n4");
    assert_eq!(run("DO falla").unwrap_err(), "textos.fost, línea 5: División entre cero");
    assert_eq!(run("SET PROCEDURE TO nadie").unwrap_err(), "Línea 1: Procedimiento o programa no encontrado: NADIE");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn tables_filter_and_sql() {
    let src = format!("{PEOPLE}
SET FILTER TO edad < 40
COUNT TO n
GO TOP
? n, TRIM(nombre)
SKIP 5
? EOF()
SET FILTER TO
COUNT TO n
? n
SELECT nombre, edad * 2 AS doble FROM gente WHERE edad > 25 ORDER BY nombre DESC INTO CURSOR mayores
? ALIAS(), RECCOUNT(), _TALLY, TRIM(nombre), doble
SKIP
? TRIM(nombre), doble
SELECT edad, COUNT(*), SUM(edad) AS total, MAX(nombre) FROM gente GROUP BY edad ORDER BY 1 INTO CURSOR grupos
SCAN
    ? edad, cnt, total, TRIM(max_nombre)
ENDSCAN
SELECT COUNT(*), AVG(edad) FROM gente WHERE edad > 100 INTO ARRAY a
? a[1], ISNULL(a[2])
SELECT DISTINCT edad FROM gente ORDER BY edad INTO ARRAY e
? ALEN(e), e[1], e[3]
SELECT TOP 1 * FROM gente g WHERE g.activo INTO CURSOR uno
? TRIM(nombre), FCOUNT(), alta
UPDATE gente SET edad = edad + 1, nota = 'x' WHERE nombre = 'Ana'
? _TALLY
DELETE FROM gente WHERE edad = 25
SELECT gente
GO 2
? edad, nota, DELETED(), RECNO()
GO 4
? DELETED()
");
    assert_eq!(
        run(&src),
        "3 Carla\n.T.\n4\nMAYORES 2 2 Carla 60\nBeto 82\n25 2 50 Dora\n30 1 30 Carla\n41 1 41 Beto\n0 .T.\n3 25 41\nCarla 5 03/01/2024\n1\n26 x .F. 2\n.T."
    );
    assert_eq!(run(&format!("{PEOPLE}\nSELECT * FROM gente, otra")).split('!').nth(1), Some("Línea 9: SELECT admite una sola tabla, con WHERE, GROUP BY, ORDER BY e INTO"));
    assert_eq!(run(&format!("{PEOPLE}\nSELECT nombre FROM gente ORDER BY edad INTO CURSOR c")).split('!').nth(1), Some("Línea 9: ORDER BY debe nombrar una columna del resultado"));
}

#[test]
fn tables_buffering_and_kept_orders() {
    let src = format!("{PEOPLE}
= CURSORSETPROP('Buffering', 5)
GO 1
REPLACE edad WITH 99
DELETE
APPEND BLANK
REPLACE nombre WITH 'Nuevo'
? RECCOUNT(), TABLEREVERT(.T.)
GO 1
? RECCOUNT(), edad, DELETED()
REPLACE edad WITH 31
? TABLEUPDATE(.T.), TABLEREVERT(.T.), edad
");
    assert_eq!(run(&src), "5 2\n4 30 .F.\n.T. 0 31");

    // an order of a table of the folder is still there after the program that made it is gone
    let dir = std::env::temp_dir().join(format!("fost-orders-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let run_here = |src: &str| {
        let mut it = Interp::default();
        it.dir = dir.clone();
        let res = it.run(src);
        res.map(|_| it.take_output()).map_err(|e| e.to_string())
    };
    run_here("CREATE TABLE t (nombre C(5), n I)\nINSERT INTO t VALUES ('b', 1)\nINSERT INTO t VALUES ('a', 2)\nINDEX ON UPPER(t.nombre) + 'x' TAG pornombre DESCENDING\nINDEX ON n TAG porn").unwrap();
    assert_eq!(run_here("USE t ORDER pornombre\n? nombre\nSET ORDER TO porn\nGO TOP\n? nombre").unwrap(), "\nb    \nb    ");
    assert_eq!(run_here("USE t\nSET ORDER TO nadie").unwrap_err(), "Línea 2: Orden no encontrado: NADIE");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn browse_lists_and_grid_columns() {
    use serde_json::json;
    let mut it = Interp::default();
    it.run(PEOPLE).unwrap();
    it.run("GO TOP\nBROWSE").unwrap();
    let forms = it.forms_json();
    assert_eq!(forms[0]["props"]["CAPTION"], "GENTE");
    let grid = &forms[0]["children"][0];
    assert_eq!(grid["grid"]["titles"], json!(["NOMBRE", "EDAD", "ACTIVO", "ALTA", "NOTA"]));
    let id = grid["id"].as_u64().unwrap();

    // a cell edited in the grid goes to that record's field, as typed text if need be
    it.ui_event(id, &json!({ "RecNo": 2, "Field": "EDAD", "Value": "52" }), Some("AfterRowColChange")).unwrap();
    it.run("? RECNO(), edad").unwrap();
    assert_eq!(it.take_output(), "\n2 52");

    // columns choose what is shown and under which title; a read-only grid ignores edits
    it.run("f = CREATEOBJECT('Form')\nf.AddObject('g', 'Grid')\nf.g.ColumnCount = 2\nf.g.Column2.ControlSource = 'gente.edad'\nf.g.Column2.Header1.Caption = 'Años'\nf.g.Column1.Width = 40\nf.g.ReadOnly = .T.\nf.g.Visible = .T.\nf.Show()\n? f.g.ColumnCount").unwrap();
    assert_eq!(it.take_output(), "\n2");
    let forms = it.forms_json();
    let g = &forms[1]["children"][0];
    assert_eq!((&g["grid"]["titles"], &g["grid"]["fields"], &g["grid"]["widths"]), (&json!(["NOMBRE", "Años"]), &json!(["NOMBRE", "EDAD"]), &json!([40.0, 75.0])));
    assert_eq!(g["grid"]["rows"][1]["cells"], json!(["Ana", 52.0]));
    it.ui_event(g["id"].as_u64().unwrap(), &json!({ "RecNo": 1, "Field": "EDAD", "Value": 1 }), None).unwrap();
    it.run("? RECNO(), edad").unwrap();
    assert_eq!(it.take_output(), "\n1 30");

    // a list filled from a table follows its filter
    it.run("f.AddObject('c', 'ComboBox')\nf.c.RowSourceType = 6\nf.c.RowSource = 'gente.nombre'\nSET FILTER TO edad > 29\n? f.c.ListCount, f.c.List(2)\nf.c.RowSourceType = 2\nf.c.RowSource = 'gente'\n? f.c.List(1)").unwrap();
    assert_eq!(it.take_output(), "\n3 Ana\nCarla");
    assert_eq!(it.forms_json()[0]["children"][0]["grid"]["rows"].as_array().unwrap().len(), 3);
}

#[test]
fn try_catch_and_on_error() {
    let src = "
TRY
    ? 'antes'
    ? 1 / 0
    ? 'no llega'
CATCH TO e
    ? e.Message, e.LineNo, MESSAGE(), ERROR()
FINALLY
    ? 'siempre'
ENDTRY
TRY
    DO falla
CATCH TO e
    ? e.Message, e.UserValue, e.ErrorNo
ENDTRY
TRY
    ? 'sin error'
CATCH
    ? 'no'
ENDTRY
? resultado()
ON ERROR ? 'atendido:', MESSAGE(), LINENO()
? nadie
? 'sigue'
TRY
    ? nadie
CATCH
    ? 'el TRY gana'
ENDTRY
ON ERROR
? nadie
PROCEDURE falla
    THROW 'propio'
FUNCTION resultado
    TRY
        RETURN 'del try'
    FINALLY
        ? 'finally'
    ENDTRY
";
    assert_eq!(
        run(src),
        "antes\nDivisión entre cero 4 División entre cero 1\nsiempre\npropio propio 2071\nsin error\nfinally\ndel try\natendido: Variable no encontrada: NADIE 23\nsigue\nel TRY gana!Línea 31: Variable no encontrada: NADIE"
    );
    assert_eq!(run("TRY\n? 1"), "!Línea 1: TRY sin cerrar");
    assert_eq!(run("CATCH"), "!Línea 1: CATCH sin su bloque de apertura");
}

#[test]
fn macros_text_and_transform() {
    let src = "
campo = 'edad'
valor = '40 + 2'
cmd = '? \"macro\",'
&cmd &valor
n&campo. = &valor
? nedad
orden = 'FOR i = 1 TO 2'
TEXT TO t TEXTMERGE NOSHOW
Hola <<UPPER('ana')>>,
  tienes <<nedad>> años.
ENDTEXT
? t
TEXT TO t ADDITIVE
 Fin & sin macro
ENDTEXT
? t
text = 5
? text
TEXT
suelto
ENDT
? 'linea', 23
? &nadie
";
    assert_eq!(
        run(src),
        "macro 42\n42\nHola ANA,\n  tienes 42 años.\nHola ANA,\n  tienes 42 años. Fin & sin macro\n5\nsuelto\nlinea 23!Línea 24: Variable no encontrada: NADIE"
    );
    assert_eq!(run("x = 5\n? &x"), "!Línea 2: La variable de la macro &X debe contener texto");
    assert_eq!(run("TEXT\nabc"), "!Línea 1: TEXT sin cerrar");
    assert_eq!(
        run("? TRANSFORM(1234567.891, '999,999,999.99') + '|', TRANSFORM(-5, '999') + '|', TRANSFORM(12345, '999'), TRANSFORM(42, '@L 99999'), TRANSFORM(0, '@Z 999') + '|', TRANSFORM(9.5, '$999.99')"),
        "  1,234,567.89|  -5| *** 00042    |   $9.50"
    );
    assert_eq!(run("? TRANSFORM('abc', '@!'), TRANSFORM('ab', '!X'), TRANSFORM('5551234', '@R 999-9999'), TRANSFORM(.T.), TRANSFORM(3)"), "ABC Ab 555-1234 .T. 3");
}
