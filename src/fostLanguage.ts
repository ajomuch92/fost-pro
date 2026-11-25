import type { StreamParser } from "@codemirror/language";

// shortcut: a copy of the words the Rust parser knows, for colouring only;
// upgrade to a list exported by the interpreter when the migration report needs one too
const keywords =
  "IF THEN ELSE ENDIF DO CASE OTHERWISE ENDCASE WHILE ENDDO FOR EACH IN TO STEP ENDFOR NEXT LOOP EXIT RETURN PROCEDURE FUNCTION ENDPROC ENDFUNC PARAMETERS LPARAMETERS LOCAL PRIVATE PUBLIC DIMENSION DECLARE ARRAY STORE RELEASE CLEAR SET WITH AS AND OR NOT DEFINE CLASS ENDDEFINE ADD OBJECT ENDWITH FORM MODIFY COMMAND CREATE TABLE CURSOR USE SELECT CLOSE APPEND BLANK INSERT INTO VALUES REPLACE DELETE RECALL PACK ZAP GO GOTO TOP BOTTOM SKIP LOCATE CONTINUE SCAN ENDSCAN INDEX ON TAG SEEK COUNT SUM LIST ALL REST WHILE ALIAS ORDER MENU MENUS PAD POPUP POPUPS BAR PROMPT SELECTION ACTIVATE DEACTIVATE OF HELP".split(
    " ",
  );

/** Like the language itself: a keyword may be shortened to four letters or more. */
const isKeyword = (word: string) =>
  keywords.some((k) => k === word || (word.length >= 4 && k.startsWith(word)));

export const fost: StreamParser<unknown> = {
  token(stream) {
    if (stream.sol()) {
      stream.eatSpace();
      if (stream.peek() === "*") {
        stream.skipToEnd();
        return "comment";
      }
      if (stream.match(/^#\w+/)) return "meta";
    }
    if (stream.eatSpace()) return null;
    if (stream.match("&&")) {
      stream.skipToEnd();
      return "comment";
    }
    if (stream.match(/^\.(t|f|y|n|null)\./i)) return "atom";
    if (stream.match(/^\.(and|or|not)\./i)) return "keyword";
    if (stream.match(/^\d+(\.\d+)?/) || stream.match(/^\.\d+/)) return "number";
    if (stream.match(/^\{[^}]*\}/)) return "number";
    if (stream.match(/^[\p{L}_][\p{L}\d_]*/u)) {
      return isKeyword(stream.current().toUpperCase()) ? "keyword" : "variableName";
    }
    const ch = stream.next();
    if (ch === '"' || ch === "'") {
      while (!stream.eol() && stream.next() !== ch);
      return "string";
    }
    return "operator";
  },
};
