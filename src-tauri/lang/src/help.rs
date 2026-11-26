//! The reference a help screen shows: every command and function the language supports.

use serde_json::{Value as Json, json};

/// Syntax and what it does.
const COMMANDS: &[(&str, &str)] = &[
    ("? expr [, expr...]", "Muestra los valores en una línea nueva."),
    ("?? expr [, expr...]", "Muestra los valores a continuación, sin cambiar de línea."),
    ("variable = expr", "Asigna un valor a una variable, a un elemento de arreglo o a una propiedad."),
    ("= expr", "Evalúa una expresión y descarta el resultado. Útil para llamar funciones."),
    ("STORE expr TO var1 [, var2...]", "Asigna el mismo valor a varias variables."),
    ("LOCAL nombres", "Declara variables visibles solo en la rutina actual."),
    ("PRIVATE nombres", "Oculta variables externas con esos nombres hasta que la rutina termine."),
    ("PUBLIC nombres", "Declara variables visibles en todo el programa."),
    ("DIMENSION nombre(filas [, columnas])", "Crea un arreglo de una o dos dimensiones. También DECLARE y LOCAL ARRAY."),
    ("RELEASE nombres | ALL", "Elimina variables."),
    ("IF cond ... [ELSE ...] ENDIF", "Ejecuta un bloque según una condición."),
    ("DO CASE ... CASE cond ... [OTHERWISE ...] ENDCASE", "Ejecuta el primer caso cuya condición se cumple."),
    ("FOR var = inicio TO fin [STEP n] ... ENDFOR", "Repite un bloque contando. También se cierra con NEXT."),
    ("FOR EACH var IN arreglo ... ENDFOR", "Repite un bloque por cada elemento de un arreglo."),
    ("DO WHILE cond ... ENDDO", "Repite un bloque mientras la condición se cumpla."),
    ("LOOP", "Salta a la siguiente vuelta del bucle."),
    ("EXIT", "Sale del bucle."),
    ("PROCEDURE nombre[(params)] ... ENDPROC", "Define un procedimiento."),
    ("FUNCTION nombre[(params)] ... ENDFUNC", "Define una función."),
    ("PARAMETERS nombres", "Recibe los argumentos como variables privadas."),
    ("LPARAMETERS nombres", "Recibe los argumentos como variables locales."),
    ("RETURN [expr]", "Termina la rutina y devuelve un valor (.T. si no se indica)."),
    ("DO nombre [WITH args]", "Ejecuta un procedimiento o el programa nombre.fost. Las variables se pasan por referencia."),
    ("SET PROCEDURE TO programa [, programa...]", "Deja disponibles los procedimientos, funciones y clases de otros programas, sin ejecutarlos."),
    ("DEFINE CLASS nombre AS clase ... ENDDEFINE", "Define una clase con propiedades, ADD OBJECT y métodos PROCEDURE o FUNCTION."),
    ("WITH objeto ... ENDWITH", "Dentro del bloque, .Propiedad se refiere a ese objeto."),
    ("DO FORM nombre [WITH args] [NAME var] [NOSHOW]", "Abre el formulario nombre.fostform."),
    ("CREATE FORM [nombre]", "Abre el diseñador con un formulario nuevo."),
    ("MODIFY FORM nombre", "Abre un formulario en el diseñador."),
    ("MODIFY COMMAND [nombre]", "Abre un programa en el editor."),
    ("CREATE TABLE nombre (campo tipo(ancho [, decimales]), ...)", "Crea una tabla en la carpeta y la abre. Tipos: C texto, N número, I entero, L lógico, D fecha, M memo."),
    ("CREATE CURSOR nombre (campo tipo, ...)", "Crea una tabla temporal en memoria, que desaparece al cerrarla."),
    ("USE [tabla] [ALIAS alias] [IN area] [ORDER orden]", "Abre una tabla en un área de trabajo. Sin tabla, cierra la del área."),
    ("SELECT area | alias", "Cambia de área de trabajo. SELECT 0 elige la primera libre."),
    ("CLOSE DATABASES | TABLES | ALL", "Cierra todas las tablas."),
    ("APPEND BLANK", "Agrega un registro en blanco y se coloca en él."),
    ("INSERT INTO tabla [(campos)] VALUES (valores)", "Agrega un registro con esos valores."),
    ("REPLACE campo WITH expr [, ...] [alcance]", "Cambia campos del registro actual, o de los del alcance."),
    ("DELETE [alcance]", "Marca registros para borrar. RECALL quita la marca."),
    ("PACK", "Elimina los registros marcados y renumera el resto. ZAP los elimina todos."),
    ("GO TOP | BOTTOM | numero", "Va al primer registro, al último, o al de ese número."),
    ("SKIP [n]", "Avanza n registros (1 si no se indica); con un número negativo, retrocede."),
    ("LOCATE FOR cond", "Busca el primer registro que cumple la condición. CONTINUE busca el siguiente."),
    ("SCAN [alcance] ... ENDSCAN", "Repite un bloque por cada registro."),
    ("INDEX ON expr TAG nombre [DESCENDING]", "Crea un orden por una expresión y lo activa."),
    ("SET ORDER TO [nombre]", "Activa un orden; sin nombre, vuelve al orden de los registros."),
    ("SEEK valor", "Busca el valor en el orden activo. El texto coincide por su comienzo, salvo con SET EXACT ON."),
    ("SET FILTER TO [condicion]", "Solo se ven los registros que cumplen la condición, también en un Grid. Sin condición, se ven todos."),
    ("SELECT columnas FROM tabla [WHERE cond] [GROUP BY ...] [ORDER BY ...] [INTO CURSOR | ARRAY | TABLE nombre]", "Consulta una tabla. Columnas: *, expresiones con AS nombre, y COUNT(*), SUM(), AVG(), MIN(), MAX(). También DISTINCT y TOP n. Sin INTO, muestra el resultado. Deja en _TALLY cuántos registros obtuvo."),
    ("UPDATE tabla SET campo = expr [, ...] [WHERE cond]", "Cambia campos en los registros que cumplen la condición."),
    ("DELETE FROM tabla [WHERE cond]", "Marca para borrar los registros que cumplen la condición."),
    ("BROWSE", "Muestra la tabla actual en una ventana, donde se puede recorrer y editar."),
    ("SET DELETED ON | OFF", "Con ON, los registros marcados para borrar se ignoran."),
    ("COUNT [alcance] TO variable", "Cuenta registros."),
    ("SUM expr [, ...] TO var [, ...] [alcance]", "Suma expresiones sobre los registros."),
    ("LIST [expr, ...] [alcance]", "Muestra los registros en la ventana de comandos."),
    ("alcance", "En los comandos de tablas: ALL, NEXT n, REST, FOR condición y WHILE condición."),
    ("TRY ... CATCH [TO var] ... [FINALLY ...] ENDTRY", "Si algo falla en TRY, sigue en CATCH; var recibe un objeto con Message, LineNo y UserValue. FINALLY se ejecuta siempre."),
    ("THROW [valor]", "Provoca un error, que un CATCH puede atender. El valor llega en UserValue."),
    ("ON ERROR [comando]", "El comando que se ejecuta cuando una sentencia falla; el programa sigue en la siguiente. Sin comando, se quita."),
    ("TEXT [TO var [ADDITIVE]] [TEXTMERGE] ... ENDTEXT", "Las líneas hasta ENDTEXT son texto: se guardan en var, o se muestran. Con TEXTMERGE, <<expresión>> se sustituye por su valor."),
    ("comando con &variable", "La macro &variable se sustituye por el texto que contiene antes de ejecutar la línea: USE &tabla. Un punto termina el nombre."),
    ("DEFINE MENU nombre", "Crea una barra de menú vacía. ACTIVATE MENU la muestra y DEACTIVATE MENU la oculta."),
    ("DEFINE PAD nombre OF menu PROMPT texto [SKIP FOR cond]", "Agrega una opción a una barra de menú. El menú _MSYSMENU siempre está visible y no hay que definirlo."),
    ("DEFINE POPUP nombre", "Crea un menú desplegable vacío."),
    ("DEFINE BAR numero OF popup PROMPT texto [SKIP FOR cond]", "Agrega una opción a un desplegable. El texto \\- dibuja una línea separadora y \\< subraya la letra que sigue. SKIP FOR la deshabilita mientras la condición se cumpla."),
    ("ON PAD nombre OF menu ACTIVATE POPUP popup", "Hace que la opción abra un desplegable. ON BAR numero OF popup ACTIVATE POPUP otro crea un submenú."),
    ("ON SELECTION PAD nombre OF menu comando", "El comando que se ejecuta al elegir la opción. También ON SELECTION BAR numero OF popup comando."),
    ("ON SELECTION POPUP nombre comando", "El comando para las opciones del desplegable que no tienen uno propio. También ON SELECTION MENU."),
    ("RELEASE MENUS | POPUPS [nombres]", "Elimina menús o desplegables; sin nombres, todos. SET SYSMENU TO vacía _MSYSMENU."),
    ("CLEAR", "Limpia la ventana de comandos. CLEAR ALL elimina además las variables."),
    ("SET EXACT ON | OFF", "Con ON, = compara cadenas completas; con OFF, solo hasta el largo de la derecha."),
    ("SET DECIMALS TO n", "Máximo de decimales que muestra ?."),
    ("#DEFINE NOMBRE valor", "Sustituye NOMBRE por el valor en el resto del programa."),
    ("HELP", "Abre esta ayuda."),
    ("* comentario", "Línea de comentario. && comenta el resto de una línea."),
    ("sentencia ;", "Un punto y coma al final continúa la sentencia en la línea siguiente."),
];

