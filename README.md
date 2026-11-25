# Fost Pro

Un IDE y un intérprete para un lenguaje con la sintaxis de Visual FoxPro. El objetivo es que quien tenga
programas viejos en FoxPro pueda migrarlos aquí: los archivos tienen formato y extensión propios, pero el
código se escribe igual.

Hecho con [Tauri](https://tauri.app) (Rust), Vue 3, TypeScript y Tailwind CSS.

## Estado

El proyecto está en desarrollo. Hoy se puede escribir y ejecutar programas, y diseñar y ejecutar
formularios. No hay base de datos todavía.

| Parte | Estado |
| --- | --- |
| Lenguaje: variables, expresiones, control de flujo, procedimientos y funciones | Hecho |
| Ventana de comandos | Hecho |
| Editor de programas (`.fost`) | Hecho |
| Objetos y clases (`DEFINE CLASS`, `THIS`, `CREATEOBJECT`) | Hecho |
| Formularios en ejecución (`DO FORM`) | Hecho, con las propiedades básicas |
| Diseñador visual de formularios (`.fostform`) | Hecho, primera versión |
| Ayuda integrada (F1 o `HELP`) | Hecho |
| Base de datos (`USE`, `SCAN`, `REPLACE`, SQL) | Pendiente |
| Proyectos y migrador desde FoxPro | Pendiente |
| Reportes, ejecutables | Sin planear todavía |

### Qué funciona

- **Sintaxis de FoxPro:** comentarios `*` y `&&`, continuación con `;`, palabras clave y funciones
  abreviadas a cuatro letras, prefijo `m.`, cadenas con `'`, `"` y `[]`, fechas `{^2024-01-31}`.
- **Sentencias:** `?`, asignación, `STORE`, `IF`, `DO CASE`, `FOR`, `FOR EACH`, `DO WHILE`, `LOOP`, `EXIT`,
  `PROCEDURE`, `FUNCTION`, `DO ... WITH`, `RETURN`, `PARAMETERS`, `LPARAMETERS`, `LOCAL`, `PRIVATE`,
  `PUBLIC`, `DIMENSION`, `RELEASE`, `CLEAR`, `WITH`, `#DEFINE`, `SET EXACT`, `SET DECIMALS`.
- **Semántica de FoxPro:** variables privadas visibles en las rutinas llamadas, paso por referencia,
  comparación de cadenas con `=` según `SET EXACT`, y 73 funciones integradas.
- **Objetos:** clases con herencia, `ADD OBJECT`, `THIS`, `THISFORM`, `DODEFAULT()`.
- **Formularios:** `Form`, `Label`, `TextBox`, `EditBox`, `CommandButton`, `CheckBox`, `ComboBox` y
  `ListBox`, con los eventos `Load`, `Init`, `Click`, `InteractiveChange`, `GotFocus`, `LostFocus` y
  `Destroy`. `MESSAGEBOX()` e `INPUTBOX()` para dialogar con el usuario.
- **IDE:** editor con resaltado, diseñador de formularios (arrastrar, redimensionar, propiedades, código
  por evento), ventana de comandos con historial, y `MODIFY COMMAND`, `MODIFY FORM` y `CREATE FORM`.

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
- Sin base de datos no hay `ControlSource` hacia tablas ni `Grid`.

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
sin error y por qué fallan las demás. Hoy pasan 6 de 34; casi todas las que fallan necesitan la base de
datos.

## Formato de los archivos

- **`.fost`**: un programa, en texto plano.
- **`.fostform`**: un formulario, en JSON. Cada objeto tiene `class`, `props`, `methods` (el código de cada
  evento) y `controls`. Solo se guardan las propiedades que difieren del valor por defecto de la clase.

## Lo que sigue

1. Base de datos: tablas, índices y los comandos de datos de FoxPro.
2. Enlace de los formularios a los datos: `ControlSource`, `Grid`, entorno de datos.
3. Proyectos, y el migrador que convierte un proyecto de FoxPro (`.prg`, `.dbf`, `.scx`) e informa de lo
   que no pudo convertir.
