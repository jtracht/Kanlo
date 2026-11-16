import { useEffect, useState } from "preact/hooks";
import type { Board, Snapshot } from "./types";
import { api } from "./client";
import { Dialog, Icon, Message, errorText, useT } from "./ui";

type DisplayView = "board" | "overview" | "statistics";
type DisplayDesign = "light" | "colorful";
const views: DisplayView[] = ["board", "overview", "statistics"];
type RotationEntry = { view: DisplayView; board_id: string | null };
const entryKey = (entry: RotationEntry) => `${entry.view}:${entry.board_id ?? ""}`;

interface DisplayState {
  revision: number;
  preferences: {
    view: DisplayView;
    design: DisplayDesign;
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
  const [targetBoard, setTargetBoard] = useState("");
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [ticketId, setTicketId] = useState("");
  const [minutes, setMinutes] = useState("");
  const board = targetBoard || currentBoard || boards[0]?.id || "";
  useEffect(() => {
    let current = true;
    setSnapshot(undefined);
    setTicketId("");
    if (board) void api<Snapshot>(`/boards/${encodeURIComponent(board)}/snapshot`)
      .then((value) => { if (current) setSnapshot(value); })
      .catch((reason) => { if (current) setError(errorText(reason)); });
    return () => { current = false; };
  }, [board]);
  const tickets = snapshot?.workspace.tickets.filter((ticket) => !ticket.details.archived && !ticket.details.trashed) ?? [];
  const ticket = tickets.find((item) => item.id === ticketId);
  const validMinutes = minutes === "" || (/^\d+$/.test(minutes) && Number(minutes) <= 300);
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
    <Dialog title={t("TV-Fernbedienung", "TV remote")} close={close}>
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        {notice && <Message>{notice}</Message>}
        <div class="actions wrap">
          <button type="button" class="secondary" disabled={busy || !state} onClick={() => void send({ kind: "scroll", delta: -1 })}>
            {t("Vorherige TV-Seite", "Previous TV page")}
          </button>
          <button type="button" class="secondary" disabled={busy || !state} onClick={() => void send({ kind: "scroll", delta: 1 })}>
            {t("Nächste TV-Seite", "Next TV page")}
          </button>
          <button type="button" class="secondary" disabled={busy || !state} onClick={() => void send({ kind: "back" })}>
            <Icon name="undo" /> {t("Auf TV zurück", "Back on TV")}
          </button>
          <button type="button" class="secondary" disabled={busy || !state} onClick={() => void send({ kind: "close" })}>
            <Icon name="close" /> {t("TV-Einblendung schließen", "Close TV overlay")}
          </button>
        </div>
        <fieldset disabled={busy || !state}>
          <legend>{t("Tickets und Backlog", "Tickets and backlog")}</legend>
          <label>{t("Quellboard", "Source board")}
            <select value={board} onChange={(event) => setTargetBoard(event.currentTarget.value)}>
              {boards.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}
            </select>
          </label>
          <label>{t("Ticket", "Ticket")}
            <select value={ticketId} disabled={!snapshot} onChange={(event) => setTicketId(event.currentTarget.value)}>
              <option value="">{t("Ticket auswählen", "Select ticket")}</option>
              {tickets.map((item) => <option key={item.id} value={item.id}>{item.public_id} · {item.title}</option>)}
            </select>
          </label>
          <label>{t("TV-Dauer in Minuten", "TV duration in minutes")}
            <input type="number" min="0" max="300" step="1" value={minutes} placeholder={t("Standard", "Default")}
              onInput={(event) => setMinutes(event.currentTarget.value)} />
          </label>
          <div class="actions wrap">
            <button type="button" disabled={!ticket || !validMinutes} onClick={() => void send({
              kind: "show_ticket", ticket_id: ticketId, minutes: minutes === "" ? undefined : Number(minutes),
            })}>{t("Auf TV anzeigen", "Show on TV")}</button>
            <button type="button" class="secondary" disabled={!ticket} onClick={() => void send({
              kind: "highlight", ticket_ids: [ticketId], seconds: 25,
            })}>{t("Auf TV hervorheben", "Highlight on TV")}</button>
            <button type="button" class="secondary" disabled={!ticket || !!ticket.details.parent_id} onClick={() => void send({
              kind: "show_children", ticket_id: ticketId,
            })}>{t("Untertickets auf TV anzeigen", "Show subtickets on TV")}</button>
            <button type="button" class="secondary" disabled={!board} onClick={() => void send({
              kind: "show_backlog", board_id: board,
            })}>{t("Backlog auf TV anzeigen", "Show backlog on TV")}</button>
          </div>
        </fieldset>
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
                <label>{t("TV-Design", "TV design")}
                  <select value={preferences.design ?? "light"}
                    onChange={(event) => update({ design: event.currentTarget.value as DisplayDesign })}>
                    <option value="light">{t("Hell", "Light")}</option>
                    <option value="colorful">{t("Farbig", "Colorful")}</option>
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
      </div>
    </Dialog>
  );
}