/// Group, syntax and what it returns. The name is the syntax up to the parenthesis.
pub(crate) const FUNCTIONS: &[(&str, &str, &str)] = &[
    ("Texto", "LEN(texto)", "Número de caracteres."),
    ("Texto", "UPPER(texto)", "El texto en mayúsculas."),
    ("Texto", "LOWER(texto)", "El texto en minúsculas."),
    ("Texto", "PROPER(texto)", "Cada palabra con inicial mayúscula."),
    ("Texto", "ALLTRIM(texto)", "Quita los espacios de ambos extremos."),
    ("Texto", "LTRIM(texto)", "Quita los espacios del inicio."),
    ("Texto", "RTRIM(texto)", "Quita los espacios del final."),
    ("Texto", "TRIM(texto)", "Igual que RTRIM."),
    ("Texto", "LEFT(texto, n)", "Los primeros n caracteres."),
    ("Texto", "RIGHT(texto, n)", "Los últimos n caracteres."),
    ("Texto", "SUBSTR(texto, inicio [, largo])", "Una parte del texto, contando desde 1."),
    ("Texto", "AT(buscado, texto [, ocurrencia])", "Posición de un texto dentro de otro; 0 si no está."),
    ("Texto", "ATC(buscado, texto [, ocurrencia])", "Como AT, sin distinguir mayúsculas."),
    ("Texto", "RAT(buscado, texto [, ocurrencia])", "Como AT, buscando desde el final."),
    ("Texto", "OCCURS(buscado, texto)", "Cuántas veces aparece un texto en otro."),
    ("Texto", "SPACE(n)", "Un texto de n espacios."),
    ("Texto", "REPLICATE(texto, n)", "El texto repetido n veces."),
    ("Texto", "STRTRAN(texto, buscado [, reemplazo])", "Reemplaza todas las apariciones."),
    ("Texto", "STUFF(texto, inicio, cuantos, reemplazo)", "Sustituye una parte del texto por otra."),
    ("Texto", "PADL(valor, ancho [, relleno])", "Rellena por la izquierda hasta el ancho."),
    ("Texto", "PADR(valor, ancho [, relleno])", "Rellena por la derecha hasta el ancho."),
    ("Texto", "PADC(valor, ancho [, relleno])", "Centra rellenando ambos lados."),
    ("Texto", "CHR(codigo)", "El carácter con ese código."),
    ("Texto", "ASC(texto)", "El código del primer carácter."),
    ("Texto", "STR(numero [, ancho [, decimales]])", "Convierte un número en texto alineado a la derecha."),
    ("Texto", "VAL(texto)", "Convierte en número el inicio de un texto."),
    ("Texto", "TRANSFORM(valor [, mascara])", "Convierte cualquier valor en texto. Con máscara: \"999,999.99\" para números, \"@!\" mayúsculas, \"@L 99999\" ceros a la izquierda, \"@R 999-9999\" intercala los símbolos."),
    ("Texto", "CHRTRAN(texto, buscados, reemplazos)", "Cambia cada carácter por su equivalente."),
    ("Texto", "ISALPHA(texto)", "Si empieza por una letra."),
    ("Texto", "ISDIGIT(texto)", "Si empieza por un dígito."),
    ("Números", "ABS(n)", "Valor absoluto."),
    ("Números", "INT(n)", "Parte entera."),
    ("Números", "ROUND(n, decimales)", "Redondea."),
    ("Números", "MOD(a, b)", "Resto de dividir a entre b. Igual que a % b."),
    ("Números", "MAX(a, b [, ...])", "El mayor de los valores."),
    ("Números", "MIN(a, b [, ...])", "El menor de los valores."),
    ("Números", "SQRT(n)", "Raíz cuadrada."),
    ("Números", "CEILING(n)", "El entero igual o siguiente."),
    ("Números", "FLOOR(n)", "El entero igual o anterior."),
    ("Números", "SIGN(n)", "1, 0 o -1 según el signo."),
    ("Números", "EXP(n)", "e elevado a n."),
    ("Números", "LOG(n)", "Logaritmo natural."),
    ("Números", "LOG10(n)", "Logaritmo en base 10."),
    ("Números", "PI()", "El número pi."),
    ("Números", "RAND()", "Un número al azar entre 0 y 1."),
    ("Números", "RGB(rojo, verde, azul)", "Un color, para ForeColor y BackColor."),
    ("Fechas", "DATE([año, mes, dia])", "La fecha de hoy, o la indicada."),
    ("Fechas", "TIME()", "La hora actual como texto HH:MM:SS."),
    ("Fechas", "SECONDS()", "Segundos transcurridos desde la medianoche."),
    ("Fechas", "CTOD(texto)", "Convierte un texto MM/DD/AAAA en fecha."),
    ("Fechas", "DTOC(fecha)", "Convierte una fecha en texto MM/DD/AAAA."),
    ("Fechas", "DTOS(fecha)", "Convierte una fecha en texto AAAAMMDD."),
    ("Fechas", "YEAR(fecha)", "El año."),
    ("Fechas", "MONTH(fecha)", "El mes, de 1 a 12."),
    ("Fechas", "DAY(fecha)", "El día del mes."),
    ("Fechas", "DOW(fecha)", "El día de la semana; 1 es domingo."),
    ("Fechas", "CDOW(fecha)", "El nombre del día de la semana, en inglés."),
    ("Fechas", "CMONTH(fecha)", "El nombre del mes, en inglés."),
    ("Fechas", "GOMONTH(fecha, meses)", "La fecha desplazada ese número de meses."),
    ("Lógica y tipos", "IIF(cond, si, no)", "Uno de dos valores según la condición."),
    ("Lógica y tipos", "EMPTY(valor)", "Si está vacío: 0, texto en blanco, .F. o fecha vacía."),
    ("Lógica y tipos", "ISNULL(valor)", "Si es .NULL."),
    ("Lógica y tipos", "NVL(valor, alterno)", "El valor, o el alterno si es .NULL."),
    ("Lógica y tipos", "BETWEEN(valor, minimo, maximo)", "Si el valor está en el rango."),
    ("Lógica y tipos", "INLIST(valor, a, b [, ...])", "Si el valor es igual a alguno de la lista."),
    ("Lógica y tipos", "TYPE(texto)", "El tipo de la expresión escrita en el texto; U si no existe."),
    ("Lógica y tipos", "VARTYPE(valor)", "El tipo del valor: N, C, L, D, O, X (nulo) o U."),
    ("Lógica y tipos", "EVALUATE(texto)", "Evalúa la expresión escrita en el texto."),
    ("Lógica y tipos", "ALEN(arreglo [, 1 | 2])", "Elementos de un arreglo, o sus filas o columnas."),
    ("Tablas", "RECNO([area])", "El número del registro actual."),
    ("Tablas", "RECCOUNT([area])", "Cuántos registros tiene la tabla, incluidos los marcados para borrar."),
    ("Tablas", "EOF([area])", "Si el puntero pasó del último registro."),
    ("Tablas", "BOF([area])", "Si se intentó retroceder antes del primer registro."),
    ("Tablas", "FOUND([area])", "Si el último LOCATE, CONTINUE o SEEK encontró algo."),
    ("Tablas", "DELETED([area])", "Si el registro actual está marcado para borrar."),
    ("Tablas", "SEEK(valor [, area])", "Busca como el comando SEEK y dice si lo encontró."),
    ("Tablas", "ALIAS([area])", "El alias de la tabla abierta en un área; vacío si no hay."),
    ("Tablas", "SELECT([alias])", "El número del área actual, o el de un alias (0 si no está abierto)."),
    ("Tablas", "USED([area])", "Si hay una tabla abierta en el área o con ese alias."),
    ("Tablas", "FCOUNT([area])", "Cuántos campos tiene la tabla."),
    ("Tablas", "FIELD(n [, area])", "El nombre del campo número n."),
    ("Tablas", "CURSORSETPROP(\"Buffering\", modo [, area])", "Con modo 2 a 5, los cambios de la tabla se pueden deshacer con TABLEREVERT(); con 1, ya no."),
    ("Tablas", "TABLEUPDATE([todos [, forzar [, area]]])", "Da por buenos los cambios hechos desde que se activó el buffering."),
    ("Tablas", "TABLEREVERT([todos [, area]])", "Deshace los cambios hechos desde el último TABLEUPDATE(). Devuelve cuántos registros."),
    ("Menús", "BAR()", "Número de la opción de desplegable elegida por última vez; 0 si fue una de la barra."),
    ("Menús", "PROMPT()", "Texto de la opción de menú elegida por última vez."),
    ("Menús", "POPUP()", "Nombre del desplegable de la opción elegida por última vez."),
    ("Menús", "PAD()", "Nombre de la opción de la barra por la que se llegó a la elegida."),
    ("Menús", "MENU()", "Nombre de la barra de menú de la opción elegida por última vez."),
    ("Errores", "MESSAGE()", "El texto del último error que el programa atendió con ON ERROR o CATCH."),
    ("Errores", "LINENO()", "La línea de ese error."),
    ("Errores", "ERROR()", "1 si hubo un error atendido, 0 si no. Los errores no tienen los números de FoxPro."),
    ("Objetos y diálogos", "CREATEOBJECT(clase [, args])", "Crea un objeto de una clase."),
    ("Objetos y diálogos", "DODEFAULT([args])", "Dentro de un método, llama al de la clase padre."),
    ("Objetos y diálogos", "MESSAGEBOX(mensaje [, tipo [, titulo]])", "Muestra un mensaje. Tipo: 0 Aceptar, 1 Aceptar/Cancelar, 3 Sí/No/Cancelar, 4 Sí/No; sume 16 error, 48 aviso, 64 información. Devuelve 1 Aceptar, 2 Cancelar, 6 Sí, 7 No."),
    ("Objetos y diálogos", "INPUTBOX(pregunta [, titulo [, inicial]])", "Pide un texto al usuario. Devuelve el texto, o vacío si cancela."),
];

