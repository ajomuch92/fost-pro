// The one place that talks to the interpreter, and the state every panel shows from it.
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

/** An object of a running form. Property names are upper-cased, as the interpreter keeps them. */
export type FormNode = {
  id: number;
  base: string;
  props: Record<string, any>;
  /** Choices of a ComboBox or ListBox. */
  items: string[];
  /** Upper-cased names of the events somebody handles: only those need reporting. */
  events: string[];
  children: FormNode[];
  /** For a Grid: the table it shows, and the number of its current record. */
  grid?: { fields: string[]; rows: { recno: number; cells: unknown[] }[]; recno: number | null };
};
/** What it takes to draw a control: a node of a running form, or one the designer is laying out. */
export type DrawNode = Pick<FormNode, "base" | "props"> &
  Partial<Omit<FormNode, "children">> & {
    children?: DrawNode[];
    /** For the designer: the node of the form file this was drawn from. */
    source?: object;
  };
/** A built-in class: its properties with their defaults, and the events it can raise. */
export type ClassInfo = { name: string; props: [string, unknown][]; events: string[] };
/** A pad of a menu bar or a bar of a popup. `items` is the popup it opens, when it opens one. */
export type MenuItem = { key: string; prompt: string; disabled: boolean; popup: string | null; items: MenuItem[] | null };
export type Menu = { name: string; items: MenuItem[] };
export type OpenRequest = { form: boolean; path: string | null };
type Entry = { kind: "cmd" | "out" | "err"; text: string };
type RunResult = {
  output: string;
  error: string | null;
  line: number | null;
  clear: boolean;
  forms: FormNode[];
  menus: Menu[];
  focus: number | null;
  open: OpenRequest | null;
  help: boolean;
};

/** What the command window shows. */
export const entries = ref<Entry[]>([]);
/** The forms on screen. */
export const forms = ref<FormNode[]>([]);
/** The menu bars a program has put on screen. */
export const menus = ref<Menu[]>([]);
/** Id of a control a program asked to focus; whoever handles it sets it back to null. */
export const focus = ref<number | null>(null);
/** A file a program asked the IDE to open (MODIFY COMMAND and friends). */
export const openRequest = ref<OpenRequest | null>(null);
/** Requests sent to the interpreter and not answered yet. */
export const pending = ref(0);
/** Folder where DO and DO FORM look for files; null keeps the one used last. */
export const dir = ref<string | null>(null);
export const classes = ref<ClassInfo[]>([]);
/** Whether the help screen is showing. */
export const helpOpen = ref(false);
/** An INPUTBOX() a running program is waiting on. */
export const question = ref<{ prompt: string; title: string; default: string } | null>(null);

export const report = (text: string) => entries.value.push({ kind: "err", text });

invoke<ClassInfo[]>("class_info").then(
  (info) => (classes.value = info),
  (e) => report(String(e)),
);

listen<NonNullable<typeof question.value>>("fost-prompt", (e) => (question.value = e.payload)).catch((e) =>
  report(String(e)),
);

/** Answers the pending INPUTBOX(); null means the user cancelled. */
export function answerPrompt(value: string | null) {
  question.value = null;
  invoke("answer_prompt", { value });
}

async function request(command: string, args: Record<string, unknown>) {
  pending.value++;
  let r: RunResult | undefined;
  try {
    r = await invoke<RunResult>(command, args);
    if (r.clear) entries.value = [];
    // `?` prints its newline first, so the output starts with one
    if (r.output) entries.value.push({ kind: "out", text: r.output.replace(/^\n/, "") });
    if (r.error) report(r.error);
    forms.value = r.forms;
    menus.value = r.menus;
    if (r.focus != null) focus.value = r.focus;
    if (r.open) openRequest.value = r.open;
    if (r.help) helpOpen.value = true;
  } catch (e) {
    report(String(e));
  }
  pending.value--;
  return r;
}

/** Runs `code`, showing `echo` as the command that was typed. */
export function run(code: string, echo: string) {
  entries.value.push({ kind: "cmd", text: echo });
  return request("run_code", { code, dir: dir.value });
}

/** Opens a form from the JSON of a form file, saved or not. */
export function runForm(json: string, name: string) {
  entries.value.push({ kind: "cmd", text: `DO FORM ${name}` });
  return request("run_form", { json, name, dir: dir.value });
}

/** Tells a form what the user did: the values that changed, and the event to run, if any. */
export const uiEvent = (id: number, props: Record<string, unknown>, event: string | null) =>
  request("ui_event", { id, props, event });

/** Tells a menu what the user chose: a pad, or the bar `bar` of `popup` reached through that pad. */
export const menuEvent = (menu: string, pad: string, popup: string | null, bar: string | null) =>
  request("menu_event", { menu, pad, popup, bar });

export const stop = () => invoke("stop_code");
