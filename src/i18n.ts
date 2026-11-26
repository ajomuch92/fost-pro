// The IDE's two languages. Text is written in Spanish in the code and looked up here for English,
// so a text nobody translated shows in Spanish instead of as a key.
import { ref, watch } from "vue";

export type Lang = "en" | "es";

function stored(): Lang {
  try {
    return localStorage.getItem("fost-lang") === "es" ? "es" : "en";
  } catch {
    return "en";
  }
}
/** The language of the IDE. English until the user chooses otherwise. */
export const lang = ref<Lang>(stored());
watch(lang, (l) => {
  try {
    localStorage.setItem("fost-lang", l);
  } catch {
    // not remembered, nothing else is lost
  }
});

const en: Record<string, string> = {
  // toolbar and documents
  "Nuevo programa": "New program",
  "Nuevo formulario": "New form",
  Proyecto: "Project",
  "Abrir o crear la carpeta de un proyecto": "Open or create a project's folder",
  Abrir: "Open",
  Guardar: "Save",
  Editar: "Edit",
  Ver: "View",
  Menú: "Menu",
  "Diseñar un menú y escribir su código en el programa": "Design a menu and write its code into the program",
  "Ejecutar (F5)": "Run (F5)",
  "Ayuda (F1)": "Help (F1)",
  "Editor de programa": "Program editor",
  "sin título": "untitled",
  " (sin guardar)": " (unsaved)",
  "Programa Fost": "Fost program",
  "Formulario Fost": "Fost form",
  "Programas y formularios Fost": "Fost programs and forms",
  "Hay cambios sin guardar. ¿Descartarlos?": "There are unsaved changes. Discard them?",
  "Carpeta del proyecto": "Project folder",
  // Edit and View menus
  "Deshacer (Ctrl+Z)": "Undo (Ctrl+Z)",
  "Rehacer (Ctrl+Y)": "Redo (Ctrl+Y)",
  "Cortar (Ctrl+X)": "Cut (Ctrl+X)",
  "Copiar (Ctrl+C)": "Copy (Ctrl+C)",
  "Pegar (Ctrl+V)": "Paste (Ctrl+V)",
  "Seleccionar todo (Ctrl+A)": "Select all (Ctrl+A)",
  "Buscar y reemplazar (Ctrl+F)": "Find and replace (Ctrl+F)",
  "Panel del proyecto": "Project panel",
  "Ventana de comandos": "Command window",
  Controles: "Controls",
  Propiedades: "Properties",
  Código: "Code",
  Ayuda: "Help",
  "Tema oscuro": "Dark theme",
  "{0}: arrastra para ajustar; doble clic para cerrar": "{0}: drag to resize; double click to close",
  "{0}: arrastra para ajustar; doble clic para abrir": "{0}: drag to resize; double click to open",
  // command window
  'Escribe un comando y pulsa Enter. Por ejemplo: ? "Hola", 2 + 2': 'Type a command and press Enter. For example: ? "Hello", 2 + 2',
  Comando: "Command",
  "Enter ejecuta · Shift+Enter nueva línea · Esc detiene": "Enter runs · Shift+Enter new line · Esc stops",
  Detener: "Stop",
  // help
  "Ayuda de Fost Pro": "Fost Pro help",
  "Buscar un comando, función o propiedad": "Search for a command, function or property",
  "Buscar en la ayuda": "Search the help",
  "Cerrar la ayuda": "Close the help",
  'Nada coincide con "{0}".': 'Nothing matches "{0}".',
  Comandos: "Commands",
  "Funciones: {0}": "Functions: {0}",
  Objetos: "Objects",
  Clases: "Classes",
  "Propiedades: {0}.": "Properties: {0}.",
  "Eventos: {0}.": "Events: {0}.",
  // dialogs and running forms
  Cancelar: "Cancel",
  Aceptar: "OK",
  Cerrar: "Close",
  Restaurar: "Restore",
  Minimizar: "Minimize",
  Maximizar: "Maximize",
  "Menú {0}": "Menu {0}",
  // form designer
  "Usa letras, números y guion bajo, sin empezar por un número.": "Use letters, digits and underscores, not starting with a digit.",
  "Ya hay un objeto con ese nombre.": "There is already an object with that name.",
  "Agregar control": "Add control",
  "Agregar a {0}": "Add to {0}",
  "Selecciona antes el grupo o el marco de páginas que lo contendrá": "Select first the group or page frame that will hold it",
  "{0}: flechas para mover, Mayús+flechas para cambiar el tamaño, Supr para eliminar": "{0}: arrows to move, Shift+arrows to resize, Del to delete",
  "Cambiar el tamaño del formulario": "Resize the form",
  "Objeto seleccionado": "Selected object",
  Eliminar: "Delete",
  "Código de {0}": "Code of {0}",
  "Dentro de": "Inside",
  // project panel
  Actualizar: "Refresh",
  "Cerrar proyecto": "Close project",
  "Ejecutar {0}": "Run {0}",
  "Ejecutar el proyecto: marca antes un programa principal": "Run the project: mark a main program first",
  "Marca un programa principal con la estrella": "Mark a main program with the star",
  "Programas ({0})": "Programs ({0})",
  "Formularios ({0})": "Forms ({0})",
  "Clases ({0})": "Classes ({0})",
  "Datos ({0})": "Data ({0})",
  "Recursos ({0})": "Assets ({0})",
  "{0}: programa principal": "{0}: main program",
  "Programa principal": "Main program",
  "{0}, línea {1}": "{0}, line {1}",
  "Agregar...": "Add...",
  "Agregar recursos al proyecto": "Add assets to the project",
  "Ejecutar": "Run",
  "Quitar como programa principal": "Unmark as main program",
  "Marcar como programa principal": "Mark as main program",
  "Renombrar...": "Rename...",
  "Abrir en el diseñador": "Open in the designer",
  "Ir a la definición": "Go to definition",
  "Examinar los datos": "Browse the data",
  "Copiar la ruta": "Copy the path",
  "Nuevo programa...": "New program...",
  "Nuevo formulario...": "New form...",
  "Agregar recursos...": "Add assets...",
  "Nuevo nombre de {0}": "New name for {0}",
  "Nombre del formulario": "Name of the form",
  "Nombre del programa": "Name of the program",
  "¿Eliminar {0}? No se puede deshacer.": "Delete {0}? This cannot be undone.",
  // menu designer
  "Diseñador de menús": "Menu designer",
  "Cada fila es una opción. Las opciones con sangría cuelgan de la que tienen encima. Un guion (-) como texto dibuja una línea separadora y \\< subraya la letra que sigue.":
    "Each row is an option. Indented options hang from the one above them. A dash (-) as the text draws a separator line and \\< underlines the next letter.",
  Texto: "Text",
  "Comando, p. ej. DO guardar": "Command, e.g. DO save",
  "Texto de la opción {0}": "Text of option {0}",
  "Comando de la opción {0}": "Command of option {0}",
  "Quitar sangría": "Outdent",
  "Poner sangría": "Indent",
  Subir: "Move up",
  Bajar: "Move down",
  "Agregar opción": "Add option",
  "Insertar en el programa": "Insert into the program",
  "\\<Archivo": "\\<File",
  Nuevo: "New",
  Salir: "Exit",
};

