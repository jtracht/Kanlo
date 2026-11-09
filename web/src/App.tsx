import { useCallback, useEffect, useRef, useState } from "preact/hooks";
import type {
  Bootstrap,
  Command,
  QueueEntry,
  Receipt,
  Snapshot,
  Ticket,
  View,
} from "./types";
import { defaultView, emptySnapshot } from "./types";
import * as client from "./client";
import * as storage from "./storage";
import { blocked, deadlineTime, visibleTickets } from "./workspace";
import { Dialog, Locale, Message, Busy, useT, errorText } from "./ui";
import { TicketDialog } from "./TicketDialog";
import { SettingsDialog } from "./SettingsDialog";
import { DataDialog } from "./DataDialog";
import { HistoryDialog } from "./HistoryDialog";
import { PreviewDialog, QueueDialog } from "./QueueDialog";
import { uuid } from "./identifiers";

export function App() {
  const [bootstrap, setBootstrap] = useState<Bootstrap>();
  return (
    <Locale.Provider value={bootstrap?.settings.language ?? "de"}>
      <Main bootstrap={bootstrap} setBootstrap={setBootstrap} />
    </Locale.Provider>
  );
}

function Main({
  bootstrap,
  setBootstrap,
}: {
  bootstrap?: Bootstrap;
  setBootstrap: (value: Bootstrap) => void;
}) {
  const t = useT();
  const [snapshot, setSnapshot] = useState<Snapshot>();
  const [boardId, setBoardId] = useState("");
  const active = useRef("");
  const [view, setView] = useState<View>(defaultView(""));
  const viewTimer = useRef<number | undefined>(undefined);
  const [profile, setProfile] = useState("");
  const [entries, setEntries] = useState<QueueEntry[]>([]);
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [title, setTitle] = useState("");
  const [backlog, setBacklog] = useState(true);
  const [selected, setSelected] = useState<string[]>([]);
  const [editor, setEditor] = useState<{
    id: string;
    token: number;
    fallback: Ticket;
    source: Snapshot;
  }>();
  const aliases = useRef<Record<string, string>>({});
  const boardRequest = useRef(0);
  const [panel, setPanel] = useState<
    "settings" | "data" | "history" | "queue" | "board" | "bulk" | "statistics"
  >();
  const [preview, setPreview] = useState<QueueEntry>();
  const [newBoardName, setNewBoardName] = useState("");
  const [newPrefix, setNewPrefix] = useState("");
  const [dragging, setDragging] = useState("");
  const [toast, setToast] = useState<Receipt>();
  const toastTimer = useRef<number | undefined>(undefined);
  const [authDismissed, setAuthDismissed] = useState(false);
  const [password, setPassword] = useState("");
  const [loginError, setLoginError] = useState("");
  const [pwa, setPwa] = useState("");
  const latest = useRef({ snapshot, bootstrap });
  latest.current = { snapshot, bootstrap };
  const context = (board: string, client = profile) => `web:${client}:${board}`;

  const refresh = useCallback(async () => {
    try {
      const boot = await client.loadBootstrap();
      setBootstrap(boot);
      let id = aliases.current[active.current] || active.current;
      if (!boot.boards.some((board) => board.id === id))
        id = boot.boards[0]?.id ?? "";
      if (id) {
        active.current = id;
        setBoardId(id);
        const request = ++boardRequest.current;
        const next = await client.loadBoard(id);
        if (request === boardRequest.current && id === active.current)
          setSnapshot(next);
      } else {
        ++boardRequest.current;
        active.current = "";
        setBoardId("");
        setSnapshot(undefined);
      }
      setEntries(await storage.queue());
      setError("");
    } catch (error) {
      setError(errorText(error));
    }
  }, []);

  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const id = await storage.clientId();
        setProfile(id);
        const boot = await client.loadBootstrap();
        if (!alive) return;
        setBootstrap(boot);
        const previous = await (await storage.db).get("meta", "last_board");
        const board =
          boot.boards.find((board) => board.id === previous)?.id ??
          boot.boards[0]?.id;
        if (board && !active.current) {
          const request = ++boardRequest.current;
          active.current = board;
          setBoardId(board);
          const data = await client.loadBoard(board);
          if (!alive) return;
          if (request === boardRequest.current) {
            setSnapshot(data);
            setView(
              data.view_states[`web:${id}:${board}`] ?? defaultView(board),
            );
          }
        }
        setEntries(await storage.queue());
      } catch (error) {
        if (alive) setError(errorText(error));
      } finally {
        if (alive) setLoading(false);
      }
    })();
    const unsubscribe = storage.subscribe((mapping) => {
      if (mapping) {
        Object.assign(aliases.current, mapping);
        if (mapping[active.current]) {
          active.current = mapping[active.current];
          setBoardId(active.current);
          setView((view) => ({ ...view, board_id: active.current }));
        }
        setEditor((previous) =>
          previous
            ? {
                ...previous,
                id: mapping[previous.id] ?? previous.id,
                fallback: storage.remap(previous.fallback, mapping) as Ticket,
                source: storage.remap(previous.source, mapping) as Snapshot,
              }
            : undefined,
        );
        setSelected((previous) => previous.map((id) => mapping[id] ?? id));
      }
      void (async () => {
        const boot = await storage.bootstrapCache();
        if (boot) setBootstrap(boot);
        const id = active.current;
        const data = await storage.boardCache(id);
        if (data && id === active.current) setSnapshot(data);
        setEntries(await storage.queue());
      })();
    });
    return () => {
      alive = false;
      unsubscribe();
    };
  }, []);

  useEffect(() => {
    if (!bootstrap) return;
    const settings = bootstrap.settings;
    const dark =
      settings.theme === "dark" ||
      (settings.theme === "system" &&
        matchMedia("(prefers-color-scheme: dark)").matches);
    document.documentElement.dataset.theme = dark ? "dark" : "light";
    document.documentElement.dataset.animation = settings.animation;
    document.documentElement.style.setProperty(
      "--accent",
      settings.accent_color,
    );
    document.documentElement.lang = settings.language;
  }, [
    bootstrap?.settings.language,
    bootstrap?.settings.theme,
    bootstrap?.settings.animation,
    bootstrap?.settings.accent_color,
  ]);
  useEffect(() => {
    if (!bootstrap || client.connection.authRequired) return;
    return client.startSynchronization(() => void refresh());
  }, [!!bootstrap, client.connection.authRequired]);
  useEffect(() => {
    if (!isSecureContext || !("serviceWorker" in navigator)) {
      setPwa("http");
      return;
    }
    navigator.serviceWorker
      .register("/sw.js")
      .then(() => navigator.serviceWorker.ready)
      .then(() => setPwa("ready"))
      .catch((error) => {
        setPwa("error");
        setError(errorText(error));
      });
  }, []);
  useEffect(() => {
    const receipt = (event: Event) => {
      const value = (event as CustomEvent<Receipt>).detail;
      if (
        value?.result?.affected?.length &&
        latest.current.snapshot?.workspace.board_options[active.current]
          ?.undo_depth
      ) {
        setToast(value);
        clearTimeout(toastTimer.current);
        toastTimer.current = window.setTimeout(() => setToast(undefined), 7000);
      }
    };
    window.addEventListener("kanlo:receipt", receipt);
    return () => {
      window.removeEventListener("kanlo:receipt", receipt);
      clearTimeout(toastTimer.current);
      clearTimeout(viewTimer.current);
    };
  }, []);

  async function selectBoard(id: string) {
    id = aliases.current[id] || id;
    const request = ++boardRequest.current;
    active.current = id;
    setBoardId(id);
    clearTimeout(viewTimer.current);
    setLoading(true);
    setError("");
    try {
      const data = await client.loadBoard(id);
      if (request !== boardRequest.current) return;
      setSnapshot(data);
      setView(data.view_states[context(id)] ?? defaultView(id));
      setSelected([]);
      await (await storage.db).put("meta", id, "last_board");
    } catch (error) {
      if (request === boardRequest.current) setError(errorText(error));
    } finally {
      if (request === boardRequest.current) setLoading(false);
    }
  }
  function changeView(next: View) {
    setView(next);
    clearTimeout(viewTimer.current);
    const id = next.board_id;
    viewTimer.current = window.setTimeout(async () => {
      const data = await storage.boardCache(id);
      if (data)
        await client
          .queueCommand(
            { kind: "set_view", context: context(id), view: next },
            id,
            data,
          )
          .catch((error) => setError(errorText(error)));
    }, 600);
  }
  async function open(id: string) {
    try {
      id = aliases.current[id] || id;
      const cached = (await (await storage.db).getAll("boards"))
        .flatMap((snapshot) => snapshot.workspace.tickets)
        .find((ticket) => ticket.id === id || ticket.public_id === id);
      const ticket =
        cached ??
        (await client.api<Ticket>(`/tickets/${encodeURIComponent(id)}`));
      if (ticket.board_id !== active.current)
        await selectBoard(ticket.board_id);
      const source = await storage.boardCache(ticket.board_id);
      if (!source)
        throw new Error(
          "Board nicht lokal verfügbar / Board unavailable locally",
        );
      setEditor({ id: ticket.id, token: Date.now(), fallback: ticket, source });
    } catch (error) {
      setError(errorText(error));
    }
  }
  async function create(event: Event) {
    event.preventDefault();
    if (!title.trim() || !snapshot) return;
    try {
      await client.createTicket(
        { kind: "create_ticket", title: title.trim() },
        boardId,
      );
      setTitle("");
    } catch (error) {
      setError(errorText(error));
    }
  }
  async function move(ticket: Ticket, column: string | null, before?: string) {
    if (!snapshot || ticket.details.parent_id) return;
    try {
      if (ticket.column_id !== column)
        await client.patchTicket(ticket, {
          board_id: boardId,
          column_id: column,
        });
      if (view.sort === "manual" && before) {
        const data = await storage.boardCache(boardId);
        if (!data) return;
        const ids = data.workspace.tickets
          .filter(
            (ticket) =>
              ticket.column_id === column &&
              !ticket.details.parent_id &&
              !ticket.details.trashed &&
              !ticket.details.archived,
          )
          .sort((a, b) => a.details.rank - b.details.rank)
          .map((ticket) => ticket.id)
          .filter((id) => id !== ticket.id);
        ids.splice(Math.max(0, ids.indexOf(before)), 0, ticket.id);
        await client.queueCommand(
          {
            kind: "reorder_tickets",
            board_id: boardId,
            column_id: column,
            ids,
          },
          boardId,
          data,
        );
      }
      setDragging("");
    } catch (error) {
      setError(errorText(error));
    }
  }
  async function reorder(ticket: Ticket, offset: number) {
    if (!snapshot) return;
    const ids = snapshot.workspace.tickets
      .filter(
        (value) =>
          value.column_id === ticket.column_id &&
          !value.details.parent_id &&
          !value.details.trashed &&
          !value.details.archived,
      )
      .sort((a, b) => a.details.rank - b.details.rank)
      .map((value) => value.id);
    const index = ids.indexOf(ticket.id),
      next = index + offset;
    if (next < 0 || next >= ids.length) return;
    [ids[index], ids[next]] = [ids[next], ids[index]];
    try {
      await client.queueCommand(
        {
          kind: "reorder_tickets",
          board_id: boardId,
          column_id: ticket.column_id,
          ids,
        },
        boardId,
        snapshot,
      );
    } catch (error) {
      setError(errorText(error));
    }
  }
  function pointerDrag(ticket: Ticket, event: PointerEvent) {
    event.preventDefault();
    setDragging(ticket.id);
    const end = (event: PointerEvent) => {
      const target = document
        .elementFromPoint(event.clientX, event.clientY)
        ?.closest<HTMLElement>("[data-drop-column]");
      if (target)
        void move(
          ticket,
          target.dataset.dropColumn === "backlog"
            ? null
            : (target.dataset.dropColumn ?? null),
        );
      setDragging("");
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", cancel);
    };
    const cancel = () => {
      setDragging("");
      window.removeEventListener("pointerup", end);
      window.removeEventListener("pointercancel", cancel);
    };
    window.addEventListener("pointerup", end, { once: true });
    window.addEventListener("pointercancel", cancel, { once: true });
  }
  async function request(command: Command) {
    if (!snapshot) return;
    try {
      const entry = await client.queueCommand(command, boardId, snapshot, true);
      if (entry) setPreview(entry);
    } catch (error) {
      setError(errorText(error));
    }
  }
  async function undo() {
    if (!toast) return;
    try {
      const preview = await client.api<{ token: string }>(`/undo/${boardId}`, {
        revision: toast.revision,
        count: 1,
      });
      const receipt = await client.api<Receipt>("/recovery/confirm", {
        token: preview.token,
        op_id: uuid(),
        client_id: profile,
      });
      await storage.acknowledge(
        {
          op_id: receipt.op_id,
          endpoint: "/recovery/confirm",
          body: {},
          action: { kind: "undo" },
          board_id: boardId,
          created_at: new Date().toISOString(),
          state: "pending",
        },
        {},
        receipt,
      );
      setToast(undefined);
      await refresh();
    } catch (error) {
      setError(errorText(error));
      setPanel("history");
    }
  }
  async function createBoard(event: Event) {
    event.preventDefault();
    if (!bootstrap) return;
    try {
      const entry = await client.queueCommand(
        { kind: "create_board", name: newBoardName, prefix: newPrefix },
        boardId,
        snapshot ?? emptySnapshot(bootstrap),
      );
      setPanel(undefined);
      setNewBoardName("");
      setNewPrefix("");
      if (entry?.action.local_id)
        await selectBoard(String(entry.action.local_id));
    } catch (error) {
      setError(errorText(error));
    }
  }

  const current = snapshot?.workspace;
  const tickets = current ? visibleTickets(current, view) : [];
  const roots = tickets.filter((ticket) => !ticket.details.parent_id);
  const columns =
    current?.columns.slice().sort((a, b) => a.position - b.position) ?? [];
  const backlogCount =
    current?.tickets.filter(
      (ticket) =>
        !ticket.column_id &&
        !ticket.details.parent_id &&
        !ticket.details.trashed &&
        !ticket.details.archived,
    ).length ?? 0;
  const editorTicket =
    current?.tickets.find((ticket) => ticket.id === editor?.id) ??
    editor?.fallback;
  const conflicts = entries.filter((entry) => entry.state !== "pending").length;

  return (
    <>
      <header class="app-header">
        <a class="brand" href="/" aria-label="Kanlo">
          kanlo<span>{t("Entwicklung", "Development")}</span>
        </a>
        <label class="board-picker">
          {t("Board", "Board")}
          <select
            aria-label={t("Board auswählen", "Select board")}
            value={boardId}
            onChange={(event) => selectBoard(event.currentTarget.value)}
          >
            {bootstrap?.boards.map((board) => (
              <option key={board.id} value={board.id}>
                {board.name}
              </option>
            ))}
          </select>
        </label>
        <div class="header-actions">
          <button
            class="icon-button"
            aria-label={t("Neues Board", "New board")}
            onClick={() => setPanel("board")}
          >
            ＋
          </button>
          <button
            class="secondary status-button"
            onClick={() => {
              if (client.connection.authRequired) setAuthDismissed(false);
              else setPanel("queue");
            }}
          >
            <span
              class={client.connection.online ? "online-dot" : "offline-dot"}
            />
            {client.connection.authRequired
              ? t("Anmelden", "Sign in")
              : entries.length
                ? `${entries.length} ${t("lokal", "local")}`
                : client.connection.online
                  ? t("Synchronisiert", "Synced")
                  : t("Offline", "Offline")}
            {conflicts > 0 && <b class="count">!</b>}
          </button>
          <button
            class="icon-button"
            aria-label={t("Einstellungen", "Settings")}
            disabled={!snapshot}
            onClick={() => setPanel("settings")}
          >
            ⚙
          </button>
        </div>
      </header>
      <main class="app-main">
        <div class="intro">
          <div>
            <p class="eyebrow">{t("DEIN ÜBERBLICK", "YOUR OVERVIEW")}</p>
            <h1>
              {current?.boards[0]?.name ??
                (loading
                  ? t("Kanlo wird geladen …", "Loading Kanlo…")
                  : "Kanlo")}
            </h1>
            <p>
              {t(
                "Ein Platz für die nächsten Schritte.",
                "A place for your next steps.",
              )}
            </p>
          </div>
          <button class="secondary" onClick={() => setPanel("statistics")}>
            {t("Übersicht & Verlauf", "Overview & history")}
          </button>
        </div>
        {pwa === "http" && (
          <p class="notice">
            {t(
              "LAN-HTTP funktioniert online. Für eine installierbare Offline-App auf anderen Geräten brauchst du vertrauenswürdiges HTTPS.",
              "LAN HTTP works online. An installable offline app on other devices requires trusted HTTPS.",
            )}
          </p>
        )}
        {snapshot && bootstrap && snapshot.epoch !== bootstrap.epoch && (
          <Message>
            {t(
              "Diese lokale Boardkopie stammt aus einem älteren Datenstand. Neue Änderungen werden einzeln zur Prüfung vorgemerkt.",
              "This cached board is from an older data snapshot. New edits are queued for individual review.",
            )}
          </Message>
        )}
        {error && <Message error>{error}</Message>}
        {(entries.length > 0 || !client.connection.online) && (
          <div class="sync-banner">
            <span>
              {client.connection.syncing
                ? t("Änderungen werden abgeglichen …", "Synchronizing edits…")
                : entries.length
                  ? `${entries.length} ${t("Änderungen auf diesem Browser gespeichert; noch nicht alle vom Server bestätigt.", "edits stored on this browser; not all acknowledged by the server yet.")}`
                  : t(
                      "Offline · geladene Boards bleiben verfügbar.",
                      "Offline · loaded boards remain available.",
                    )}
            </span>
            <button class="text-button" onClick={() => setPanel("queue")}>
              {conflicts
                ? t("Konflikte prüfen", "Review conflicts")
                : t("Details", "Details")}
            </button>
          </div>
        )}
        <div class="toolbar">
          <button
            class="backlog-button secondary"
            aria-expanded={backlog}
            aria-controls="backlog"
            onClick={() => setBacklog((value) => !value)}
          >
            ≡ Backlog <span>{backlogCount}</span>
          </button>
          <label class="search">
            <span class="sr-only">
              {t("Tickets durchsuchen", "Search tickets")}
            </span>
            <input
              type="search"
              placeholder={t(
                "Dieses Board durchsuchen …",
                "Search this board…",
              )}
              aria-label={t("Tickets durchsuchen", "Search tickets")}
              value={view.query}
              onInput={(event) =>
                changeView({ ...view, query: event.currentTarget.value })
              }
            />
          </label>
          <select
            aria-label={t("Statusfilter", "Status filter")}
            value={view.status}
            onChange={(event) =>
              changeView({ ...view, status: event.currentTarget.value })
            }
          >
            {[
              ["active", t("Aktive Tickets", "Active tickets")],
              [
                "all",
                t(
                  "Alle inkl. Archiv/Papierkorb",
                  "All including archive/trash",
                ),
              ],
              ["open", t("Offen", "Open")],
              ["done", t("Erledigt", "Done")],
              ["blocked", t("Blockiert", "Blocked")],
              ["overdue", t("Überfällig", "Overdue")],
              ["archive", t("Archiv", "Archive")],
              ["trash", t("Papierkorb", "Trash")],
            ].map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
          <select
            aria-label={t("Sortierung", "Sorting")}
            value={view.sort}
            onChange={(event) =>
              changeView({ ...view, sort: event.currentTarget.value })
            }
          >
            {[
              ["manual", t("Manuell", "Manual")],
              ["score", t("Gesamtscore", "Combined score")],
              ["importance", t("Priorität", "Importance")],
              ["title", t("Titel", "Title")],
              ["deadline", t("Deadline", "Deadline")],
              ["type", t("Typ", "Type")],
              ["created", t("Erstellt", "Created")],
              ["updated", t("Geändert", "Updated")],
              ["public_id", t("Kennung", "Identifier")],
              ...[
                ...new Set(
                  current?.tickets.flatMap((ticket) =>
                    Object.keys(ticket.details.custom_fields),
                  ) ?? [],
                ),
              ].map((name) => [`custom:${name}`, name]),
            ].map(([id, name]) => (
              <option key={id} value={id}>
                {name}
              </option>
            ))}
          </select>
          <button
            class="icon-button"
            aria-label={t(
              "Sortierrichtung umkehren",
              "Reverse sorting direction",
            )}
            onClick={() =>
              changeView({ ...view, descending: !view.descending })
            }
          >
            {view.descending ? "↓" : "↑"}
          </button>
          <button class="secondary" onClick={() => void refresh()}>
            {t("Aktualisieren", "Refresh")}
          </button>
        </div>
        <div class="filter-row">
          <label>
            {t("Typfilter", "Type filter")}
            <select
              value={view.type_id ?? ""}
              onChange={(event) =>
                changeView({
                  ...view,
                  type_id: event.currentTarget.value || null,
                })
              }
            >
              <option value="">{t("Alle Typen", "All types")}</option>
              {current?.types.map((type) => (
                <option key={type.id} value={type.id}>
                  {type.name}
                </option>
              ))}
            </select>
          </label>
          <details class="label-filter">
            <summary>
              {t("Labels filtern", "Filter labels")}{" "}
              {view.labels.length ? `(${view.labels.length})` : ""}
            </summary>
            <div class="label-options">
              {current?.labels.map((label) => (
                <label class="inline check" key={label.id}>
                  <input
                    type="checkbox"
                    checked={view.labels.includes(label.id)}
                    onChange={(event) =>
                      changeView({
                        ...view,
                        labels: event.currentTarget.checked
                          ? [...view.labels, label.id]
                          : view.labels.filter((id) => id !== label.id),
                      })
                    }
                  />
                  {label.name}
                </label>
              ))}
            </div>
          </details>
          <button
            class="text-button"
            onClick={() => changeView(defaultView(boardId))}
          >
            {t("Filter zurücksetzen", "Reset filters")}
          </button>
          <span class="muted">
            {roots.length} {t("Treffer", "matches")}
          </span>
          <button
            class="text-button"
            onClick={() => setSelected(roots.map((ticket) => ticket.id))}
          >
            {t("Suchtreffer auswählen", "Select matches")}
          </button>
          {selected.length > 0 && (
            <>
              <button class="secondary" onClick={() => setPanel("bulk")}>
                {t("Sammelaktion", "Bulk action")} ({selected.length})
              </button>
              <button class="text-button" onClick={() => setSelected([])}>
                {t("Auswahl aufheben", "Clear selection")}
              </button>
            </>
          )}
        </div>
        <details class="advanced-filters">
          <summary>{t("Weitere Filter", "More filters")}</summary>
          <div class="form-grid">
            <label>
              {t("Priorität ab", "Minimum importance")}
              <input
                type="number"
                min="0"
                max="255"
                value={view.importance_min ?? ""}
                onInput={(event) =>
                  changeView({
                    ...view,
                    importance_min:
                      event.currentTarget.value === ""
                        ? null
                        : Number(event.currentTarget.value),
                  })
                }
              />
            </label>
            <label>
              {t("Priorität bis", "Maximum importance")}
              <input
                type="number"
                min="0"
                max="255"
                value={view.importance_max ?? ""}
                onInput={(event) =>
                  changeView({
                    ...view,
                    importance_max:
                      event.currentTarget.value === ""
                        ? null
                        : Number(event.currentTarget.value),
                  })
                }
              />
            </label>
            <label>
              {t("Deadline ab", "Deadline from")}
              <input
                type="date"
                value={view.deadline_from ?? ""}
                onInput={(event) =>
                  changeView({
                    ...view,
                    deadline_from: event.currentTarget.value || null,
                  })
                }
              />
            </label>
            <label>
              {t("Deadline bis", "Deadline through")}
              <input
                type="date"
                value={view.deadline_to ?? ""}
                onInput={(event) =>
                  changeView({
                    ...view,
                    deadline_to: event.currentTarget.value || null,
                  })
                }
              />
            </label>
            {[
              ...new Set(
                current?.tickets.flatMap((ticket) =>
                  Object.keys(ticket.details.custom_fields),
                ) ?? [],
              ),
            ].map((name) => (
              <label key={name}>
                {name} · {t("enthält", "contains")}
                <input
                  value={view.custom_fields?.[name] ?? ""}
                  onInput={(event) => {
                    const fields = { ...view.custom_fields };
                    if (event.currentTarget.value)
                      fields[name] = event.currentTarget.value;
                    else delete fields[name];
                    changeView({ ...view, custom_fields: fields });
                  }}
                />
              </label>
            ))}
          </div>
        </details>
        {snapshot && (
          <form class="create-ticket" onSubmit={create}>
            <label for="new-title">
              {t("Neues Ticket im Backlog", "New ticket in backlog")}
            </label>
            <div class="input-row">
              <input
                id="new-title"
                required
                maxLength={300}
                value={title}
                onInput={(event) => setTitle(event.currentTarget.value)}
                placeholder={t("Was steht an?", "What is next?")}
              />
              <button disabled={!title.trim()}>
                {t("Ticket anlegen", "Create ticket")}
              </button>
            </div>
          </form>
        )}
        <p id="status" role="status" aria-live="polite">
          {loading ? (
            <Busy>{t("Board wird geladen …", "Loading board…")}</Busy>
          ) : (
            `${current?.tickets.filter((ticket) => !ticket.details.trashed && !ticket.details.archived).length ?? 0} ${t("Tickets", "tickets")}`
          )}
        </p>
        <div class={`board-layout ${backlog ? "with-backlog" : ""}`}>
          {backlog && (
            <section
              id="backlog"
              class="column backlog"
              aria-label="Backlog"
              data-drop-column="backlog"
              onDragOver={(event) => event.preventDefault()}
              onDrop={(event) => {
                event.preventDefault();
                const id = event.dataTransfer?.getData("text/kanlo-ticket");
                const ticket = current?.tickets.find(
                  (ticket) => ticket.id === id,
                );
                if (ticket) void move(ticket, null);
              }}
            >
              <h2>
                Backlog ·{" "}
                {roots.filter((ticket) => ticket.column_id === null).length}
              </h2>
              {renderTickets(null)}
            </section>
          )}
          <div class="board-columns">
            {columns.map((column) => {
              const amount =
                current?.tickets.filter(
                  (ticket) =>
                    ticket.column_id === column.id &&
                    !ticket.details.parent_id &&
                    !ticket.details.trashed &&
                    !ticket.details.archived,
                ).length ?? 0;
              return (
                <section
                  class="column"
                  key={column.id}
                  data-drop-column={column.id}
                  aria-label={column.name}
                  onDragOver={(event) => event.preventDefault()}
                  onDrop={(event) => {
                    event.preventDefault();
                    const id = event.dataTransfer?.getData("text/kanlo-ticket");
                    const ticket = current?.tickets.find(
                      (ticket) => ticket.id === id,
                    );
                    if (ticket) void move(ticket, column.id);
                  }}
                >
                  <h2>
                    <span
                      class="column-marker"
                      style={{
                        backgroundColor:
                          current?.board_options[boardId]?.column_colors[
                            column.id
                          ] ?? (column.is_done ? "#527f5e" : "#9eaa9c"),
                      }}
                    />
                    {column.name} ·{" "}
                    {
                      roots.filter((ticket) => ticket.column_id === column.id)
                        .length
                    }
                  </h2>
                  {column.wip_limit !== null && (
                    <p
                      class={`wip ${amount > column.wip_limit ? "warning-text" : ""}`}
                    >
                      WIP {amount}/{column.wip_limit} ·{" "}
                      {column.wip_strict
                        ? t("Sperre", "Strict")
                        : t("Warnung", "Warning")}
                    </p>
                  )}
                  {renderTickets(column.id)}
                </section>
              );
            })}
          </div>
        </div>
        {bootstrap && !bootstrap.boards.length && !loading && (
          <Message>
            {t(
              "Noch kein Board. Lege über ＋ ein neues Board an.",
              "No boards yet. Use ＋ to create a board.",
            )}
          </Message>
        )}
      </main>
      {toast && (
        <div class="undo-toast" role="status">
          <span>{t("Änderung synchronisiert", "Change synchronized")}</span>
          <button onClick={undo}>{t("Rückgängig", "Undo")}</button>
          <button
            class="icon-button"
            aria-label={t("Hinweis schließen", "Dismiss notification")}
            onClick={() => setToast(undefined)}
          >
            ×
          </button>
        </div>
      )}
      {editor && editorTicket && bootstrap && (
        <TicketDialog
          key={editor.token}
          ticket={editorTicket}
          snapshot={
            snapshot?.workspace.boards[0]?.id ===
            editor.source.workspace.boards[0]?.id
              ? snapshot
              : editor.source
          }
          bootstrap={bootstrap}
          close={() => setEditor(undefined)}
          open={open}
          preview={setPreview}
          refresh={() => {
            const id = active.current;
            void storage.boardCache(id).then((data) => {
              if (data && id === active.current) setSnapshot(data);
            });
          }}
        />
      )}
      {panel === "settings" && snapshot && bootstrap && (
        <SettingsDialog
          snapshot={snapshot}
          bootstrap={bootstrap}
          close={() => setPanel(undefined)}
          refresh={() => void refresh()}
          preview={setPreview}
          data={() => setPanel("data")}
          history={() => setPanel("history")}
        />
      )}
      {panel === "data" && snapshot && (
        <DataDialog
          snapshot={snapshot}
          close={() => setPanel(undefined)}
          refresh={() => void refresh()}
        />
      )}
      {panel === "history" && snapshot && (
        <HistoryDialog
          board={boardId}
          close={() => setPanel(undefined)}
          refresh={() => void refresh()}
        />
      )}
      {panel === "queue" && (
        <QueueDialog
          entries={entries}
          close={() => setPanel(undefined)}
          preview={setPreview}
          refreshed={() => void refresh()}
        />
      )}
      {preview && (
        <PreviewDialog
          entry={preview}
          close={() => setPreview(undefined)}
          refreshed={() => void refresh()}
        />
      )}
      {panel === "board" && (
        <Dialog
          title={t("Neues Board", "New board")}
          close={() => setPanel(undefined)}
        >
          <form onSubmit={createBoard}>
            <div class="dialog-body">
              <label>
                {t("Boardname", "Board name")}
                <input
                  required
                  maxLength={120}
                  value={newBoardName}
                  onInput={(event) =>
                    setNewBoardName(event.currentTarget.value)
                  }
                />
              </label>
              <label>
                {t(
                  "Dauerhaftes Kürzel (A–Z, maximal 12 Zeichen)",
                  "Permanent prefix (A–Z, up to 12 letters)",
                )}
                <input
                  required
                  pattern="[A-Z]{1,12}"
                  maxLength={12}
                  value={newPrefix}
                  onInput={(event) =>
                    setNewPrefix(event.currentTarget.value.toUpperCase())
                  }
                />
              </label>
            </div>
            <footer class="dialog-footer">
              <button>{t("Board anlegen", "Create board")}</button>
            </footer>
          </form>
        </Dialog>
      )}
      {panel === "bulk" && snapshot && (
        <BulkDialog
          selected={selected}
          snapshot={snapshot}
          close={() => setPanel(undefined)}
          execute={async (command) => {
            await request(command);
            setPanel(undefined);
            setSelected([]);
          }}
        />
      )}
      {panel === "statistics" && snapshot && (
        <StatisticsDialog
          snapshot={snapshot}
          close={() => setPanel(undefined)}
        />
      )}
      {client.connection.authRequired && !authDismissed && (
        <Dialog
          title={t("Externen Zugang anmelden", "Sign in to external access")}
          close={() => {
            setAuthDismissed(true);
            setLoginError(
              t(
                "Die Anmeldung ist für den Serverabgleich nötig. Lokale Änderungen bleiben erhalten.",
                "Sign-in is required for synchronization. Local edits are retained.",
              ),
            );
          }}
        >
          <form
            onSubmit={async (event) => {
              event.preventDefault();
              try {
                await client.authenticate(password);
                setPassword("");
                setLoginError("");
                await refresh();
                void client.flush();
              } catch (error) {
                setLoginError(errorText(error));
              }
            }}
          >
            <div class="dialog-body">
              <p>
                {t(
                  "Deine lokalen Änderungen bleiben bis zur erneuten Anmeldung gespeichert.",
                  "Local changes remain stored until you sign in again.",
                )}
              </p>
              {loginError && <Message error>{loginError}</Message>}
              <label>
                {t("Gemeinsamer Zugangscode", "Shared passcode")}
                <input
                  type="password"
                  autoComplete="current-password"
                  required
                  value={password}
                  onInput={(event) => setPassword(event.currentTarget.value)}
                />
              </label>
            </div>
            <footer class="dialog-footer">
              <button
                type="button"
                class="secondary"
                onClick={() => setAuthDismissed(true)}
              >
                {t("Lokal weiterarbeiten", "Continue locally")}
              </button>
              <button>{t("Anmelden", "Sign in")}</button>
            </footer>
          </form>
        </Dialog>
      )}
    </>
  );

  function renderTickets(column: string | null) {
    const shown = roots.filter((ticket) => ticket.column_id === column);
    if (!shown.length)
      return <p class="empty">{t("Noch keine Tickets", "No tickets yet")}</p>;
    return shown.map((ticket) => (
      <article
        class={`ticket ${dragging === ticket.id ? "dragging" : ""}`}
        key={ticket.id}
        draggable={!ticket.details.trashed && !ticket.details.archived}
        onDragStart={(event) => {
          event.dataTransfer?.setData("text/kanlo-ticket", ticket.id);
          setDragging(ticket.id);
        }}
        onDragEnd={() => setDragging("")}
        onDragOver={(event) => event.preventDefault()}
        onDrop={(event) => {
          event.preventDefault();
          event.stopPropagation();
          const id = event.dataTransfer?.getData("text/kanlo-ticket");
          const source = current?.tickets.find((value) => value.id === id);
          if (source) void move(source, column, ticket.id);
        }}
      >
        <div class="inline spread">
          <span class="ticket-id">{ticket.public_id}</span>
          <div class="inline">
            <input
              type="checkbox"
              aria-label={`${ticket.public_id} ${t("auswählen", "select")}`}
              checked={selected.includes(ticket.id)}
              onChange={(event) =>
                setSelected(
                  event.currentTarget.checked
                    ? [...selected, ticket.id]
                    : selected.filter((id) => id !== ticket.id),
                )
              }
            />
            <button
              class="drag-handle icon-button"
              aria-label={`${ticket.public_id} ${t("ziehen", "drag")}`}
              onPointerDown={(event) => pointerDrag(ticket, event)}
            >
              ⠿
            </button>
          </div>
        </div>
        <h3>
          <button class="ticket-title" onClick={() => open(ticket.id)}>
            {ticket.title}
          </button>
        </h3>
        <div class="ticket-meta">
          {ticket.details.type_id && (
            <span>
              {
                current?.types.find(
                  (type) => type.id === ticket.details.type_id,
                )?.name
              }
            </span>
          )}
          <span>P {ticket.importance}</span>
          {ticket.details.deadline && (
            <span
              class={
                !ticket.done &&
                deadlineTime(
                  ticket.details.deadline,
                  bootstrap?.settings.timezone ?? "Europe/Berlin",
                ) < Date.now()
                  ? "overdue"
                  : ""
              }
            >
              {ticket.details.deadline.includes("T")
                ? new Date(ticket.details.deadline).toLocaleString()
                : ticket.details.deadline}
            </span>
          )}
          {current && blocked(current, ticket.id) && (
            <span class="blocked">
              {t("Voraussetzung prüfen", "Check prerequisite")}
            </span>
          )}
          {ticket.id.startsWith("tmp:") && <span>{t("lokal", "local")}</span>}
        </div>
        <div class="ticket-labels">
          {current?.labels
            .filter((label) => ticket.details.labels.includes(label.id))
            .map((label) => (
              <span key={label.id}>
                <span
                  class="color-dot"
                  style={{ backgroundColor: label.color }}
                />
                {label.name}
              </span>
            ))}
        </div>
        {!!current?.tickets.filter(
          (child) =>
            child.details.parent_id === ticket.id && !child.details.trashed,
        ).length && (
          <p class="muted child-count">
            ✓{" "}
            {
              current.tickets.filter(
                (child) =>
                  child.details.parent_id === ticket.id &&
                  child.done &&
                  !child.details.trashed,
              ).length
            }
            /
            {
              current.tickets.filter(
                (child) =>
                  child.details.parent_id === ticket.id &&
                  !child.details.trashed,
              ).length
            }{" "}
            {t("Untertickets", "subtickets")}
          </p>
        )}
        {!ticket.details.trashed && !ticket.details.archived && (
          <label class="move-label">
            {t("Verschieben nach", "Move to")}
            <select
              aria-label={`${ticket.public_id} ${t("verschieben nach", "move to")}`}
              value={ticket.column_id ?? ""}
              onChange={(event) =>
                move(ticket, event.currentTarget.value || null)
              }
            >
              <option value="">Backlog</option>
              {columns.map((column) => (
                <option key={column.id} value={column.id}>
                  {column.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <div class="card-actions">
          <button class="text-button" onClick={() => open(ticket.id)}>
            {t("Details", "Details")}
          </button>
          {view.sort === "manual" && (
            <>
              <button
                class="icon-button"
                aria-label={`${ticket.public_id} ${t("nach oben", "move up")}`}
                onClick={() => reorder(ticket, -1)}
              >
                ↑
              </button>
              <button
                class="icon-button"
                aria-label={`${ticket.public_id} ${t("nach unten", "move down")}`}
                onClick={() => reorder(ticket, 1)}
              >
                ↓
              </button>
            </>
          )}
        </div>
      </article>
    ));
  }
}

function BulkDialog({
  selected,
  snapshot,
  close,
  execute,
}: {
  selected: string[];
  snapshot: Snapshot;
  close: () => void;
  execute: (command: Command) => Promise<void>;
}) {
  const t = useT();
  const [action, setAction] = useState("importance");
  const [importance, setImportance] = useState(100);
  const [column, setColumn] = useState("");
  const tickets = snapshot.workspace.tickets.filter((ticket) =>
    selected.includes(ticket.id),
  );
  const submit = async () => {
    let command: Command;
    if (action === "trash") command = { kind: "trash", ids: selected };
    else if (action === "archive" || action === "unarchive")
      command = {
        kind: "archive",
        ids: selected,
        archived: action === "archive",
      };
    else
      command = {
        kind: "batch",
        commands: tickets.map((ticket) =>
          action === "move"
            ? {
                kind: "move_ticket",
                id: ticket.id,
                base_revision: ticket.revision,
                board_id: ticket.board_id,
                column_id: column || null,
                activate_types: false,
                convert_types: {},
              }
            : {
                kind: "patch_ticket",
                id: ticket.id,
                base_revision: ticket.revision,
                fields: { importance },
              },
        ),
      };
    await execute(command);
  };
  return (
    <Dialog title={t("Sammelaktion", "Bulk action")} close={close}>
      <div class="dialog-body">
        <p>
          {selected.length}{" "}
          {t(
            "ausgewählte Tickets. Vor der Ausführung folgt eine bestätigungspflichtige Vorschau.",
            "selected tickets. A preview and confirmation are required before execution.",
          )}
        </p>
        <label>
          {t("Aktion", "Action")}
          <select
            value={action}
            onChange={(event) => setAction(event.currentTarget.value)}
          >
            <option value="importance">
              {t("Priorität setzen", "Set importance")}
            </option>
            <option value="move">{t("Verschieben", "Move")}</option>
            <option value="archive">{t("Archivieren", "Archive")}</option>
            <option value="unarchive">
              {t("Aus Archiv holen", "Unarchive")}
            </option>
            <option value="trash">{t("In Papierkorb", "Move to trash")}</option>
          </select>
        </label>
        {action === "importance" && (
          <label>
            {t("Neue Priorität (0–255)", "New importance (0–255)")}
            <input
              type="number"
              min="0"
              max="255"
              value={importance}
              onInput={(event) =>
                setImportance(Number(event.currentTarget.value))
              }
            />
          </label>
        )}
        {action === "move" && (
          <label>
            {t("Zielspalte", "Target column")}
            <select
              value={column}
              onChange={(event) => setColumn(event.currentTarget.value)}
            >
              <option value="">Backlog</option>
              {snapshot.workspace.columns.map((column) => (
                <option key={column.id} value={column.id}>
                  {column.name}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>
      <footer class="dialog-footer">
        <button class="secondary" onClick={close}>
          {t("Abbrechen", "Cancel")}
        </button>
        <button onClick={submit}>
          {t("Vorschau anfordern", "Request preview")}
        </button>
      </footer>
    </Dialog>
  );
}

function StatisticsDialog({
  snapshot,
  close,
}: {
  snapshot: Snapshot;
  close: () => void;
}) {
  const t = useT();
  const [days, setDays] = useState(30);
  const [global, setGlobal] = useState(false);
  const [samples, setSamples] = useState<
    {
      created_at: string;
      sample: {
        created: number;
        completed: number;
        backlog: number;
        blocked: number;
      };
    }[]
  >([]);
  const [error, setError] = useState("");
  useEffect(() => {
    const now = new Date();
    const from = new Date(Date.now() - days * 86400000);
    const query = new URLSearchParams({
      from: from.toISOString(),
      to: now.toISOString(),
    });
    if (!global) query.set("board_id", snapshot.workspace.boards[0].id);
    client
      .api<typeof samples>(`/statistics?${query}`)
      .then(setSamples)
      .catch((error) => setError(errorText(error)));
  }, [days, global]);
  const created = samples.reduce(
      (total, item) => total + item.sample.created,
      0,
    ),
    completed = samples.reduce(
      (total, item) => total + item.sample.completed,
      0,
    );
  const recent = samples.slice(-30);
  const maximum = Math.max(1, ...recent.map((item) => item.sample.backlog));
  return (
    <Dialog
      title={t("Übersicht und Verlauf", "Overview and history")}
      close={close}
      wide
    >
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        <div class="form-grid">
          <label>
            {t("Zeitraum in Tagen", "Time range in days")}
            <input
              type="number"
              min="1"
              max="36500"
              value={days}
              onInput={(event) => setDays(Number(event.currentTarget.value))}
            />
          </label>
          <label class="inline check">
            <input
              type="checkbox"
              checked={global}
              onChange={(event) => setGlobal(event.currentTarget.checked)}
            />
            {t("Alle Boards", "All boards")}
          </label>
        </div>
        <div class="metric-grid">
          <article>
            <strong>{created}</strong>
            <span>{t("Erstellt", "Created")}</span>
          </article>
          <article>
            <strong>{completed}</strong>
            <span>{t("Abgeschlossen", "Completed")}</span>
          </article>
          <article>
            <strong>
              {
                snapshot.workspace.tickets.filter(
                  (ticket) =>
                    !ticket.details.trashed &&
                    !ticket.details.archived &&
                    ticket.column_id === null,
                ).length
              }
            </strong>
            <span>Backlog · {t("aktuelles Board", "current board")}</span>
          </article>
        </div>
        <h3>
          {t(
            "Backlog-Verlauf (letzte Ereignisse)",
            "Backlog history (recent events)",
          )}
        </h3>
        {recent.length ? (
          <svg
            class="chart"
            viewBox="0 0 600 180"
            role="img"
            aria-label={t(
              "Historische Backlog-Anzahl",
              "Historical backlog count",
            )}
          >
            <polyline
              fill="none"
              stroke="currentColor"
              stroke-width="3"
              points={recent
                .map(
                  (item, index) =>
                    `${20 + (index / Math.max(1, recent.length - 1)) * 560},${160 - (item.sample.backlog / maximum) * 140}`,
                )
                .join(" ")}
            />
          </svg>
        ) : (
          <Message>
            {t(
              "Für diesen Zeitraum liegen noch keine historischen Daten vor.",
              "No historical data for this period yet.",
            )}
          </Message>
        )}
        <p class="muted">
          {t(
            "Historische Daten werden unabhängig vom Undo-Verlauf gespeichert.",
            "Historical statistics are stored independently of undo history.",
          )}
        </p>
      </div>
    </Dialog>
  );
}
