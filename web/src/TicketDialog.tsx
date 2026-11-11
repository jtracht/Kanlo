import { useEffect, useRef, useState } from "preact/hooks";
import type { Bootstrap, Command, QueueEntry, Snapshot, Ticket } from "./types";
import { fields } from "./types";
import {
  addComment,
  api,
  createTicket,
  patchTicket,
  queueCommand,
  connection,
  loadBoard,
} from "./client";
import {
  Dialog,
  Markdown,
  Message,
  confirmDiscard,
  errorText,
  useT,
  useUnsavedGuard,
} from "./ui";
import { uuid } from "./identifiers";
import * as storage from "./storage";

interface Props {
  ticket: Ticket;
  snapshot: Snapshot;
  bootstrap: Bootstrap;
  close: () => void;
  open: (id: string) => void;
  preview: (entry: QueueEntry) => void;
  refresh: () => void;
}
interface Draft {
  title: string;
  description: string;
  importance: number;
  type_id: string;
  labels: string[];
  custom: { key: string; name: string; value: string }[];
  date: string;
  time: string;
}
function dateParts(value: string | null, timezone: string) {
  if (!value) return { date: "", time: "" };
  if (!value.includes("T")) return { date: value, time: "" };
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-CA", {
      timeZone: timezone,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    })
      .formatToParts(new Date(value))
      .map((part) => [part.type, part.value]),
  );
  return {
    date: `${parts.year}-${parts.month}-${parts.day}`,
    time: `${parts.hour}:${parts.minute}`,
  };
}
function dateValue(date: string, time: string, timezone: string) {
  if (!date) return null;
  if (!time) return date;
  const wanted = `${date}T${time}`;
  const formatter = new Intl.DateTimeFormat("en-CA", {
    timeZone: timezone,
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    hourCycle: "h23",
  });
  const local = (time: number) => {
    const parts = Object.fromEntries(
      formatter.formatToParts(time).map((part) => [part.type, part.value]),
    );
    return `${parts.year}-${parts.month}-${parts.day}T${parts.hour}:${parts.minute}`;
  };
  const target = Date.parse(`${wanted}:00Z`);
  let guess = target;
  for (let i = 0; i < 4; i++)
    guess += target - Date.parse(`${local(guess)}:00Z`);
  const candidates: number[] = [];
  for (let offset = -180; offset <= 180; offset += 15) {
    const candidate = guess + offset * 60000;
    if (local(candidate) === wanted) candidates.push(candidate);
  }
  if (!candidates.length)
    throw new Error(
      "Diese Uhrzeit existiert an diesem Datum nicht / This local time does not exist on that date.",
    );
  return new Date(Math.min(...candidates)).toISOString();
}
function draftFor(ticket: Ticket, timezone: string): Draft {
  return {
    title: ticket.title,
    description: ticket.description,
    importance: ticket.importance,
    type_id: ticket.details.type_id ?? "",
    labels: [...ticket.details.labels].sort(),
    custom: Object.entries(ticket.details.custom_fields).map(
      ([name, value]) => ({ key: uuid(), name, value }),
    ),
    ...dateParts(ticket.details.deadline, timezone),
  };
}