/** A text of the IDE in the current language. `{0}`, `{1}`... take the arguments. */
export function t(es: string, ...args: unknown[]) {
  const text = lang.value === "es" ? es : (en[es] ?? es);
  return text.replace(/\{(\d)\}/g, (_, i) => String(args[i]));
}

// What the interpreter and the file commands say, which arrives in Spanish. `{}` is whatever varies.
// shortcut: matched by shape here instead of translated where it is raised; a message that is not
// listed shows in Spanish. Upgrade to error codes if the list becomes hard to keep.
const messages: [string, string][] = [
  ["Línea {}: {}", "Line {}: {}"],
  ["{}, línea {}: {}", "{}, line {}: {}"],
  ["Error: {}", "Error: {}"],
  ["Variable no encontrada: {}", "Variable not found: {}"],
  ["Función no encontrada: {}", "Function not found: {}"],
  ["Función no implementada", "Function not implemented"],
  ["Comando no reconocido: {}", "Unknown command: {}"],
  ["Sentencia no válida. Para ver el valor de una expresión, escribe ? delante", "Invalid statement. To see the value of an expression, write ? before it"],
  ["Expresión no válida", "Invalid expression"],
  ["Sobra texto al final de la sentencia", "Unexpected text at the end of the statement"],
  ["Se esperaba '{}'", "Expected '{}'"],
  ["Se esperaba un nombre", "Expected a name"],
  ["Se esperaba un objeto", "Expected an object"],
  ["Se esperaba un valor lógico", "Expected a logical value"],
  ["Se esperaba un valor numérico", "Expected a numeric value"],
  ["Se esperaba ON u OFF", "Expected ON or OFF"],
  ["Se esperaba TAG nombre", "Expected TAG name"],
  ["Se esperaba el tamaño del campo", "Expected the size of the field"],
  ["Se esperaba CASE, OTHERWISE o ENDCASE", "Expected CASE, OTHERWISE or ENDCASE"],
  ["Se esperaba CLOSE DATABASES, CLOSE TABLES o CLOSE ALL", "Expected CLOSE DATABASES, CLOSE TABLES or CLOSE ALL"],
  ["Se esperaba INTO CURSOR, INTO ARRAY o INTO TABLE", "Expected INTO CURSOR, INTO ARRAY or INTO TABLE"],
  ["Se esperaba {}", "Expected {}"],
  ["Se pasaron más argumentos de los declarados", "More arguments were passed than declared"],
  ["Faltan argumentos", "Missing arguments"],
  ["Tipo de argumento no válido", "Invalid argument type"],
  ["Los tipos de datos no coinciden con el operador", "The data types do not match the operator"],
  ["División entre cero", "Division by zero"],
  ["Ejecución interrumpida", "Execution interrupted"],
  ["Demasiadas llamadas anidadas (máximo 128)", "Too many nested calls (128 at most)"],
  ["Cadena sin cerrar", "Unclosed string"],
  ["Cadena demasiado larga", "String too long"],
  ["Fecha sin cerrar", "Unclosed date"],
  ["Fecha no válida", "Invalid date"],
  ["Número no válido", "Invalid number"],
  ["Carácter no válido: '{}'", "Invalid character: '{}'"],
  ["';' solo puede ir al final de la línea", "';' can only go at the end of the line"],
  ["Directiva de preprocesador aún no soportada", "Preprocessor directive not supported yet"],
  ["Después de un procedimiento o clase solo puede ir otro PROCEDURE, FUNCTION o DEFINE CLASS", "After a procedure or class only another PROCEDURE, FUNCTION or DEFINE CLASS can follow"],
  ["{} sin cerrar", "{} is not closed"],
  ["{} fuera de un bucle", "{} outside a loop"],
  ["{} sin su bloque de apertura", "{} without its opening block"],
  ["{} no puede ir dentro de un bloque", "{} cannot go inside a block"],
  ["Esta variante de {} aún no está soportada", "This form of {} is not supported yet"],
  ["SET {} aún no está soportado", "SET {} is not supported yet"],
  ["SET DECIMALS admite de 0 a 18", "SET DECIMALS takes 0 to 18"],
  ["Esta regla de columna aún no está soportada en CREATE TABLE", "This column rule is not supported in CREATE TABLE yet"],
  ["Tipo de campo no soportado: {}", "Unsupported field type: {}"],
  ["Cláusula de TEXT no válida", "Invalid TEXT clause"],
  ["Cláusula no soportada en DEFINE PAD o DEFINE BAR", "Clause not supported in DEFINE PAD or DEFINE BAR"],
  ["'@' solo se permite en argumentos de una llamada", "'@' is only allowed in the arguments of a call"],
  ["'.nombre' solo puede usarse dentro de WITH", "'.name' can only be used inside WITH"],
  ["No se puede asignar a esta expresión", "Cannot assign to this expression"],
  ["No es un arreglo", "Not an array"],
  ["Subíndice fuera de rango", "Subscript out of range"],
  ["Dimensiones de arreglo no válidas", "Invalid array dimensions"],
  ["Un arreglo admite una o dos dimensiones", "An array takes one or two dimensions"],
  ["FOR EACH necesita un arreglo", "FOR EACH needs an array"],
  ["El contador del FOR debe ser numérico", "The FOR counter must be numeric"],
  ["El logaritmo necesita un número positivo", "The logarithm needs a positive number"],
  ["SQRT no admite números negativos", "SQRT does not take negative numbers"],
  ["Procedimiento o programa no encontrado: {}", "Procedure or program not found: {}"],
  ["No se pudo leer {}: {}", "Could not read {}: {}"],
  ["Error lanzado por el programa", "Error thrown by the program"],
  ["La variable de la macro &{} debe contener texto", "The variable of the macro &{} must hold text"],
  ["Una macro no puede definir procedimientos ni clases", "A macro cannot define procedures or classes"],
  // objects and forms
  ["Clase no encontrada: {}", "Class not found: {}"],
  ["La herencia de {} es circular", "The inheritance of {} is circular"],
  ["Propiedad no encontrada: {}", "Property not found: {}"],
  ["Método no encontrado: {}", "Method not found: {}"],
  ["Ya existe un miembro llamado {}", "There is already a member called {}"],
  ["Este objeto no tiene Parent", "This object has no Parent"],
  ["Este objeto no está dentro de un formulario", "This object is not inside a form"],
  ["THIS y THISFORM solo pueden usarse dentro de un método", "THIS and THISFORM can only be used inside a method"],
  ["DODEFAULT solo puede usarse dentro de un método", "DODEFAULT can only be used inside a method"],
  ["PageCount, ButtonCount y ColumnCount necesitan un número, en un PageFrame, un grupo o un Grid", "PageCount, ButtonCount and ColumnCount need a number, on a PageFrame, a group or a Grid"],
  ["Formulario no encontrado: {}", "Form not found: {}"],
  ["{} no es un formulario válido: {}", "{} is not a valid form: {}"],
  ["{} no describe un formulario", "{} does not describe a form"],
  ["El código de un evento no puede definir procedimientos ni clases", "The code of an event cannot define procedures or classes"],
  ["{}: el valor de {} no es válido", "{}: the value of {} is not valid"],
  ["El valor de {} no es válido", "The value of {} is not valid"],
  ["El diseñador no conoce la clase {}", "The designer does not know the class {}"],
  ["Un {} no puede contener un {}", "A {} cannot contain a {}"],
  ["El archivo no describe un formulario", "The file does not describe a form"],
  ["No se pudo abrir la ventana del formulario: {}", "Could not open the form's window: {}"],
  // menus
  ["Menú no definido: {}", "Menu not defined: {}"],
  ["Desplegable no definido: {}", "Popup not defined: {}"],
  ["{} no tiene la opción {}", "{} does not have the option {}"],
  // tables
  ["No hay ninguna tabla abierta en esta área de trabajo", "No table is open in this work area"],
  ["No hay ninguna tabla abierta en esa área de trabajo", "No table is open in that work area"],
  ["Tabla no encontrada: {}", "Table not found: {}"],
  ["La tabla {} ya existe", "The table {} already exists"],
  ["Campo no encontrado: {}", "Field not found: {}"],
  ["Alias no encontrado: {}", "Alias not found: {}"],
  ["El alias {} ya está en uso", "The alias {} is already in use"],
  ["Orden no encontrado: {}", "Order not found: {}"],
  ["El registro {} está fuera de rango", "Record {} is out of range"],
  ["El nombre de la tabla debe ser texto", "The name of the table must be text"],
  ["El valor no es del tipo del campo {}", "The value is not of the type of the field {}"],
  ["Un área de trabajo se indica con un número o un alias", "A work area is given as a number or an alias"],
  ["INSERT: el número de valores no coincide con el de campos", "INSERT: the number of values does not match the number of fields"],
  ["SUM necesita una variable por cada expresión", "SUM needs one variable for each expression"],
  ["SEEK necesita un orden activo: use INDEX ON o SET ORDER TO", "SEEK needs a current order: use INDEX ON or SET ORDER TO"],
  ["CONTINUE necesita un LOCATE anterior", "CONTINUE needs an earlier LOCATE"],
  ["SELECT admite una sola tabla, con WHERE, GROUP BY, ORDER BY e INTO", "SELECT takes a single table, with WHERE, GROUP BY, ORDER BY and INTO"],
  ["ORDER BY debe nombrar una columna del resultado", "ORDER BY must name a column of the result"],
  ["Una consulta con GROUP BY debe nombrar sus columnas", "A query with GROUP BY must name its columns"],
  ["Error de la base de datos: {}", "Database error: {}"],
  // files
  ["No se pudo abrir {}: {}", "Could not open {}: {}"],
  ["No se pudo abrir", "Could not open the file"],
  ["No se pudo guardar {}: {}", "Could not save {}: {}"],
  ["No se pudo copiar {}: {}", "Could not copy {}: {}"],
  ["No se pudo renombrar {}: {}", "Could not rename {}: {}"],
  ["No se pudo eliminar {}: {}", "Could not delete {}: {}"],
  ["Ya existe {}", "{} already exists"],
];

