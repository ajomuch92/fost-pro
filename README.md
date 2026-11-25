# Fost Pro

Un IDE y un intérprete para un lenguaje con la sintaxis de Visual FoxPro. El objetivo es que quien tenga
programas viejos en FoxPro pueda migrarlos aquí: los archivos tienen formato y extensión propios, pero el
código se escribe igual.

Hecho con [Tauri](https://tauri.app) (Rust), Vue 3, TypeScript y Tailwind CSS.

## Estado

El proyecto está en desarrollo. Hoy se puede escribir y ejecutar programas, diseñar y ejecutar
formularios enlazados a tablas, y trabajar con ellas mediante los comandos de registros de FoxPro. Falta
el SQL.

| Parte | Estado |
| --- | --- |
| Lenguaje: variables, expresiones, control de flujo, procedimientos y funciones | Hecho |
| Ventana de comandos | Hecho |
| Editor de programas (`.fost`) | Hecho |
| Objetos y clases (`DEFINE CLASS`, `THIS`, `CREATEOBJECT`) | Hecho |
| Formularios en ejecución (`DO FORM`) | Hecho, con las propiedades básicas |
| Diseñador visual de formularios (`.fostform`) | Hecho, primera versión |
| Ayuda integrada (F1 o `HELP`) | Hecho |
| Menús (`DEFINE MENU`, `DEFINE POPUP`, `_MSYSMENU`) | Hecho, primera versión |
| Tablas: `USE`, `REPLACE`, `SCAN`, `SEEK`, índices, áreas de trabajo | Hecho, primera entrega |
| Formularios enlazados a datos (`ControlSource`, `Grid`) | Hecho, primera versión |
| SQL (`SELECT`, `UPDATE`, `DELETE FROM`), `BROWSE` | Pendiente |
| Proyectos: una carpeta, con panel de programas, formularios, clases y tablas | Hecho, primera versión |
| Migrador desde FoxPro | Pendiente |
| Reportes, ejecutables | Sin planear todavía |

### Qué funciona

- **Sintaxis de FoxPro:** comentarios `*` y `&&`, continuación con `;`, palabras clave y funciones
  abreviadas a cuatro letras, prefijo `m.`, cadenas con `'`, `"` y `[]`, fechas `{^2024-01-31}`.
- **Sentencias:** `?`, asignación, `STORE`, `IF`, `DO CASE`, `FOR`, `FOR EACH`, `DO WHILE`, `LOOP`, `EXIT`,
  `PROCEDURE`, `FUNCTION`, `DO ... WITH`, `RETURN`, `PARAMETERS`, `LPARAMETERS`, `LOCAL`, `PRIVATE`,
  `PUBLIC`, `DIMENSION`, `RELEASE`, `CLEAR`, `WITH`, `#DEFINE`, `SET EXACT`, `SET DECIMALS`,
  `SET PROCEDURE TO` para usar las funciones y clases de otro programa.
- **Semántica de FoxPro:** variables privadas visibles en las rutinas llamadas, paso por referencia,
  comparación de cadenas con `=` según `SET EXACT`, y 85 funciones integradas.
- **Tablas:** `CREATE TABLE`, `CREATE CURSOR`, `USE`, `SELECT` de área, `APPEND BLANK`, `INSERT INTO`,
  `REPLACE`, `DELETE`, `RECALL`, `PACK`, `ZAP`, `GO`, `SKIP`, `LOCATE`, `CONTINUE`, `SCAN`, `INDEX ON`,
  `SET ORDER`, `SEEK`, `SET DELETED`, `COUNT`, `SUM` y `LIST`, con los alcances `ALL`, `NEXT`, `REST`, `FOR` y
  `WHILE`. Se comportan como en FoxPro: número de registro, marca de borrado, puntero con `EOF()` y
  `BOF()`, campos de texto rellenos hasta su ancho, y un campo gana a una variable del mismo nombre
  (`m.nombre` llega a la variable).
- **Objetos:** clases con herencia, `ADD OBJECT`, `THIS`, `THISFORM`, `DODEFAULT()`.
- **Formularios:** 20 controles, con los eventos `Load`, `Init`, `Click`, `InteractiveChange`, `GotFocus`,
  `LostFocus`, `Timer`, `Activate` y `Destroy`. `MESSAGEBOX()` e `INPUTBOX()` para dialogar con el usuario.
  - Texto y entrada: `Label`, `TextBox`, `EditBox`, `Spinner`, `CheckBox`, `ComboBox`, `ListBox`.
  - Botones y grupos: `CommandButton`, `CommandGroup`, `OptionGroup` con sus `OptionButton`.
  - Dibujo: `Image`, `Line`, `Shape`.
  - Contenedores: `Container`, y `PageFrame` con sus `Page` (pestañas). Se pueden anidar.
  - Otros: `Timer`, y `HyperLink`, que se dibuja como un enlace y abre su `Url` en el navegador.
  - Datos: `ControlSource` enlaza un `TextBox`, `EditBox`, `Spinner`, `CheckBox`, `ComboBox`, `ListBox` u
    `OptionGroup` a un campo (`cliente.nombre`) o a una variable pública: muestra el registro actual y
    guarda lo que el usuario escribe. `Grid` muestra la tabla de su `RecordSource`, y un clic en una fila
    la hace el registro actual.
- **Menús:** `DEFINE MENU`, `DEFINE PAD`, `DEFINE POPUP`, `DEFINE BAR`, `ON PAD`, `ON BAR`, `ON SELECTION`,
  `ACTIVATE MENU`, `DEACTIVATE MENU`, `RELEASE MENUS`, `RELEASE POPUPS` y `SET SYSMENU TO`, con submenús,
  separadores (`\-`), letra subrayada (`\<`), `SKIP FOR` y las funciones `BAR()`, `PROMPT()`, `POPUP()`,
  `PAD()` y `MENU()`. Los menús se dibujan en una barra bajo la de herramientas; lo definido en
  `_MSYSMENU` aparece sin activarlo, como el código que genera el diseñador de menús de FoxPro.
- **Proyectos:** un proyecto es una carpeta. El botón Proyecto la abre (o la crea desde el diálogo) y un
  panel lateral lista sus programas, formularios, clases (`DEFINE CLASS`, con un clic va a su línea) y
  tablas con sus campos. `DO`, `DO FORM` y las tablas trabajan en esa carpeta.
- **IDE:** editor con resaltado, diseñador de formularios (arrastrar, redimensionar, propiedades, código
  por evento; los controles se arrastran dentro y fuera de contenedores y pestañas), ventana de comandos
  con historial, y
  `MODIFY COMMAND`, `MODIFY FORM` y `CREATE FORM`.

La lista completa de comandos, funciones, clases y propiedades está en la ayuda de la aplicación.

### Diferencias conocidas con FoxPro

- Los números se muestran sin relleno de columnas y sin ceros finales; las fechas, siempre como
  MM/DD/AAAA.
- `SET` solo acepta `EXACT`, `DECIMALS` y las opciones de interfaz (`TALK`, `SAFETY`...). Las demás dan
  error en lugar de ignorarse.
- No existen la sustitución de macros (`&variable`), `TRY...CATCH`, `TEXT...ENDTEXT`, `READ EVENTS` ni
  `TRANSFORM` con máscara.
- Los nombres definidos en código se guardan en mayúsculas (`ADD OBJECT cmdOk` se llama `CMDOK`).
- Un formulario sigue abierto hasta `Release()` o cerrarlo, aunque ninguna variable lo referencie.
- Los controles solo tienen las propiedades básicas: posición, tamaño, texto, colores y fuente. Asignar
  una que no existe da error.
- Los órdenes de `INDEX ON` duran mientras la aplicación está abierta: al reiniciar hay que crearlos de
  nuevo. Los datos sí se conservan.
- En las tablas faltan el `SELECT` de SQL, `BROWSE`, `SET FILTER`, `SET RELATION`, los campos de fecha y
  hora, y las reglas de columna (`DEFAULT`, `PRIMARY KEY`).
- Los controles enlazados se actualizan solos cuando termina cada evento, sin llamar a `Refresh()`.
  Dentro de un método sí hace falta, como en FoxPro.
- El `Grid` es de solo lectura, muestra todos los campos (no hay `Column` ni `ColumnCount`) y como máximo
  1000 registros. No hay entorno de datos: las tablas se abren con `USE` en el evento `Load`. `RowSource`
  de un `ComboBox` aún no acepta tablas, y no hay `TABLEUPDATE()` ni buffering.
- `HyperLink` es un control visible, no el objeto invisible de FoxPro; `NavigateTo()` sí funciona igual.
- `ACTIVATE MENU` no detiene el programa a esperar una elección: el menú queda en pantalla y el programa
  sigue. Un bucle que lo active una y otra vez no termina.
- En los menús no hay atajos de teclado (`KEY` se acepta y se ignora), `BEFORE`/`AFTER`, barras del sistema
  (`_MED_COPY`...), `ACTIVATE POPUP` suelto como menú contextual ni diseñador visual de menús.
- Faltan `Collection` y los controles ActiveX, que no se podrán portar.

## Cómo ejecutarlo

Requisitos: [Rust](https://rustup.rs), Node.js, [pnpm](https://pnpm.io) y las
[dependencias de Tauri](https://tauri.app/start/prerequisites/) de tu sistema.

```bash
pnpm install
pnpm tauri dev
```

La primera compilación de Rust tarda unos minutos.

El intérprete también funciona solo, desde [src-tauri/](src-tauri/):

```bash
cargo run -p fost-lang -- programa.fost   # ejecuta un programa
cargo run -p fost-lang                    # lee comandos por la entrada estándar
```

## Un ejemplo

```foxpro
total = 0
FOR i = 1 TO 10
    IF i % 2 = 0
        LOOP
    ENDIF
    total = total + i
ENDFOR
? "Suma de impares:", total

nombre = INPUTBOX("¿Cómo te llamas?")
= MESSAGEBOX("Hola " + nombre)
```

## Organización del código

| Ruta | Contenido |
| --- | --- |
| [src-tauri/lang/](src-tauri/lang/) | El lenguaje: lexer, parser, intérprete, objetos, funciones y la referencia de la ayuda. No depende de Tauri. |
| [src-tauri/src/lib.rs](src-tauri/src/lib.rs) | Puente entre la interfaz y el intérprete, que corre en su propio hilo. |
| [src/engine.ts](src/engine.ts) | El único punto de la interfaz que habla con el intérprete, y el estado que comparten los paneles. |
| [src/components/](src/components/) | Editor, ventana de comandos, diseñador, formularios en ejecución, ayuda. |

## Pruebas

```bash
cd src-tauri
cargo test --workspace     # pruebas del lenguaje y del puente
./lang/corpus.sh           # ejemplos públicos de Visual FoxPro
```

`corpus.sh` descarga las soluciones en Visual FoxPro de
[Rosetta Code](https://rosettacode.org) a una carpeta que git ignora, las ejecuta y dice cuántas terminan
sin error y por qué fallan las demás. Hoy pasan 7 de 34.

## Formato de los archivos

- **`.fost`**: un programa, en texto plano.
- **`datos.fostdb`**: las tablas de una carpeta, en una base de datos SQLite. Se crea con el primer
  `CREATE TABLE`. Los cursores (`CREATE CURSOR`) viven en memoria y no dejan archivo.
- **`.fostform`**: un formulario, en JSON. Cada objeto tiene `class`, `props`, `methods` (el código de cada
  evento) y `controls`. Solo se guardan las propiedades que difieren del valor por defecto de la clase.

## Lo que sigue

1. Más base de datos: `SELECT` y demás SQL, `BROWSE`, órdenes que se conserven, filtros y relaciones.
2. Más enlace a datos: `Grid` editable con columnas, entorno de datos, listas llenas desde tablas.
3. El migrador que convierte un proyecto de FoxPro (`.prg`, `.dbf`, `.scx`) e informa de lo
   que no pudo convertir.
