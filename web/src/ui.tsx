import { createContext, type ComponentChildren } from "preact";
import { useContext, useEffect, useRef, useState } from "preact/hooks";
import DOMPurify from "dompurify";
import { marked } from "marked";
import { HttpError } from "./client";

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
          ×
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
      FORBID_TAGS: ["style", "iframe", "object", "form"],
      FORBID_ATTR: ["style"],
      ALLOW_DATA_ATTR: false,
    },
  );
  const fragment = new DOMParser().parseFromString(html, "text/html");
  fragment.querySelectorAll("a").forEach((link) => {
    link.rel = "noopener noreferrer";
    link.target = "_blank";
  });
  const hasImages = !!fragment.querySelector("img");
  if (!images)
    fragment.querySelectorAll("img").forEach((image) => {
      const placeholder = fragment.createElement("span");
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
        dangerouslySetInnerHTML={{ __html: fragment.body.innerHTML }}
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