/// What every object answers to, besides the properties and events of its class.
const MEMBERS: &[(&str, &str)] = &[
    ("THIS", "Dentro de un método, el objeto al que pertenece."),
    ("THISFORM", "Dentro de un método, el formulario que contiene al objeto."),
    ("objeto.Parent", "El objeto que lo contiene."),
    ("objeto.Controls / ControlCount", "Los controles que contiene, como arreglo, y cuántos son."),
    ("objeto.Show() / Hide()", "Muestra u oculta el objeto."),
    ("objeto.Release()", "Cierra el formulario o elimina el control, tras ejecutar Destroy."),
    ("objeto.SetFocus()", "Lleva el cursor al control."),
    ("contenedor.control", "Un control dentro de otro: THISFORM.PageFrame1.Page1.txtNombre."),
    ("marco.PageCount / grupo.ButtonCount", "Cuántas páginas o botones tiene. Asignarle un número los crea o elimina."),
    ("marco.Pages(n) / grupo.Buttons(n)", "Una página de un PageFrame, o un botón de un OptionGroup o CommandGroup."),
    ("marco.ActivePage", "El número de la página que se muestra."),
    ("grupo.Value", "En un OptionGroup, el número de la opción elegida; en un CommandGroup, el del último botón pulsado."),
    ("enlace.NavigateTo(direccion)", "Abre una dirección web en el navegador."),
    ("objeto.AddObject(nombre, clase)", "Agrega un control en ejecución. Nace oculto: ponga Visible = .T."),
    ("objeto.RemoveObject(nombre)", "Elimina un control."),
    ("objeto.AddProperty(nombre [, valor])", "Agrega una propiedad."),
    ("formulario.ShowWindow", "Con 2, el formulario se abre en una ventana propia, fuera de la del IDE."),
    ("control.ControlSource", "El campo (cliente.nombre) o la variable que el control muestra y en la que guarda lo que el usuario escribe. Lo tienen TextBox, EditBox, Spinner, CheckBox, ComboBox, ListBox y OptionGroup."),
    ("objeto.Refresh()", "Vuelve a cargar los controles enlazados con el valor de su ControlSource."),
    ("grid.ColumnCount / Column1.ControlSource / Column1.Header1.Caption", "Con ColumnCount el Grid muestra solo esas columnas: cada Column elige su campo, su Width y el título de su Header1."),
    ("grid.ReadOnly", "Con .T., las celdas no se pueden editar. Para editar una celda se hace doble clic en ella."),
    ("lista.RowSourceType / RowSource", "Con tipo 1, RowSource es una lista separada por comas; con 6, un campo (cliente.nombre); con 2, un alias (su primer campo)."),
    ("grid.RecordSource", "El alias de la tabla que muestra un Grid; vacío, la del área actual. Un clic en una fila la hace el registro actual y ejecuta AfterRowColChange."),
    ("lista.AddItem(texto) / RemoveItem(n) / Clear()", "Cambia las opciones de un ComboBox o ListBox."),
    ("lista.List(n) / ListCount / ListIndex", "Una opción, cuántas hay, y cuál está elegida."),
];

