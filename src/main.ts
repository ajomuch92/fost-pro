import { createApp } from "vue";
import App from "./App.vue";
import FormWindow from "./components/FormWindow.vue";
import { ownForm } from "./engine";
import "./style.css";

// this is an application, not a web page: no browser menu with "Inspect" or "Reload"
document.addEventListener("contextmenu", (e) => e.preventDefault());

createApp(ownForm ? FormWindow : App).mount("#app");