export function TicketDialog({
  ticket,
  snapshot,
  bootstrap,
  close,
  open,
  preview,
  refresh,
}: Props) {
  const t = useT();
  const base = useRef(structuredClone(ticket));
  const epoch = useRef(snapshot.epoch);
  const [draft, setDraft] = useState(() =>
    draftFor(ticket, bootstrap.settings.timezone),
  );
  const initialDraft = useRef(JSON.stringify(draft));
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [showPreview, setShowPreview] = useState(false);
  const [comment, setComment] = useState("");
  const [childTitle, setChildTitle] = useState("");
  const [reference, setReference] = useState("");
  const [referenceKind, setReferenceKind] = useState("dependency");
  const [targetBoard, setTargetBoard] = useState(ticket.board_id);
  const [targetColumn, setTargetColumn] = useState(ticket.column_id ?? "");
  const [target, setTarget] = useState<Snapshot>(snapshot);
  const [typeDecision, setTypeDecision] = useState<"activate" | "convert" | "">(
    "",
  );
  const [convertType, setConvertType] = useState("");
  const dirty =
    JSON.stringify(draft) !== initialDraft.current || !!comment || !!childTitle;
  useUnsavedGuard(dirty);
  const state = snapshot.workspace;
  const root = ticket.details.parent_id === null;
  const children = state.tickets.filter(
    (child) => child.details.parent_id === ticket.id && !child.details.trashed,
  );
  const comments = state.comments
    .filter((comment) => comment.ticket_id === ticket.id)
    .sort((a, b) => a.created_at.localeCompare(b.created_at));
  const dependencies = state.dependencies.filter(
    (edge) => edge.ticket_id === ticket.id,
  );
  const links = state.links.filter((link) => link.ticket_id === ticket.id);
  const enabled = state.board_options[ticket.board_id]?.enabled_types ?? [];
  const allowedTypes = state.types.filter((type) => enabled.includes(type.id));
  useEffect(() => {
    if (base.current.id.startsWith("tmp:") && !ticket.id.startsWith("tmp:"))
      base.current.id = ticket.id;
  }, [ticket.id]);
  function set<K extends keyof Draft>(key: K, value: Draft[K]) {
    setDraft((previous) => ({ ...previous, [key]: value }));
    setMessage("");
  }
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
      refresh();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function save(event: Event) {
    event.preventDefault();
    await run(async () => {
      const names = draft.custom.map((field) => field.name.trim());
      if (names.some((name) => !name) || new Set(names).size !== names.length)
        throw new Error(
          t(
            "Textfelder brauchen eindeutige, nicht leere Namen.",
            "Custom fields need unique, nonempty names.",
          ),
        );
      if (new TextEncoder().encode(draft.description).length > 65536)
        throw new Error(
          t(
            "Beschreibung ist größer als 64 KiB.",
            "Description exceeds 64 KiB.",
          ),
        );
      const initial = JSON.parse(initialDraft.current) as Draft;
      const deadline =
        draft.date === initial.date && draft.time === initial.time
          ? base.current.details.deadline
          : dateValue(draft.date, draft.time, bootstrap.settings.timezone);
      const values = {
        title: draft.title.trim(),
        description: draft.description,
        importance: draft.importance,
        type_id: draft.type_id || null,
        labels: [...draft.labels].sort(),
        custom_fields: Object.fromEntries(
          draft.custom.map((field) => [field.name.trim(), field.value]),
        ),
        deadline,
      };
      const original = fields(base.current);
      const changed = Object.fromEntries(
        Object.entries(values).filter(
          ([key, value]) =>
            JSON.stringify(value) !==
            JSON.stringify(original[key as keyof typeof original]),
        ),
      );
      if (Object.keys(changed).length) {
        await patchTicket(
          { ...base.current, id: ticket.id },
          changed,
          original,
          epoch.current,
        );
        base.current = {
          ...base.current,
          id: ticket.id,
          title: values.title,
          description: values.description,
          importance: values.importance,
          details: {
            ...base.current.details,
            type_id: values.type_id,
            labels: values.labels,
            custom_fields: values.custom_fields,
            deadline,
          },
        };
        initialDraft.current = JSON.stringify(draft);
        setMessage(
          t(
            "Dauerhaft lokal gespeichert; Synchronisierung folgt.",
            "Saved durably on this device; synchronization follows.",
          ),
        );
      } else setMessage(t("Keine Änderung.", "No change."));
    });
  }
  async function changeBoard(id: string) {
    setTargetBoard(id);
    setTypeDecision("");
    try {
      const board = await (connection.online
        ? loadBoard(id)
        : storage.boardCache(id));
      if (!board)
        throw new Error(
          t(
            "Dieses Board wurde noch nicht geladen. Bitte zuerst online besuchen.",
            "This board has not been loaded. Visit it online first.",
          ),
        );
      setTarget(board);
      const currentColumn = state.columns.find(
        (column) => column.id === ticket.column_id,
      )?.name;
      setTargetColumn(
        board.workspace.columns.find((column) => column.name === currentColumn)
          ?.id ?? "",
      );
    } catch (error) {
      setError(errorText(error));
    }
  }
  const familyTypes = [ticket, ...children]
    .map((member) => member.details.type_id)
    .filter((id): id is string => !!id);
  const missingTypes = familyTypes.filter(
    (id) =>
      !target.workspace.board_options[targetBoard]?.enabled_types.includes(id),
  );
  async function move() {
    await run(async () => {
      if (missingTypes.length && !typeDecision)
        throw new Error(
          t(
            "Typ auf dem Zielboard aktivieren oder bewusst konvertieren.",
            "Activate the type on the target board or explicitly convert it.",
          ),
        );
      if (missingTypes.length) {
        const conversions =
          typeDecision === "convert"
            ? Object.fromEntries(
                [ticket, ...children]
                  .filter(
                    (member) =>
                      member.details.type_id &&
                      missingTypes.includes(member.details.type_id),
                  )
                  .map((member) => [member.id, convertType || null]),
              )
            : {};
        await queueCommand(
          {
            kind: "move_ticket",
            id: ticket.id,
            base_revision: ticket.revision,
            board_id: targetBoard,
            column_id: targetColumn || null,
            activate_types: typeDecision === "activate",
            convert_types: conversions,
          },
          ticket.board_id,
          snapshot,
        );
      } else
        await patchTicket(
          { ...base.current, id: ticket.id },
          { board_id: targetBoard, column_id: targetColumn || null },
        );
      setMessage(t("Verschieben lokal vorgemerkt.", "Move saved locally."));
    });
  }
  async function destructive(command: Command) {
    const entry = await queueCommand(command, ticket.board_id, snapshot, true);
    if (entry) preview(entry);
  }
  async function addReference() {
    await run(async () => {
      const known = state.tickets.find(
        (value) =>
          value.public_id.toLowerCase() === reference.trim().toLowerCase() ||
          value.id === reference.trim(),
      );
      const target =
        known ??
        (await api<Ticket>(`/tickets/${encodeURIComponent(reference.trim())}`));
      await queueCommand(
        referenceKind === "dependency"
          ? {
              kind: "add_dependency",
              ticket_id: ticket.id,
              prerequisite_id: target.id,
            }
          : { kind: "add_link", ticket_id: ticket.id, target_id: target.id },
        ticket.board_id,
        snapshot,
      );
      setReference("");
    });
  }
  function name(id: string) {
    const target =
      state.tickets.find((ticket) => ticket.id === id) ??
      snapshot.external_references.find((ticket) => ticket.id === id);
    return target?.public_id ?? id;
  }
  const requestClose = () => {
    if (confirmDiscard(dirty, t)) close();
  };
  return (
    <Dialog
      title={`${ticket.public_id} · ${t("Ticketdetails", "Ticket details")}`}
      close={requestClose}
      wide
    >
      <form onSubmit={save}>
        <div class="dialog-body ticket-editor">
          <div class="inline wrap">
            <span class="badge">
              {ticket.done ? t("Erledigt", "Done") : t("Offen", "Open")}
            </span>
            {ticket.id.startsWith("tmp:") && (
              <span class="badge">
                {t(
                  "Kennung wird beim Abgleich vergeben",
                  "Permanent ID assigned on synchronization",
                )}
              </span>
            )}
            {ticket.details.trashed && (
              <span class="badge danger-text">
                {t("Im Papierkorb", "In trash")}
              </span>
            )}
            {ticket.details.archived && (
              <span class="badge">{t("Archiviert", "Archived")}</span>
            )}
          </div>
          {error && <Message error>{error}</Message>}
          {message && <Message>{message}</Message>}
          <label>
            {t("Titel", "Title")}
            <input
              required
              maxLength={300}
              value={draft.title}
              onInput={(event) => set("title", event.currentTarget.value)}
            />
          </label>
          <div class="form-grid">
            <label>
              {t("Priorität (0–255)", "Importance (0–255)")}
              <input
                type="number"
                min="0"
                max="255"
                required
                value={draft.importance}
                onInput={(event) =>
                  set("importance", Number(event.currentTarget.value))
                }
              />
            </label>
            <label>
              {t("Tickettyp", "Ticket type")}
              <select
                value={draft.type_id}
                onChange={(event) => set("type_id", event.currentTarget.value)}
              >
                <option value="">{t("Kein Typ", "No type")}</option>
                {allowedTypes.map((type) => (
                  <option
                    key={type.id}
                    value={type.id}
                    disabled={children.length > 0 && !type.allow_children}
                  >
                    {type.name}
                  </option>
                ))}
              </select>
            </label>
            <label>
              {t("Deadline: Datum", "Deadline: date")}
              <input
                type="date"
                value={draft.date}
                onInput={(event) => set("date", event.currentTarget.value)}
              />
            </label>
            <label>
              {t("Uhrzeit (optional)", "Time (optional)")}
              <input
                type="time"
                disabled={!draft.date}
                value={draft.time}
                onInput={(event) => set("time", event.currentTarget.value)}
              />
              <small>{bootstrap.settings.timezone}</small>
            </label>
          </div>
          <fieldset>
            <legend>{t("Globale Labels", "Global labels")}</legend>
            <div class="label-options">
              {state.labels.map((label) => (
                <label class="inline check" key={label.id}>
                  <input
                    type="checkbox"
                    checked={draft.labels.includes(label.id)}
                    onChange={(event) =>
                      set(
                        "labels",
                        event.currentTarget.checked
                          ? [...draft.labels, label.id]
                          : draft.labels.filter((id) => id !== label.id),
                      )
                    }
                  />
                  <span
                    class="color-dot"
                    style={{ backgroundColor: label.color }}
                  />
                  {label.name}
                </label>
              ))}
              {!state.labels.length && (
                <small>
                  {t(
                    "Noch keine Labels angelegt.",
                    "No labels have been created yet.",
                  )}
                </small>
              )}
            </div>
          </fieldset>
          <div class="inline spread">
            <label for="description">
              {t("Beschreibung (Markdown)", "Description (Markdown)")}
            </label>
            <button
              type="button"
              class="text-button"
              onClick={() => setShowPreview((value) => !value)}
            >
              {showPreview ? t("Bearbeiten", "Edit") : t("Vorschau", "Preview")}
            </button>
          </div>
          {showPreview ? (
            <section class="markdown-preview">
              <Markdown text={draft.description} />
            </section>
          ) : (
            <textarea
              id="description"
              rows={8}
              value={draft.description}
              onInput={(event) => set("description", event.currentTarget.value)}
            />
          )}
          <fieldset>
            <legend>{t("Eigene Textfelder", "Custom text fields")}</legend>
            <p class="muted">
              {t(
                "Unabhängig vom Tickettyp. Namen und Werte sind frei wählbar.",
                "Independent of ticket type. Choose any field names and text values.",
              )}
            </p>
            {draft.custom.map((field, index) => (
              <div class="custom-field" key={field.key}>
                <input
                  aria-label={t("Feldname", "Field name")}
                  maxLength={120}
                  value={field.name}
                  onInput={(event) =>
                    set(
                      "custom",
                      draft.custom.map((item, i) =>
                        i === index
                          ? { ...item, name: event.currentTarget.value }
                          : item,
                      ),
                    )
                  }
                />
                <input
                  aria-label={t("Textwert", "Text value")}
                  maxLength={16384}
                  value={field.value}
                  onInput={(event) =>
                    set(
                      "custom",
                      draft.custom.map((item, i) =>
                        i === index
                          ? { ...item, value: event.currentTarget.value }
                          : item,
                      ),
                    )
                  }
                />
                <button
                  type="button"
                  class="icon-button"
                  aria-label={t("Textfeld entfernen", "Remove custom field")}
                  onClick={() =>
                    set(
                      "custom",
                      draft.custom.filter((_, i) => i !== index),
                    )
                  }
                >
                  ×
                </button>
              </div>
            ))}
            <button
              type="button"
              class="secondary"
              onClick={() =>
                set("custom", [
                  ...draft.custom,
                  { key: uuid(), name: "", value: "" },
                ])
              }
            >
              {t("Textfeld hinzufügen", "Add text field")}
            </button>
          </fieldset>
          <section>
            <h3>
              {t(
                "Voraussetzungen und Verknüpfungen",
                "Prerequisites and links",
              )}
            </h3>
            <ul class="relation-list">
              {dependencies.map((edge) => (
                <li key={edge.prerequisite_id}>
                  {t("Benötigt", "Requires")}{" "}
                  <button
                    type="button"
                    class="text-button"
                    onClick={() => {
                      if (confirmDiscard(dirty, t)) open(edge.prerequisite_id);
                    }}
                  >
                    {name(edge.prerequisite_id)}
                  </button>
                  <button
                    type="button"
                    class="icon-button"
                    aria-label={t(
                      "Voraussetzung entfernen",
                      "Remove prerequisite",
                    )}
                    onClick={() =>
                      run(async () => {
                        await queueCommand(
                          { kind: "remove_dependency", ...edge },
                          ticket.board_id,
                          snapshot,
                        );
                      })
                    }
                  >
                    ×
                  </button>
                </li>
              ))}
              {links.map((link) => (
                <li key={link.target_id}>
                  {t("Verknüpft mit", "Linked to")}{" "}
                  <button
                    type="button"
                    class="text-button"
                    onClick={() => {
                      if (confirmDiscard(dirty, t)) open(link.target_id);
                    }}
                  >
                    {name(link.target_id)}
                  </button>
                  <button
                    type="button"
                    class="icon-button"
                    aria-label={t("Verknüpfung entfernen", "Remove link")}
                    onClick={() =>
                      run(async () => {
                        await queueCommand(
                          { kind: "remove_link", ...link },
                          ticket.board_id,
                          snapshot,
                        );
                      })
                    }
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
            <div class="input-row">
              <select
                aria-label={t("Beziehungsart", "Relationship type")}
                value={referenceKind}
                onChange={(event) =>
                  setReferenceKind(event.currentTarget.value)
                }
              >
                <option value="dependency">
                  {t("Voraussetzung", "Prerequisite")}
                </option>
                <option value="link">{t("Verknüpfung", "Link")}</option>
              </select>
              <input
                aria-label={t(
                  "Ticketkennung der Beziehung",
                  "Related ticket ID",
                )}
                value={reference}
                placeholder="DEV-42"
                onInput={(event) => setReference(event.currentTarget.value)}
              />
              <button
                type="button"
                class="secondary"
                disabled={!reference.trim() || busy}
                onClick={addReference}
              >
                {t("Hinzufügen", "Add")}
              </button>
            </div>
          </section>
          {root && (
            <section>
              <h3>
                {t("Untertickets", "Subtickets")} ·{" "}
                {children.filter((child) => child.done).length}/
                {children.length}
              </h3>
              {children.map((child) => (
                <div class="child-row" key={child.id}>
                  <label class="inline check">
                    <input
                      type="checkbox"
                      disabled={ticket.done && child.done}
                      checked={child.done}
                      onChange={(event) =>
                        run(() =>
                          patchTicket(child, {
                            done: event.currentTarget.checked,
                          }),
                        )
                      }
                    />
                    {child.public_id}
                  </label>
                  <button
                    type="button"
                    class="text-button"
                    onClick={() => {
                      if (confirmDiscard(dirty, t)) open(child.id);
                    }}
                  >
                    {child.title}
                  </button>
                </div>
              ))}
              <div class="input-row">
                <input
                  aria-label={t("Neues Unterticket", "New subticket")}
                  value={childTitle}
                  maxLength={300}
                  onInput={(event) => setChildTitle(event.currentTarget.value)}
                />
                <button
                  type="button"
                  class="secondary"
                  disabled={
                    !childTitle.trim() ||
                    busy ||
                    !!state.types.find(
                      (type) =>
                        type.id === draft.type_id && !type.allow_children,
                    )
                  }
                  onClick={() =>
                    run(async () => {
                      await createTicket(
                        {
                          kind: "create_ticket",
                          title: childTitle.trim(),
                          parent_id: ticket.id,
                        },
                        ticket.board_id,
                      );
                      setChildTitle("");
                    })
                  }
                >
                  {t("Unterticket anlegen", "Create subticket")}
                </button>
              </div>
            </section>
          )}
          {!root && (
            <p>
              <button
                type="button"
                class="text-button"
                onClick={() => {
                  if (confirmDiscard(dirty, t)) open(ticket.details.parent_id!);
                }}
              >
                {t("Zum Elternticket", "Open parent ticket")}
              </button>{" "}
              ·{" "}
              {t(
                "Untertickets wechseln Board und Spalte gemeinsam mit dem Elternteil.",
                "Subtickets move between boards and columns with their parent.",
              )}
            </p>
          )}
          <section>
            <h3>{t("Kommentare", "Comments")}</h3>
            {comments.map((comment) => (
              <article class="comment" key={comment.id}>
                <div class="inline spread">
                  <time dateTime={comment.created_at}>
                    {new Date(comment.created_at).toLocaleString()}
                  </time>
                  <button
                    type="button"
                    class="text-button danger-text"
                    onClick={() =>
                      destructive({ kind: "delete_comment", id: comment.id })
                    }
                  >
                    {t("Löschen", "Delete")}
                  </button>
                </div>
                <Markdown text={comment.markdown} />
              </article>
            ))}
            <label>
              {t("Neuer Kommentar (Markdown)", "New comment (Markdown)")}
              <textarea
                rows={3}
                value={comment}
                onInput={(event) => setComment(event.currentTarget.value)}
              />
            </label>
            <button
              type="button"
              class="secondary"
              disabled={!comment.trim() || busy}
              onClick={() =>
                run(async () => {
                  await addComment(ticket, comment);
                  setComment("");
                })
              }
            >
              {t("Kommentar speichern", "Save comment")}
            </button>
          </section>
          {root && (
            <details>
              <summary>
                {t("Board und Spalte ändern", "Change board and column")}
              </summary>
              <div class="form-grid">
                <label>
                  {t("Zielboard", "Target board")}
                  <select
                    value={targetBoard}
                    onChange={(event) => changeBoard(event.currentTarget.value)}
                  >
                    {bootstrap.boards.map((board) => (
                      <option key={board.id} value={board.id}>
                        {board.name}
                      </option>
                    ))}
                  </select>
                </label>
                <label>
                  {t("Zielspalte", "Target column")}
                  <select
                    value={targetColumn}
                    onChange={(event) =>
                      setTargetColumn(event.currentTarget.value)
                    }
                  >
                    <option value="">Backlog</option>
                    {target.workspace.columns.map((column) => (
                      <option key={column.id} value={column.id}>
                        {column.name}
                      </option>
                    ))}
                  </select>
                </label>
              </div>
              {!!missingTypes.length && (
                <fieldset>
                  <legend>
                    {t(
                      "Tickettyp fehlt auf dem Zielboard",
                      "Ticket type missing on target board",
                    )}
                  </legend>
                  <label class="inline check">
                    <input
                      type="radio"
                      name="type-decision"
                      checked={typeDecision === "activate"}
                      onChange={() => setTypeDecision("activate")}
                    />
                    {t(
                      "Benötigte Typen dort aktivieren",
                      "Activate required types there",
                    )}
                  </label>
                  <label class="inline check">
                    <input
                      type="radio"
                      name="type-decision"
                      checked={typeDecision === "convert"}
                      onChange={() => setTypeDecision("convert")}
                    />
                    {t(
                      "Betroffene Tickets ausdrücklich konvertieren",
                      "Explicitly convert affected tickets",
                    )}
                  </label>
                  {typeDecision === "convert" && (
                    <select
                      aria-label={t("Zieltyp", "Target type")}
                      value={convertType}
                      onChange={(event) =>
                        setConvertType(event.currentTarget.value)
                      }
                    >
                      <option value="">{t("Ohne Typ", "No type")}</option>
                      {target.workspace.types
                        .filter((type) =>
                          target.workspace.board_options[
                            targetBoard
                          ]?.enabled_types.includes(type.id),
                        )
                        .map((type) => (
                          <option key={type.id} value={type.id}>
                            {type.name}
                          </option>
                        ))}
                    </select>
                  )}
                </fieldset>
              )}
              <button
                type="button"
                class="secondary"
                disabled={busy || !!ticket.details.parent_id}
                onClick={move}
              >
                {t("Mit Untertickets verschieben", "Move with subtickets")}
              </button>
            </details>
          )}
          <div class="actions wrap">
            <button type="button" class="secondary"
              disabled={busy || !connection.online || connection.authRequired || ticket.details.trashed || ticket.details.archived}
              onClick={() => run(async () => {
                await api("/display", { kind: "show_ticket", ticket_id: ticket.id });
                setMessage(t("Ticket auf TV angezeigt.", "Ticket sent to TV."));
              })}>
              {t("Auf TV anzeigen", "Show on TV")}
            </button>
            <button type="button" class="secondary"
              disabled={busy || !connection.online || connection.authRequired || ticket.details.trashed || ticket.details.archived}
              onClick={() => run(async () => {
                await api("/display", { kind: "highlight", ticket_ids: [ticket.id], seconds: 25 });
                setMessage(t("Ticket auf dem TV-Board hervorgehoben.", "Ticket highlighted on the TV board."));
              })}>
              {t("Auf TV hervorheben", "Highlight on TV")}
            </button>
            {root && (
              <button type="button" class="secondary"
                disabled={busy || !connection.online || connection.authRequired || ticket.details.trashed || ticket.details.archived}
                onClick={() => run(async () => {
                  await api("/display", { kind: "show_children", ticket_id: ticket.id });
                  setMessage(t("Untertickets auf TV angezeigt.", "Subtickets sent to TV."));
                })}>
                {t("Untertickets auf TV anzeigen", "Show subtickets on TV")}
              </button>
            )}
            <button
              type="button"
              class="secondary"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  await queueCommand(
                    { kind: "clone_ticket", id: ticket.id },
                    ticket.board_id,
                    snapshot,
                  );
                })
              }
            >
              {t("Klonen", "Clone")}
            </button>
            {root && (
              <button
                type="button"
                class="secondary"
                disabled={busy || !ticket.done}
                onClick={() =>
                  run(() =>
                    patchTicket(ticket, { archived: !ticket.details.archived }),
                  )
                }
              >
                {ticket.details.archived
                  ? t("Aus Archiv holen", "Unarchive")
                  : t("Archivieren", "Archive")}
              </button>
            )}
            {ticket.details.trashed ? (
              <button
                type="button"
                class="secondary"
                onClick={() =>
                  run(async () => {
                    await queueCommand(
                      { kind: "restore", ids: [ticket.id] },
                      ticket.board_id,
                      snapshot,
                    );
                  })
                }
              >
                {t("Wiederherstellen", "Restore")}
              </button>
            ) : (
              <button
                type="button"
                class="danger secondary"
                onClick={() => destructive({ kind: "trash", ids: [ticket.id] })}
              >
                {t("In Papierkorb", "Move to trash")}
              </button>
            )}
          </div>
        </div>
        <footer class="dialog-footer">
          <span class="muted">
            {dirty
              ? t("Ungespeicherte Eingaben", "Unsaved input")
              : t("Keine ungespeicherten Eingaben", "No unsaved input")}
          </span>
          <button type="button" class="secondary" onClick={requestClose}>
            {t("Schließen", "Close")}
          </button>
          <button disabled={busy || ticket.details.trashed} type="submit">
            {busy ? t("Speichern …", "Saving…") : t("Speichern", "Save")}
          </button>
        </footer>
      </form>
    </Dialog>
  );
}
