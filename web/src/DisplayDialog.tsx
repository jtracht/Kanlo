import { useEffect, useState } from "preact/hooks";
import type { Board } from "./types";
import { api } from "./client";
import { Dialog, Message, errorText, useT } from "./ui";

type DisplayView = "board" | "overview" | "statistics";
const views: DisplayView[] = ["board", "overview", "statistics"];
type RotationEntry = { view: DisplayView; board_id: string | null };
const entryKey = (entry: RotationEntry) => `${entry.view}:${entry.board_id ?? ""}`;

interface DisplayState {
  revision: number;
  preferences: {
    view: DisplayView;
    board_id: string | null;
    rotation: RotationEntry[];
    rotation_seconds: number;
    overlay_minutes: number;
    statistics_days: number;
    auto_scroll_seconds: number;
  };
}

export function DisplayDialog({
  boards, currentBoard, close,
}: { boards: Board[]; currentBoard: string; close: () => void }) {
  const t = useT();
  const [state, setState] = useState<DisplayState>();
  const [error, setError] = useState("");
  const [notice, setNotice] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    void api<DisplayState>("/display").then(setState).catch((error) => setError(errorText(error)));
  }, []);
  const preferences = state?.preferences;
  const name = (view: DisplayView) => ({
    board: t("Board", "Board"),
    overview: t("Gesamtübersicht", "Overview"),
    statistics: t("Statistik", "Statistics"),
  }[view]);
  const entryName = (entry: RotationEntry) => entry.board_id
    ? boards.find((board) => board.id === entry.board_id)?.name ?? name(entry.view)
    : name(entry.view);
  const rotationChoices: RotationEntry[] = [
    ...boards.map((board): RotationEntry => ({ view: "board", board_id: board.id })),
    { view: "overview", board_id: null },
    { view: "statistics", board_id: null },
  ];

  function update(patch: Partial<DisplayState["preferences"]>) {
    setState((state) => state && { ...state, preferences: { ...state.preferences, ...patch } });
  }

  async function send(action: object) {
    setBusy(true);
    setError("");
    setNotice("");
    try {
      setState(await api<DisplayState>("/display", action));
      setNotice(t("TV-Anzeige aktualisiert.", "TV display updated."));
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }

  function moveRotation(index: number, shift: number) {
    if (!preferences) return;
    const rotation = [...preferences.rotation];
    const item = rotation[index];
    rotation.splice(index, 1);
    rotation.splice(index + shift, 0, item);
    update({ rotation });
  }

  return (
    <Dialog title={t("TV-Anzeige", "TV display")} close={close}>
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        {notice && <Message>{notice}</Message>}
        {state && preferences && (
          <form onSubmit={(event) => {
            event.preventDefault();
            void send({ kind: "configure", preferences, revision: state.revision });
          }}>
            <fieldset disabled={busy}>
              <div class="form-grid">
                <label>{t("Ansicht", "View")}
                  <select value={preferences.view} onChange={(event) => update({ view: event.currentTarget.value as DisplayView })}>
                    {views.map((view) => <option value={view} key={view}>{name(view)}</option>)}
                  </select>
                </label>
                <label>{t("Board auf dem TV", "Board on TV")}
                  <select value={preferences.board_id ?? ""} onChange={(event) => update({ board_id: event.currentTarget.value || null })}>
                    <option value="">{t("Erstes Board", "First board")}</option>
                    {boards.map((board) => <option value={board.id} key={board.id}>{board.name}</option>)}
                  </select>
                </label>
                <label>{t("Rotation in Sekunden", "Rotation in seconds")}
                  <input type="number" min="5" max="3600" required value={preferences.rotation_seconds}
                    onInput={(event) => update({ rotation_seconds: event.currentTarget.valueAsNumber })} />
                </label>
                <label>{t("Einblendung in Minuten (0 = ohne Ablauf)", "Overlay in minutes (0 = no expiry)")}
                  <input type="number" min="0" max="300" required value={preferences.overlay_minutes}
                    onInput={(event) => update({ overlay_minutes: event.currentTarget.valueAsNumber })} />
                </label>
                <label>{t("Statistikzeitraum in Tagen", "Statistics range in days")}
                  <input type="number" min="1" max="90" required value={preferences.statistics_days}
                    onInput={(event) => update({ statistics_days: event.currentTarget.valueAsNumber })} />
                </label>
                <label>{t("TV-Auto-Scroll in Sekunden (0 = aus)", "TV auto-scroll in seconds (0 = off)")}
                  <input type="number" min="0" max="3600" required value={preferences.auto_scroll_seconds}
                    onInput={(event) => update({ auto_scroll_seconds: event.currentTarget.valueAsNumber })} />
                </label>
              </div>
              <fieldset>
                <legend>{t("Ansichten in der Rotation", "Views in rotation")}</legend>
                {rotationChoices.map((entry) => (
                  <label class="inline check" key={entryKey(entry)}>
                    <input type="checkbox" checked={preferences.rotation.some((item) => entryKey(item) === entryKey(entry))}
                      onChange={(event) => update({ rotation: event.currentTarget.checked
                        ? [...preferences.rotation, entry] : preferences.rotation.filter((item) => entryKey(item) !== entryKey(entry)) })} />
                    {entryName(entry)}
                  </label>
                ))}
                {preferences.rotation.map((entry, index) => (
                  <div class="inline spread" key={entryKey(entry)}>
                    <span>{entryName(entry)}</span>
                    <div class="inline">
                      <button type="button" class="secondary" disabled={index === 0}
                        aria-label={`${entryName(entry)} ${t("nach oben", "up")}`} onClick={() => moveRotation(index, -1)}>↑</button>
                      <button type="button" class="secondary" disabled={index === preferences.rotation.length - 1}
                        aria-label={`${entryName(entry)} ${t("nach unten", "down")}`} onClick={() => moveRotation(index, 1)}>↓</button>
                    </div>
                  </div>
                ))}
              </fieldset>
              <button type="submit">{t("Anzeige speichern", "Save display")}</button>
            </fieldset>
          </form>
        )}
        <div class="actions wrap">
          <button type="button" class="secondary" disabled={busy} onClick={() => void send({ kind: "scroll", delta: -1 })}>
            {t("Vorherige TV-Seite", "Previous TV page")}
          </button>
          <button type="button" class="secondary" disabled={busy} onClick={() => void send({ kind: "scroll", delta: 1 })}>
            {t("Nächste TV-Seite", "Next TV page")}
          </button>
          <button type="button" class="secondary" disabled={busy || !currentBoard}
            onClick={() => void send({ kind: "show_backlog", board_id: currentBoard })}>
            {t("Backlog auf TV anzeigen", "Show backlog on TV")}
          </button>
          <button type="button" class="secondary" disabled={busy} onClick={() => void send({ kind: "back" })}>
            {t("Auf TV zurück", "Back on TV")}
          </button>
          <button type="button" class="secondary" disabled={busy} onClick={() => void send({ kind: "close" })}>
            {t("TV-Einblendung schließen", "Close TV overlay")}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
