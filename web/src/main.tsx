import { render } from "preact";
import { App } from "./App";
import "./style.css";

const root = document.getElementById("app")!;
root.replaceChildren();
render(<App />, root);
