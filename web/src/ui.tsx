import { createContext, type ComponentChildren } from "preact";
import { useContext, useEffect, useRef, useState } from "preact/hooks";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { HttpError } from "./client";

const iconPaths = {
  add: "M12 5v14M5 12h14",
  calendar: "M3 5h18v16H3zM3 10h18M8 3v4m8-4v4",
  close: "m6 6 12 12M6 18 18 6",
  display: "M3 4h18v13H3zM8 21h8m-4-4v4",
  overview: "M3 19h18M6 15v-4m6 4V5m6 10V8",
  refresh: "M20 7v5h-5M4 17v-5h5m11 0a8 8 0 0 0-14-5m-2 5a8 8 0 0 0 14 5",
  settings: "M4 6h16M4 12h16M4 18h16M8 3v6m8 0v6m-6 0v6",
  sortDown: "M12 5v14m-5-5 5 5 5-5",
  sortUp: "M12 19V5m-5 5 5-5 5 5",
  undo: "M9 5 3 11l6 6m-6-6h12a5 5 0 0 1 5 5v2",
} as const;

export function Icon({ name }: { name: keyof typeof iconPaths }) {
  return (
    <svg class="ui-icon" aria-hidden="true" focusable="false" width="18" height="18"
      viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"
      stroke-linecap="round" stroke-linejoin="round">
      <path d={iconPaths[name]} />
    </svg>
  );
}

export const Locale = createContext<"de" | "en">("de");
export function useT() {
  const locale = useContext(Locale);
  return (de: string, en: string) => (locale === "de" ? de : en);
}
export function errorText(error: unknown) {
  if (error instanceof HttpError && error.code === "WIP_LIMIT")
    return "WIP-Limit erreicht / WIP limit reached.";
  if (error instanceof HttpError && error.code === "BLOCKED_BY_DEPENDENCY")
    return "Offene Voraussetzung verhindert den Abschluss / An unfinished prerequisite blocks completion.";
  if (error instanceof HttpError && error.code === "AUTH_EXPIRED")
    return "Bitte erneut anmelden. Lokale Änderungen bleiben gespeichert. / Sign in again; local edits are retained.";
  return error instanceof Error ? error.message : String(error);
}
export function Dialog({
  title,
  children,
  close,
  wide = false,
}: {
  title: string;
  children: ComponentChildren;
  close: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const node = ref.current!;
    node.showModal();
    const cancel = (event: Event) => {
      event.preventDefault();
      closeRef.current();
    };
    node.addEventListener("cancel", cancel);
    return () => {
      node.removeEventListener("cancel", cancel);
      node.close();
      previous?.focus();
    };
  }, []);
  const t = useT();
  return (
    <dialog
      ref={ref}
      class={wide ? "dialog wide" : "dialog"}
      aria-label={title}
    >
      <header class="dialog-header">
        <h2>{title}</h2>
        <button
          type="button"
          class="icon-button"
          aria-label={t("Schließen", "Close")}
          onClick={close}
        >
          <Icon name="close" />
        </button>
      </header>
      {children}
    </dialog>
  );
}
export function Markdown({ text }: { text: string }) {
  const t = useT();
  const [images, setImages] = useState(false);
  const html = DOMPurify.sanitize(
    marked.parse(text, { async: false }) as string,
    {
      ALLOWED_TAGS: [
        "p",
        "div",
        "span",
        "br",
        "hr",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "strong",
        "b",
        "em",
        "i",
        "del",
        "s",
        "ins",
        "mark",
        "small",
        "sup",
        "sub",
        "blockquote",
        "pre",
        "code",
        "kbd",
        "var",
        "ul",
        "ol",
        "li",
        "dl",
        "dt",
        "dd",
        "table",
        "thead",
        "tbody",
        "tfoot",
        "tr",
        "th",
        "td",
        "a",
        "img",
        "details",
        "summary",
      ],
      ALLOWED_ATTR: [
        "href",
        "title",
        "src",
        "alt",
        "width",
        "height",
        "class",
        "align",
        "start",
        "colspan",
        "rowspan",
      ],
      FORBID_ATTR: ["style"],
      ALLOW_DATA_ATTR: false,
    },
  );
  // Template content is inert: parsing must not fetch images before consent.
  const template = document.createElement("template");
  template.innerHTML = html;
  const fragment = template.content;
  fragment.querySelectorAll("a").forEach((link) => {
    link.rel = "noopener noreferrer";
    link.target = "_blank";
  });
  const hasImages = !!fragment.querySelector("img");
  if (!images)
    fragment.querySelectorAll("img").forEach((image) => {
      const placeholder = document.createElement("span");
      placeholder.textContent = `[${image.alt || t("Bild", "Image")}]`;
      image.replaceWith(placeholder);
    });
  return (
    <div>
      {hasImages && (
        <label class="inline check">
          <input
            type="checkbox"
            checked={images}
            onChange={(event) => setImages(event.currentTarget.checked)}
          />
          {t(
            "Externe Bilder laden (Verbindung zum Bildanbieter)",
            "Load external images (contacts the image provider)",
          )}
        </label>
      )}
      <div
        class="markdown"
        dangerouslySetInnerHTML={{ __html: template.innerHTML }}
      />
    </div>
  );
}
export function Message({
  children,
  error = false,
}: {
  children: ComponentChildren;
  error?: boolean;
}) {
  return (
    <p
      class={error ? "message error" : "message"}
      role={error ? "alert" : "status"}
    >
      {children}
    </p>
  );
}
export function Busy({ children }: { children: ComponentChildren }) {
  return (
    <span class="busy" role="status">
      <span class="spinner" aria-hidden="true" />
      {children}
    </span>
  );
}
export function confirmDiscard(
  dirty: boolean,
  t: (de: string, en: string) => string,
) {
  return (
    !dirty ||
    window.confirm(
      t("Ungespeicherte Eingaben verwerfen?", "Discard unsaved input?"),
    )
  );
}
export function useUnsavedGuard(dirty: boolean) {
  useEffect(() => {
    if (!dirty) return;
    const guard = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", guard);
    return () => window.removeEventListener("beforeunload", guard);
  }, [dirty]);
}
