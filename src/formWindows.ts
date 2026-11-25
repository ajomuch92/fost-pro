// Opens a window of its own for every top-level form (ShowWindow = 2), and closes it when the form goes.
import { watch } from "vue";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { forms, report, TOP_LEVEL } from "./engine";

const open = new Map<number, WebviewWindow>();

watch(forms, (list) => {
  const wanted = list.filter((f) => f.props.SHOWWINDOW === TOP_LEVEL);
  for (const [id, win] of open) {
    if (wanted.some((f) => f.id === id)) continue;
    open.delete(id);
    win.destroy().catch((e) => report(String(e)));
  }
  for (const f of wanted) {
    if (open.has(f.id)) continue;
    // shortcut: the system decides where the window goes, and resizing it does not change Width and Height;
    // upgrade when programs need Left, Top or the Resize event of a top-level form
    const win = new WebviewWindow(`form-${f.id}`, {
      url: `index.html?form=${f.id}`,
      title: f.props.CAPTION,
      width: f.props.WIDTH,
      height: f.props.HEIGHT,
      minimizable: !!f.props.MINBUTTON,
      maximizable: !!f.props.MAXBUTTON,
    });
    open.set(f.id, win);
    win.once("tauri://error", (e) => report(`No se pudo abrir la ventana del formulario: ${e.payload}`));
  }
});

export function closeFormWindows() {
  for (const win of open.values()) win.destroy().catch(() => {});
  open.clear();
}