const literal = (s: string) => s.replace(/[.*+?^$()|[\]\\]/g, "\\$&");
const shapes = messages.map(([es, to]) => ({ shape: new RegExp(`^${es.split("{}").map(literal).join("([\\s\\S]*?)")}$`), to }));

/** A message of the interpreter in the current language. What varies in it is translated in turn: an error inside an error. */
export function message(text: string): string {
  if (lang.value === "es") return text;
  for (const { shape, to } of shapes) {
    const found = shape.exec(text);
    if (!found) continue;
    let part = 0;
    return to.replace(/\{\}/g, () => message(found[++part]));
  }
  return text;
}

// The shapes are tried in order, so a general one listed before a particular one would hide it.
if (import.meta.env.DEV) {
  const before = lang.value;
  lang.value = "en";
  const check = (es: string, wanted: string) => console.assert(message(es) === wanted, `i18n: ${es} -> ${message(es)}`);
  check("Línea 3: Variable no encontrada: X", "Line 3: Variable not found: X");
  check("demo.cmdGo.Click, línea 2: División entre cero", "demo.cmdGo.Click, line 2: Division by zero");
  check("Se esperaba ON u OFF", "Expected ON or OFF");
  check("Línea 1: TRY sin cerrar", "Line 1: TRY is not closed");
  check("Algo que nadie tradujo", "Algo que nadie tradujo");
  lang.value = before;
}
