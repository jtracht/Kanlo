import { useEffect, useRef, useState } from "preact/hooks";
import { api } from "./client";
import { Dialog, Message, errorText, useT } from "./ui";

export type Entry = { id: string; kind: "sprint" | "milestone"; title: string; start: string | null; end: string; milestone_id?: string | null };
export type PlannedTicket = { id: string; public_id: string; title: string; in_backlog: boolean; entry_id: string | null };
type GiteaMilestone = { id: number; title: string; end: string | null; closed: boolean };
type GiteaCalendar = { configured: boolean; milestones: GiteaMilestone[] };
const calendarPath = "/calendar";
const monthStart = (date: Date, offset = 0) => new Date(date.getFullYear(), date.getMonth() + offset, 1);
const dayKey = (date: Date) => [date.getFullYear(), date.getMonth() + 1, date.getDate()]
  .map((number, index) => String(number).padStart(index === 0 ? 4 : 2, "0")).join("-");

export function CalendarDialog({ close }: { close: () => void }) {
  const t = useT();
  const locale = t("de", "en");
  const [month, setMonth] = useState(() => monthStart(new Date()));
  const [selectedDay, setSelectedDay] = useState("");
  const [entries, setEntries] = useState<Entry[]>([]);
  const [gitea, setGitea] = useState<GiteaCalendar>();
  const [giteaError, setGiteaError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [kind, setKind] = useState<Entry["kind"]>("sprint");
  const [title, setTitle] = useState("");
  const [start, setStart] = useState("");
  const [end, setEnd] = useState("");
  const [parent, setParent] = useState("");
  const [tickets, setTickets] = useState<PlannedTicket[]>([]);
  const [ticketId, setTicketId] = useState("");
  const [entryId, setEntryId] = useState("");
  const titleField = useRef<HTMLInputElement>(null);

  function refreshGitea() {
    setGiteaError("");
    void api<GiteaCalendar>(calendarPath + "/gitea").then(setGitea)
      .catch((reason) => setGiteaError(errorText(reason)));
  }
  useEffect(() => {
    void api<Entry[]>(calendarPath).then(setEntries).catch((reason) => setError(errorText(reason)));
    void api<PlannedTicket[]>(calendarPath + "/tickets").then(setTickets).catch((reason) => setError(errorText(reason)));
    refreshGitea();
  }, []);

  async function save() {
    setBusy(true);
    setError("");
    try {
      const entry = await api<Entry>(calendarPath, { kind, title, start: kind === "sprint" ? start : null, end,
        milestone_id: kind === "sprint" && parent ? parent : null });
      setEntries((existing) => [...existing, entry]);
      setMonth(monthStart(new Date(entry.end + "T12:00:00")));
      setSelectedDay("");
      setTitle("");
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  const next = monthStart(month, 1);
  const days = new Date(next.getFullYear(), next.getMonth(), 0).getDate();
  const first = dayKey(month);
  const boundary = dayKey(next);
  const events = [
    ...entries.map((entry) => ({ ...entry, source: "Kanlo", closed: false })),
    ...(gitea?.milestones ?? []).filter((item) => item.end).map((item) => ({
      ...item, id: String(item.id), end: item.end!, start: null, source: "Gitea", kind: "milestone" as const,
    })),
  ].filter((entry) => (entry.start ?? entry.end) < boundary && entry.end >= first)
    .sort((a, b) => a.end.localeCompare(b.end) || a.title.localeCompare(b.title));
  const includes = (entry: typeof events[number], day: string) => (entry.start ?? entry.end) <= day && day <= entry.end;
  const visible = selectedDay ? events.filter((entry) => includes(entry, selectedDay)) : events;
  const monthName = new Intl.DateTimeFormat(locale, { month: "long", year: "numeric" }).format(month);
  const weekdays = t("Mo Di Mi Do Fr Sa So", "Mon Tue Wed Thu Fri Sat Sun").split(" ");

  function move(offset: number) {
    setMonth(monthStart(month, offset));
    setSelectedDay("");
  }

  function planSprint(milestone: { title: string; end: string | null }) {
    setKind("sprint");
    setTitle(milestone.title.slice(0, 160));
    setStart("");
    setEnd(milestone.end ?? "");
    setParent("");
    titleField.current?.focus();
  }

  const milestones = entries.filter((entry) => entry.kind === "milestone");
  const sprints = entries.filter((entry) => entry.kind === "sprint");
  const selected = tickets.find((ticket) => ticket.id === ticketId);
  async function assign() {
    setBusy(true);
    setError("");
    try {
      await api(calendarPath + "/tickets", { ticket_id: ticketId, entry_id: entryId || null });
      setTickets((all) => all.map((ticket) => ticket.id === ticketId ? { ...ticket, entry_id: entryId || null } : ticket));
    } catch (reason) {
      setError(errorText(reason));
    } finally {
      setBusy(false);
    }
  }

  function issues(id: string) {
    const items = tickets.filter((ticket) => ticket.entry_id === id);
    return items.length > 0 && <ul>{items.map((ticket) => <li key={ticket.id}>{ticket.public_id} · {ticket.title}</li>)}</ul>;
  }

  return (
    <Dialog title={t("Kalender", "Calendar")} close={close} wide>
      <div class="dialog-body">
        {error && <Message error>{error}</Message>}
        <details open>
          <summary>{t("Meilensteine und Sprints", "Milestones and sprints")}</summary>
          <ul>
            {milestones.map((milestone) => <li key={milestone.id}>
              <strong>{milestone.title}</strong> · {milestone.end}
              {issues(milestone.id)}
              <ul>{sprints.filter((sprint) => sprint.milestone_id === milestone.id).map((sprint) =>
                <li key={sprint.id}>{sprint.title} · {sprint.start} – {sprint.end}{issues(sprint.id)}</li>
              )}</ul>
            </li>)}
            {sprints.filter((sprint) => !sprint.milestone_id).map((sprint) =>
              <li key={sprint.id}>{sprint.title} · {sprint.start} – {sprint.end}{issues(sprint.id)}</li>
            )}
          </ul>
          {entries.length > 0 && <form onSubmit={(event) => { event.preventDefault(); void assign(); }}>
            <fieldset disabled={busy}>
              <legend>{t("Ticket zuordnen", "Assign ticket")}</legend>
              <div class="form-grid">
                <label>{t("Ticket", "Ticket")}
                  <select required value={ticketId} onChange={(event) => {
                    const id = event.currentTarget.value;
                    setTicketId(id);
                    setEntryId(tickets.find((ticket) => ticket.id === id)?.entry_id ?? "");
                  }}>
                    <option value="">{t("Ticket auswählen", "Select ticket")}</option>
                    {tickets.map((ticket) => <option key={ticket.id} value={ticket.id}>{ticket.public_id} · {ticket.title}</option>)}
                  </select>
                </label>
                <label>{t("Zuordnung", "Assignment")}
                  <select value={entryId} disabled={!selected} onChange={(event) => setEntryId(event.currentTarget.value)}>
                    <option value="">{t("Keine Zuordnung", "Unassigned")}</option>
                    {milestones.map((entry) => <option key={entry.id} value={entry.id}>{t("Meilenstein", "Milestone")} · {entry.title}</option>)}
                    {sprints.map((entry) => <option key={entry.id} value={entry.id} disabled={!selected?.in_backlog}>Sprint · {entry.title}</option>)}
                  </select>
                </label>
              </div>
              {selected && !selected.in_backlog && <p>{t("Sprint-Tickets müssen im Backlog liegen.", "Sprint tickets must be in the backlog.")}</p>}
              <button type="submit" disabled={!selected}>{t("Zuordnung speichern", "Save assignment")}</button>
            </fieldset>
          </form>}
        </details>
        <div class="inline spread">
          <button type="button" class="secondary" onClick={() => move(-1)} aria-label={t("Vorheriger Monat", "Previous month")}>←</button>
          <h3>{monthName}</h3>
          <button type="button" class="secondary" onClick={() => move(1)} aria-label={t("Nächster Monat", "Next month")}>→</button>
        </div>
        <div class="calendar-grid">
          {weekdays.map((day) => <span key={day}>{day}</span>)}
          {Array.from({ length: (month.getDay() + 6) % 7 }, (_, index) => <span key={`empty-${index}`} />)}
          {Array.from({ length: days }, (_, index) => {
            const key = dayKey(new Date(month.getFullYear(), month.getMonth(), index + 1));
            const count = events.filter((entry) => includes(entry, key)).length;
            return <button key={key} type="button" class={count ? "" : "secondary"} aria-pressed={selectedDay === key}
              aria-label={`${key}, ${count} ${t("Einträge", "entries")}`}
              onClick={() => setSelectedDay(selectedDay === key ? "" : key)}>{index + 1}</button>;
          })}
        </div>
        <ul>
          {visible.map((entry) => <li key={`${entry.source}-${entry.id}`}>
            <strong>{entry.title}</strong> · {entry.kind === "sprint" ? "Sprint" : t("Meilenstein", "Milestone")}
            {" · "}{entry.start ? `${entry.start} – ` : ""}{entry.end} · {entry.source}
            {entry.closed && ` · ${t("Abgeschlossen", "Closed")}`}
            {entry.source === "Gitea" && <button type="button" class="text-button" onClick={() => planSprint(entry)}>
              {t("Sprint daraus planen", "Plan a sprint from this")}
            </button>}
          </li>)}
        </ul>
        {!visible.length && <p>{t("Keine Termine im gewählten Zeitraum.", "No entries in the selected period.")}</p>}
        {giteaError && <Message error>{giteaError}</Message>}
        {gitea?.configured && (
          <details>
            <summary>{t("Gitea-Meilensteine ohne Termin", "Gitea milestones without a date")}</summary>
            <ul>{gitea.milestones.filter((item) => !item.end).map((item) => <li key={item.id}>{item.title}
              <button type="button" class="text-button" onClick={() => planSprint(item)}>
                {t("Sprint daraus planen", "Plan a sprint from this")}
              </button>
            </li>)}</ul>
            <button type="button" class="secondary" onClick={refreshGitea}>{t("Gitea aktualisieren", "Refresh Gitea")}</button>
          </details>
        )}
        <form onSubmit={(event) => { event.preventDefault(); void save(); }}>
          <fieldset disabled={busy}>
            <legend>{t("Kalendereintrag hinzufügen", "Add calendar entry")}</legend>
            <div class="form-grid">
              <label>{t("Art", "Type")}
                <select value={kind} onChange={(event) => setKind(event.currentTarget.value as Entry["kind"])}>
                  <option value="sprint">Sprint</option>
                  <option value="milestone">{t("Meilenstein", "Milestone")}</option>
                </select>
              </label>
              <label>{t("Titel", "Title")}<input ref={titleField} required maxLength={160} value={title} onInput={(event) => setTitle(event.currentTarget.value)} /></label>
              {kind === "sprint" && <label>{t("Beginn", "Start")}<input type="date" required value={start} onInput={(event) => setStart(event.currentTarget.value)} /></label>}
              {kind === "sprint" && <label>{t("Übergeordneter Meilenstein", "Parent milestone")}
                <select value={parent} onChange={(event) => setParent(event.currentTarget.value)}>
                  <option value="">{t("Ohne Meilenstein", "No milestone")}</option>
                  {milestones.map((entry) => <option key={entry.id} value={entry.id}>{entry.title}</option>)}
                </select>
              </label>}
              <label>{kind === "sprint" ? t("Ende", "End") : t("Termin", "Due date")}
                <input type="date" required min={kind === "sprint" ? start : undefined} value={end}
                  onInput={(event) => setEnd(event.currentTarget.value)} />
              </label>
            </div>
            <button type="submit">{t("Hinzufügen", "Add")}</button>
          </fieldset>
        </form>
      </div>
    </Dialog>
  );
}
