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
    assert_eq!(run("BROWSE"), "!Línea 1: Comando no reconocido: BROWSE");
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