#[cfg(test)]
pub(crate) fn function_name(syntax: &str) -> &str {
    syntax.split('(').next().unwrap()
}

/// The reference, in Spanish or in English.
pub fn reference(english: bool) -> Json {
    use crate::help_en as en;
    let (commands, functions, members) = if english { (en::COMMANDS, en::FUNCTIONS, en::MEMBERS) } else { (COMMANDS, FUNCTIONS, MEMBERS) };
    let commands: Vec<Json> = commands.iter().map(|(syntax, text)| json!({ "syntax": syntax, "text": text })).collect();
    let functions: Vec<Json> =
        functions.iter().map(|(group, syntax, text)| json!({ "group": group, "syntax": syntax, "text": text })).collect();
    let members: Vec<Json> = members.iter().map(|(syntax, text)| json!({ "syntax": syntax, "text": text })).collect();
    json!({ "commands": commands, "functions": functions, "members": members })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::help_en as en;

    // the English reference is the Spanish one, entry by entry
    #[test]
    fn both_languages_list_the_same_things() {
        assert_eq!(COMMANDS.len(), en::COMMANDS.len());
        assert_eq!(MEMBERS.len(), en::MEMBERS.len());
        let names = |list: &[(&'static str, &'static str, &'static str)]| list.iter().map(|f| function_name(f.1)).collect::<Vec<_>>();
        assert_eq!(names(FUNCTIONS), names(en::FUNCTIONS));
        // the first word of a command is the command itself, whatever the language
        let word = |list: &[(&'static str, &'static str)]| list.iter().map(|c| c.0.split(' ').next().unwrap()).collect::<Vec<_>>();
        let (es, english) = (word(COMMANDS), word(en::COMMANDS));
        let differ: Vec<_> = es.iter().zip(&english).filter(|(a, b)| a != b).collect();
        // only the entries that are words and not commands: "alcance", "comando con &variable", "sentencia ;"
        assert_eq!(differ.len(), 3, "{differ:?}");
    }
}
