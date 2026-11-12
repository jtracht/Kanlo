import { useEffect, useRef, useState } from "preact/hooks";
import { IntegrationSetup } from "./IntegrationSetup";
import { UpdateStatus } from "./UpdateStatus";
import type {
  BoardOptions,
  Bootstrap,
  Command,
  QueueEntry,
  Settings,
  Snapshot,
  TicketType,
  Label,
  Recurrence,
} from "./types";
import { api, connection, queueCommand } from "./client";
import { optimistic } from "./workspace";
import {
  Dialog,
  Message,
  useT,
  errorText,
  confirmDiscard,
  useUnsavedGuard,
} from "./ui";
import { uuid } from "./identifiers";
import * as storage from "./storage";

interface Props {
  bootstrap: Bootstrap;
  snapshot: Snapshot;
  close: () => void;
  refresh: () => void;
  preview: (entry: QueueEntry) => void;
  data: () => void;
  history: () => void;
}
export function SettingsDialog({
  bootstrap,
  snapshot,
  close,
  refresh,
  preview,
  data,
  history,
}: Props) {
  const t = useT();
  const baseline = useRef(structuredClone(snapshot));
  const [settings, setSettings] = useState<Settings>(
    structuredClone(bootstrap.settings),
  );
  const board = snapshot.workspace.boards[0];
  const [options, setOptions] = useState<BoardOptions>(
    structuredClone(snapshot.workspace.board_options[board.id]),
  );
  const [boardName, setBoardName] = useState(board.name);
  const [columnName, setColumnName] = useState("");
  const [error, setError] = useState("");
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [passcode, setPasscode] = useState("");
  const [keyName, setKeyName] = useState("");
  const [keyRows, setKeyRows] = useState<
    { id: string; name: string; revoked: boolean }[]
  >([]);
  const [secret, setSecret] = useState("");
  const initial = useRef(JSON.stringify({ settings, options, boardName }));
  const [type, setType] = useState<TicketType>({
    id: "",
    name: "",
    color: "#45765d",
    icon: "●",
    rank: 0,
    allow_children: true,
  });
  const [label, setLabel] = useState<Label>({
    id: "",
    name: "",
    color: "#a56523",
  });
  const dirty =
    JSON.stringify({ settings, options, boardName }) !== initial.current ||
    !!passcode ||
    !!keyName ||
    !!columnName ||
    !!type.name ||
    !!label.name;
  useUnsavedGuard(dirty);
  function setSetting<K extends keyof Settings>(key: K, value: Settings[K]) {
    setSettings((previous) => ({ ...previous, [key]: value }));
  }
  function setOption<K extends keyof BoardOptions>(
    key: K,
    value: BoardOptions[K],
  ) {
    setOptions((previous) => ({ ...previous, [key]: value }));
  }
  async function run(callback: () => Promise<void>) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await callback();
      refresh();
    } catch (error) {
      setError(errorText(error));
    } finally {
      setBusy(false);
    }
  }
  async function command(action: Command, confirm = false) {
    const entry = await queueCommand(
      action,
      board.id,
      baseline.current,
      confirm,
    );
    if (entry && confirm) preview(entry);
    else if (entry)
      baseline.current = optimistic(baseline.current, entry, [
        baseline.current,
      ]);
    setMessage(
      t("Dauerhaft lokal vorgemerkt.", "Saved durably on this device."),
    );
  }
  function saved(part: "settings" | "options" | "boardName", value: unknown) {
    const original = JSON.parse(initial.current);
    original[part] = value;
    initial.current = JSON.stringify(original);
  }
  useEffect(() => {
    if (connection.online)
      api<typeof keyRows>("/security/keys")
        .then(setKeyRows)
        .catch(() => {});
  }, []);
  return (
    <Dialog
      title={t("Einstellungen", "Settings")}
      close={() => {
        if (confirmDiscard(dirty, t)) close();
      }}
      wide
    >
      <div class="dialog-body settings">
        {error && <Message error>{error}</Message>}
        {message && <Message>{message}</Message>}
        <section>
          <h3>
            {t(
              "Darstellung, Sprache und Sicherung",
              "Appearance, language and backups",
            )}
          </h3>
          <div class="form-grid">
            <label>
              {t("Sprache", "Language")}
              <select
                value={settings.language}
                onChange={(event) =>
                  setSetting(
                    "language",
                    event.currentTarget.value as Settings["language"],
                  )
                }
              >
                <option value="de">Deutsch</option>
                <option value="en">English</option>
              </select>
            </label>
            <label>
              {t("Design", "Theme")}
              <select
                value={settings.theme}
                onChange={(event) =>
                  setSetting(
                    "theme",
                    event.currentTarget.value as Settings["theme"],
                  )
                }
              >
                <option value="light">{t("Hell", "Light")}</option>
                <option value="dark">{t("Dunkel", "Dark")}</option>
                <option value="system">
                  {t("Wie Gerät", "Device preference")}
                </option>
              </select>
            </label>
            <label>
              {t("Animationen", "Animations")}
              <select
                value={settings.animation}
                onChange={(event) =>
                  setSetting(
                    "animation",
                    event.currentTarget.value as Settings["animation"],
                  )
                }
              >
                <option value="off">{t("Aus", "Off")}</option>
                <option value="subtle">{t("Dezent", "Subtle")}</option>
                <option value="fluid">{t("Flüssig", "Fluid")}</option>
                <option value="dynamic">{t("Dynamisch", "Dynamic")}</option>
              </select>
            </label>
            <label>
              {t("Akzentfarbe", "Accent color")}
              <input
                type="color"
                value={settings.accent_color}
                onInput={(event) =>
                  setSetting("accent_color", event.currentTarget.value)
                }
              />
            </label>
            <label>
              {t("Zeitzone", "Timezone")}
              <input
                value={settings.timezone}
                onInput={(event) =>
                  setSetting("timezone", event.currentTarget.value)
                }
                placeholder="Europe/Berlin"
              />
            </label>
            <label>
              {t(
                "Automatische Sicherung: alle Stunden (leer = aus)",
                "Automatic backup: every N hours (empty = off)",
              )}
              <input
                type="number"
                min="1"
                max="8760"
                value={settings.backup_every_hours ?? ""}
                onInput={(event) =>
                  setSetting(
                    "backup_every_hours",
                    event.currentTarget.value
                      ? Number(event.currentTarget.value)
                      : null,
                  )
                }
              />
            </label>
            <label>
              {t(
                "Automatische Sicherungen behalten",
                "Automatic backups to retain",
              )}
              <input
                type="number"
                min="1"
                max="365"
                value={settings.backup_keep}
                onInput={(event) =>
                  setSetting("backup_keep", Number(event.currentTarget.value))
                }
              />
            </label>
          </div>
          <button
            disabled={busy}
            onClick={() =>
              run(async () => {
                await command({ kind: "set_settings", settings });
                saved("settings", settings);
              })
            }
          >
            {t("Allgemeine Einstellungen speichern", "Save general settings")}
          </button>
        </section>
        <section>
          <h3>
            {t("Aktuelles Board", "Current board")} · {board.prefix}
          </h3>
          <div class="input-row">
            <label>
              {t("Boardname", "Board name")}
              <input
                value={boardName}
                maxLength={120}
                onInput={(event) => setBoardName(event.currentTarget.value)}
              />
            </label>
            <button
              class="secondary"
              disabled={busy || !boardName.trim()}
              onClick={() =>
                run(async () => {
                  await command({
                    kind: "update_board",
                    id: board.id,
                    name: boardName,
                    base_revision:
                      baseline.current.workspace.boards[0].revision,
                  });
                  saved("boardName", boardName);
                })
              }
            >
              {t("Namen speichern", "Save name")}
            </button>
          </div>
          <p class="muted">
            {t(
              "Kürzel und ausgegebene Ticketkennungen werden nicht wiederverwendet.",
              "Prefixes and issued ticket identifiers are never reused.",
            )}
          </p>
          <div class="form-grid">
            <label>
              {t("Boardfarbe", "Board color")}
              <input
                type="color"
                value={options.color}
                onInput={(event) =>
                  setOption("color", event.currentTarget.value)
                }
              />
            </label>
            <label>
              {t("Undo-Schritte (0 = aus)", "Undo steps (0 = off)")}
              <input
                type="number"
                min="0"
                max="1000"
                value={options.undo_depth}
                onInput={(event) =>
                  setOption("undo_depth", Number(event.currentTarget.value))
                }
              />
            </label>
            <label>
              {t("Gewichtung Priorität", "Importance weight")}
              <input
                type="number"
                min="0"
                max="65535"
                value={options.importance_weight}
                onInput={(event) =>
                  setOption(
                    "importance_weight",
                    Number(event.currentTarget.value),
                  )
                }
              />
            </label>
            <label>
              {t("Gewichtung Typ", "Type weight")}
              <input
                type="number"
                min="0"
                max="65535"
                value={options.type_weight}
                onInput={(event) =>
                  setOption("type_weight", Number(event.currentTarget.value))
                }
              />
            </label>
            <label>
              {t("Gewichtung Deadline", "Deadline weight")}
              <input
                type="number"
                min="0"
                max="65535"
                value={options.deadline_weight}
                onInput={(event) =>
                  setOption(
                    "deadline_weight",
                    Number(event.currentTarget.value),
                  )
                }
              />
            </label>
            <label>
              {t(
                "Deadlinewarnung: Tage vorher",
                "Deadline warning: days ahead",
              )}
              <input
                type="number"
                min="0"
                max="3650"
                value={options.deadline_warning_days}
                onInput={(event) =>
                  setOption(
                    "deadline_warning_days",
                    Number(event.currentTarget.value),
                  )
                }
              />
            </label>
            <label>
              {t(
                "Automatisch archivieren nach Tagen (leer = aus)",
                "Auto-archive after days (empty = off)",
              )}
              <input
                type="number"
                min="0"
                max="65535"
                value={options.auto_archive_days ?? ""}
                onInput={(event) =>
                  setOption(
                    "auto_archive_days",
                    event.currentTarget.value
                      ? Number(event.currentTarget.value)
                      : null,
                  )
                }
              />
            </label>
            <label>
              {t(
                "Papierkorb automatisch leeren nach Tagen (leer = aus)",
                "Purge trash after days (empty = off)",
              )}
              <input
                type="number"
                min="0"
                max="65535"
                value={options.trash_retention_days ?? ""}
                onInput={(event) =>
                  setOption(
                    "trash_retention_days",
                    event.currentTarget.value
                      ? Number(event.currentTarget.value)
                      : null,
                  )
                }
              />
            </label>
          </div>
          <fieldset>
            <legend>
              {t("Aktive globale Tickettypen", "Enabled global ticket types")}
            </legend>
            {bootstrap.types.map((type) => (
              <label key={type.id} class="inline check">
                <input
                  type="checkbox"
                  checked={options.enabled_types.includes(type.id)}
                  onChange={(event) =>
                    setOption(
                      "enabled_types",
                      event.currentTarget.checked
                        ? [...options.enabled_types, type.id]
                        : options.enabled_types.filter((id) => id !== type.id),
                    )
                  }
                />
                {type.name}
              </label>
            ))}
          </fieldset>
          {options.undo_depth === 0 && (
            <Message>
              {t(
                "Undo wird deaktiviert und vorhandener Undo-Verlauf entfernt. Der Papierkorb bleibt erhalten.",
                "Undo is disabled and existing undo history removed. Trash remains available.",
              )}
            </Message>
          )}
          <div class="actions wrap">
            <button
              disabled={busy}
              onClick={() =>
                run(async () => {
                  if (
                    options.undo_depth === 0 &&
                    !window.confirm(
                      t(
                        "Undo-Verlauf entfernen und Undo deaktivieren?",
                        "Remove undo history and disable undo?",
                      ),
                    )
                  )
                    return;
                  await command({
                    kind: "set_board_options",
                    board_id: board.id,
                    options,
                  });
                  saved("options", options);
                })
              }
            >
              {t("Boardregeln speichern", "Save board rules")}
            </button>
            <button class="secondary" onClick={history}>
              {t("Änderungsverlauf", "Change history")}
            </button>
            <button
              class="danger secondary"
              onClick={() =>
                run(() =>
                  command(
                    {
                      kind: "delete_board",
                      id: board.id,
                      base_revision:
                        baseline.current.workspace.boards[0].revision,
                    },
                    true,
                  ),
                )
              }
            >
              {t("Leeres Board löschen", "Delete empty board")}
            </button>
          </div>
        </section>
        <section>
          <h3>{t("Spalten", "Columns")}</h3>
          {snapshot.workspace.columns
            .slice()
            .sort((a, b) => a.position - b.position)
            .map((column) => (
              <ColumnEditor
                key={column.id}
                column={column}
                color={options.column_colors[column.id] ?? "#66806b"}
                busy={busy}
                save={(action) => run(() => command(action))}
                remove={() =>
                  run(() =>
                    command({ kind: "delete_column", id: column.id }, true),
                  )
                }
                move={(offset) =>
                  run(async () => {
                    const ids = snapshot.workspace.columns
                      .slice()
                      .sort((a, b) => a.position - b.position)
                      .map((column) => column.id);
                    const index = ids.indexOf(column.id);
                    const next = index + offset;
                    if (next < 0 || next >= ids.length) return;
                    [ids[index], ids[next]] = [ids[next], ids[index]];
                    await command({
                      kind: "reorder_columns",
                      board_id: board.id,
                      ids,
                    });
                  })
                }
              />
            ))}
          <div class="input-row">
            <input
              aria-label={t("Neue Spalte", "New column")}
              value={columnName}
              maxLength={120}
              onInput={(event) => setColumnName(event.currentTarget.value)}
            />
            <button
              class="secondary"
              disabled={!columnName.trim() || busy}
              onClick={() =>
                run(async () => {
                  await command({
                    kind: "create_column",
                    board_id: board.id,
                    name: columnName,
                    is_done: false,
                    wip_limit: null,
                    wip_strict: false,
                  });
                  setColumnName("");
                })
              }
            >
              {t("Spalte anlegen", "Create column")}
            </button>
          </div>
        </section>
        <section>
          <h3>{t("Globale Typbibliothek", "Global type library")}</h3>
          <p class="muted">
            {t(
              "Typen haben keine Pflichtfelder. Eigene Textfelder gehören immer zum einzelnen Ticket.",
              "Types do not define required fields. Custom fields always belong to individual tickets.",
            )}
          </p>
          <div class="catalog-list">
            {bootstrap.types.map((value) => (
              <div class="inline spread" key={value.id}>
                <span>
                  <span
                    class="color-dot"
                    style={{ backgroundColor: value.color }}
                  />
                  {value.icon} {value.name}
                </span>
                <span>
                  <button
                    class="text-button"
                    onClick={() => setType(structuredClone(value))}
                  >
                    {t("Bearbeiten", "Edit")}
                  </button>
                  <button
                    class="text-button danger-text"
                    onClick={() =>
                      run(() =>
                        command({ kind: "delete_type", id: value.id }, true),
                      )
                    }
                  >
                    {t("Löschen", "Delete")}
                  </button>
                </span>
              </div>
            ))}
          </div>
          <div class="form-grid">
            <label>
              {t("Typname", "Type name")}
              <input
                value={type.name}
                maxLength={80}
                onInput={(event) =>
                  setType({ ...type, name: event.currentTarget.value })
                }
              />
            </label>
            <label>
              {t("Typfarbe", "Type color")}
              <input
                type="color"
                value={type.color}
                onInput={(event) =>
                  setType({ ...type, color: event.currentTarget.value })
                }
              />
            </label>
            <label>
              {t("Symbol", "Icon")}
              <input
                value={type.icon}
                maxLength={64}
                onInput={(event) =>
                  setType({ ...type, icon: event.currentTarget.value })
                }
              />
            </label>
            <label>
              {t("Priorisierungsrang (0–255)", "Type rank (0–255)")}
              <input
                type="number"
                min="0"
                max="255"
                value={type.rank}
                onInput={(event) =>
                  setType({ ...type, rank: Number(event.currentTarget.value) })
                }
              />
            </label>
          </div>
          <label class="inline check">
            <input
              type="checkbox"
              checked={type.allow_children}
              onChange={(event) =>
                setType({
                  ...type,
                  allow_children: event.currentTarget.checked,
                })
              }
            />
            {t("Untertickets erlauben", "Allow subtickets")}
          </label>
          <button
            class="secondary"
            disabled={!type.name.trim() || busy}
            onClick={() =>
              run(async () => {
                await command({
                  kind: "upsert_type",
                  value: { ...type, id: type.id || uuid() },
                });
                setType({
                  id: "",
                  name: "",
                  color: "#45765d",
                  icon: "●",
                  rank: 0,
                  allow_children: true,
                });
              })
            }
          >
            {t("Typ speichern", "Save type")}
          </button>
        </section>
        <section>
          <h3>{t("Globale Labels", "Global labels")}</h3>
          <p class="muted">
            {t(
              "Jedes Label ist auf jedem Board verfügbar.",
              "Every label is available on every board.",
            )}
          </p>
          <div class="catalog-list">
            {bootstrap.labels.map((value) => (
              <div class="inline spread" key={value.id}>
                <span>
                  <span
                    class="color-dot"
                    style={{ backgroundColor: value.color }}
                  />
                  {value.name}
                </span>
                <span>
                  <button
                    class="text-button"
                    onClick={() => setLabel(structuredClone(value))}
                  >
                    {t("Bearbeiten", "Edit")}
                  </button>
                  <button
                    class="text-button danger-text"
                    onClick={() =>
                      run(() =>
                        command({ kind: "delete_label", id: value.id }, true),
                      )
                    }
                  >
                    {t("Löschen", "Delete")}
                  </button>
                </span>
              </div>
            ))}
          </div>
          <div class="form-grid">
            <label>
              {t("Labelname", "Label name")}
              <input
                value={label.name}
                maxLength={80}
                onInput={(event) =>
                  setLabel({ ...label, name: event.currentTarget.value })
                }
              />
            </label>
            <label>
              {t("Labelfarbe", "Label color")}
              <input
                type="color"
                value={label.color}
                onInput={(event) =>
                  setLabel({ ...label, color: event.currentTarget.value })
                }
              />
            </label>
          </div>
          <button
            class="secondary"
            disabled={!label.name.trim() || busy}
            onClick={() =>
              run(async () => {
                await command({
                  kind: "upsert_label",
                  value: { ...label, id: label.id || uuid() },
                });
                setLabel({ id: "", name: "", color: "#a56523" });
              })
            }
          >
            {t("Label speichern", "Save label")}
          </button>
        </section>
        <RecurrenceEditor
          snapshot={snapshot}
          save={(action) => run(() => command(action))}
        />
        <section>
          <h3>{t("Daten und Sicherung", "Data and backups")}</h3>
          <p>
            {t(
              "JSON bewahrt die vollständigen Boardbeziehungen. CSV enthält tabellarische Kerndaten. Wiederherstellung erfolgt mit Vorschau und zusätzlicher Sicherung.",
              "JSON preserves complete board relationships. CSV contains tabular core data. Restore uses a preview and an additional safety backup.",
            )}
          </p>
          <button class="secondary" onClick={data}>
            {t(
              "Import, Export und Wiederherstellung",
              "Import, export and restore",
            )}
          </button>
        </section>
        <section>
          <h3>{t("Zugriff und Integrationen", "Access and integrations")}</h3>
          <Message>
            {t(
              "Im freigegebenen LAN-Zugang können Netzwerkteilnehmer Kanlo ohne Anmeldung bearbeiten. Externer Zugriff gehört ausschließlich auf den geschützten HTTPS-Zugangsweg.",
              "Participants on the enabled LAN access can edit Kanlo without signing in. External access must use the separate protected HTTPS ingress.",
            )}
          </Message>
          <label>
            {t(
              "Neuer gemeinsamer Zugangscode (mindestens 12 Zeichen)",
              "New shared passcode (at least 12 characters)",
            )}
            <input
              type="password"
              minLength={12}
              maxLength={256}
              autoComplete="new-password"
              value={passcode}
              onInput={(event) => setPasscode(event.currentTarget.value)}
            />
          </label>
          <button
            class="secondary"
            disabled={!connection.online || passcode.length < 12 || busy}
            onClick={() =>
              run(async () => {
                await api("/security/passcode", { passcode });
                setPasscode("");
                setMessage(
                  t(
                    "Zugangscode gespeichert; externe Sitzungen wurden abgemeldet.",
                    "Passcode saved; external sessions were signed out.",
                  ),
                );
              })
            }
          >
            {t("Zugangscode speichern", "Save passcode")}
          </button>
          <h4>{t("Getrennte API-Schlüssel", "Independent API keys")}</h4>
          {keyRows.map((key) => (
            <div class="inline spread" key={key.id}>
              <span>
                {key.name} {key.revoked && `(${t("widerrufen", "revoked")})`}
              </span>
              {!key.revoked && (
                <button
                  class="text-button danger-text"
                  onClick={() =>
                    run(async () => {
                      if (
                        !window.confirm(
                          `${key.name}: ${t("Schlüssel widerrufen?", "Revoke key?")}`,
                        )
                      )
                        return;
                      await api(`/security/keys/${key.id}/revoke`, {});
                      setKeyRows(await api("/security/keys"));
                    })
                  }
                >
                  {t("Widerrufen", "Revoke")}
                </button>
              )}
            </div>
          ))}
          <div class="input-row">
            <input
              aria-label={t("Name für API-Schlüssel", "API key name")}
              value={keyName}
              onInput={(event) => setKeyName(event.currentTarget.value)}
            />
            <button
              class="secondary"
              disabled={!keyName.trim() || !connection.online || busy}
              onClick={() =>
                run(async () => {
                  const value = await api<{ token: string }>("/security/keys", {
                    name: keyName,
                  });
                  setSecret(value.token);
                  setKeyName("");
                  setKeyRows(await api("/security/keys"));
                })
              }
            >
              {t("Schlüssel erstellen", "Create key")}
            </button>
          </div>
          <IntegrationSetup />
          <UpdateStatus />
          {secret && (
            <div class="secret">
              <strong>
                {t(
                  "Nur jetzt sichtbar. Sicher kopieren.",
                  "Shown only now. Copy it securely.",
                )}
              </strong>
              <output>{secret}</output>
              <button class="text-button" onClick={() => setSecret("")}>
                {t("Ausblenden", "Hide")}
              </button>
            </div>
          )}
          <button
            class="text-button danger-text"
            disabled={!connection.online}
            onClick={() =>
              run(async () => {
                if (
                  window.confirm(
                    t(
                      "Alle externen Sitzungen abmelden?",
                      "Sign out all external sessions?",
                    ),
                  )
                )
                  await api("/security/sessions/revoke", {});
              })
            }
          >
            {t(
              "Alle externen Sitzungen abmelden",
              "Sign out all external sessions",
            )}
          </button>
        </section>
        <section>
          <h3>{t("Daten auf diesem Browser", "Data on this browser")}</h3>
          <p>
            {t(
              "Nur bereits geladene Boards und deine Änderungswarteschlange werden lokal gespeichert. API-Schlüssel und Zugangscodes gehören nicht in diesen Cache.",
              "Only loaded boards and your pending edits are stored locally. API keys and passcodes are excluded from this cache.",
            )}
          </p>
          <button
            class="danger secondary"
            onClick={() =>
              run(async () => {
                const pending = await storage.queue();
                if (
                  !window.confirm(
                    `${t("Lokale Boardkopien entfernen?", "Remove local board copies?")} ${pending.length ? `${pending.length} ${t("noch nicht synchronisierte Änderungen werden dauerhaft verworfen.", "unsynchronized edits will be permanently discarded.")}` : t("Serverdaten bleiben erhalten.", "Server data will be retained.")}`,
                  )
                )
                  return;
                await storage.clearCache(pending.length);
                location.reload();
              })
            }
          >
            {t("Lokale Daten entfernen", "Remove local data")}
          </button>
        </section>
      </div>
      <footer class="dialog-footer">
        <button
          class="secondary"
          onClick={() => {
            if (confirmDiscard(dirty, t)) close();
          }}
        >
          {t("Schließen", "Close")}
        </button>
      </footer>
    </Dialog>
  );
}

function ColumnEditor({
  column,
  color,
  busy,
  save,
  remove,
  move,
}: {
  column: Snapshot["workspace"]["columns"][number];
  color: string;
  busy: boolean;
  save: (action: Command) => Promise<void>;
  remove: () => void;
  move: (offset: number) => void;
}) {
  const t = useT();
  const [draft, setDraft] = useState({ ...column, color });
  return (
    <div class="column-editor">
      <label>
        {t("Spaltenname", "Column name")}
        <input
          value={draft.name}
          onInput={(event) =>
            setDraft({ ...draft, name: event.currentTarget.value })
          }
        />
      </label>
      <label>
        {t("Farbe", "Color")}
        <input
          type="color"
          value={draft.color}
          onInput={(event) =>
            setDraft({ ...draft, color: event.currentTarget.value })
          }
        />
      </label>
      <label>
        {t("WIP-Limit", "WIP limit")}
        <input
          type="number"
          min="0"
          value={draft.wip_limit ?? ""}
          onInput={(event) =>
            setDraft({
              ...draft,
              wip_limit: event.currentTarget.value
                ? Number(event.currentTarget.value)
                : null,
            })
          }
        />
      </label>
      <label class="inline check">
        <input
          type="checkbox"
          checked={draft.wip_strict}
          onChange={(event) =>
            setDraft({ ...draft, wip_strict: event.currentTarget.checked })
          }
        />
        {t("Strikte Sperre", "Strict limit")}
      </label>
      <label class="inline check">
        <input
          type="checkbox"
          checked={draft.is_done}
          onChange={(event) =>
            setDraft({ ...draft, is_done: event.currentTarget.checked })
          }
        />
        Done
      </label>
      <div class="actions wrap">
        <button
          class="secondary"
          disabled={busy}
          onClick={() =>
            save({
              kind: "update_column",
              id: column.id,
              name: draft.name,
              is_done: draft.is_done,
              wip_limit: draft.wip_limit,
              wip_strict: draft.wip_strict,
              color: draft.color,
            })
          }
        >
          {t("Spalte speichern", "Save column")}
        </button>
        <button
          class="icon-button"
          aria-label={t("Spalte nach links", "Move column left")}
          onClick={() => move(-1)}
        >
          ←
        </button>
        <button
          class="icon-button"
          aria-label={t("Spalte nach rechts", "Move column right")}
          onClick={() => move(1)}
        >
          →
        </button>
        <button class="text-button danger-text" onClick={remove}>
          {t("Löschen", "Delete")}
        </button>
      </div>
    </div>
  );
}

function RecurrenceEditor({
  snapshot,
  save,
}: {
  snapshot: Snapshot;
  save: (action: Command) => Promise<void>;
}) {
  const t = useT();
  const initial = (): Recurrence => ({
    id: "",
    template_id:
      snapshot.workspace.tickets.find(
        (ticket) => !ticket.details.parent_id && !ticket.details.trashed,
      )?.id ?? "",
    frequency: "weekly",
    interval: 1,
    weekdays: [],
    month_day: null,
    nth_weekday: null,
    time: "09:00",
    timezone: snapshot.settings.timezone,
    next_at: new Date(Date.now() + 86400000).toISOString(),
    anchor_at: null,
    after_completion: false,
    last_created: null,
    enabled: true,
  });
  const [rule, setRule] = useState<Recurrence>(initial);
  return (
    <section>
      <h3>{t("Wiederkehrende Tickets", "Recurring tickets")}</h3>
      {snapshot.workspace.recurrences.map((value) => (
        <div class="inline spread" key={value.id}>
          <span>
            {
              snapshot.workspace.tickets.find(
                (ticket) => ticket.id === value.template_id,
              )?.public_id
            }{" "}
            · {value.frequency} · {value.interval}
          </span>
          <span>
            <button
              class="text-button"
              onClick={() => setRule(structuredClone(value))}
            >
              {t("Bearbeiten", "Edit")}
            </button>
            <button
              class="text-button danger-text"
              onClick={() => save({ kind: "delete_recurrence", id: value.id })}
            >
              {t("Entfernen", "Remove")}
            </button>
          </span>
        </div>
      ))}
      <div class="form-grid">
        <label>
          {t("Vorlagenticket", "Template ticket")}
          <select
            value={rule.template_id}
            onChange={(event) =>
              setRule({ ...rule, template_id: event.currentTarget.value })
            }
          >
            <option value="">{t("Bitte wählen", "Please select")}</option>
            {snapshot.workspace.tickets
              .filter(
                (ticket) =>
                  !ticket.details.parent_id && !ticket.details.trashed,
              )
              .map((ticket) => (
                <option key={ticket.id} value={ticket.id}>
                  {ticket.public_id} · {ticket.title}
                </option>
              ))}
          </select>
        </label>
        <label>
          {t("Rhythmus", "Frequency")}
          <select
            value={rule.frequency}
            onChange={(event) =>
              setRule({ ...rule, frequency: event.currentTarget.value })
            }
          >
            <option value="daily">{t("Täglich", "Daily")}</option>
            <option value="weekly">{t("Wöchentlich", "Weekly")}</option>
            <option value="monthly">{t("Monatlich", "Monthly")}</option>
            <option value="yearly">{t("Jährlich", "Yearly")}</option>
          </select>
        </label>
        <label>
          {t("Alle N Einheiten", "Every N intervals")}
          <input
            type="number"
            min="1"
            max="1000"
            value={rule.interval}
            onInput={(event) =>
              setRule({ ...rule, interval: Number(event.currentTarget.value) })
            }
          />
        </label>
        <label>
          {t("Ortszeit", "Local time")}
          <input
            type="time"
            value={rule.time}
            onInput={(event) =>
              setRule({ ...rule, time: event.currentTarget.value })
            }
          />
        </label>
        <label>
          {t(
            "Nächster Termin (ISO mit Zeitzone)",
            "Next occurrence (ISO with timezone)",
          )}
          <input
            value={rule.next_at}
            onInput={(event) =>
              setRule({
                ...rule,
                next_at: event.currentTarget.value,
                anchor_at: null,
              })
            }
          />
        </label>
        <label>
          {t("Zeitzone der Regel", "Schedule timezone")}
          <input
            value={rule.timezone}
            onInput={(event) =>
              setRule({ ...rule, timezone: event.currentTarget.value })
            }
          />
        </label>
        {rule.frequency === "monthly" && (
          <>
            <label>
              {t("Tag im Monat (optional)", "Day of month (optional)")}
              <input
                type="number"
                min="1"
                max="31"
                value={rule.month_day ?? ""}
                onInput={(event) =>
                  setRule({
                    ...rule,
                    month_day: event.currentTarget.value
                      ? Number(event.currentTarget.value)
                      : null,
                    nth_weekday: null,
                  })
                }
              />
            </label>
            <label>
              {t(
                "N-ter Wochentag (1–5, -1 = letzter)",
                "Nth weekday (1–5, -1 = last)",
              )}
              <input
                type="number"
                min="-5"
                max="5"
                value={rule.nth_weekday?.[0] ?? ""}
                onInput={(event) =>
                  setRule({
                    ...rule,
                    month_day: null,
                    nth_weekday: event.currentTarget.value
                      ? [
                          Number(event.currentTarget.value),
                          rule.nth_weekday?.[1] ?? 0,
                        ]
                      : null,
                  })
                }
              />
            </label>
            {rule.nth_weekday && (
              <label>
                {t(
                  "Wochentag (0 = Montag, 6 = Sonntag)",
                  "Weekday (0 = Monday, 6 = Sunday)",
                )}
                <input
                  type="number"
                  min="0"
                  max="6"
                  value={rule.nth_weekday[1]}
                  onInput={(event) =>
                    setRule({
                      ...rule,
                      nth_weekday: [
                        rule.nth_weekday![0],
                        Number(event.currentTarget.value),
                      ],
                    })
                  }
                />
              </label>
            )}
          </>
        )}
      </div>
      {rule.frequency === "weekly" && (
        <fieldset>
          <legend>
            {t(
              "Wochentage (leer = Tag des Starttermins)",
              "Weekdays (empty = start-date weekday)",
            )}
          </legend>
          {["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"].map((name, day) => (
            <label class="inline check" key={day}>
              <input
                type="checkbox"
                checked={rule.weekdays.includes(day)}
                onChange={(event) =>
                  setRule({
                    ...rule,
                    weekdays: event.currentTarget.checked
                      ? [...rule.weekdays, day]
                      : rule.weekdays.filter((value) => value !== day),
                  })
                }
              />
              {name}
            </label>
          ))}
        </fieldset>
      )}
      <label class="inline check">
        <input
          type="checkbox"
          checked={rule.after_completion}
          onChange={(event) =>
            setRule({ ...rule, after_completion: event.currentTarget.checked })
          }
        />
        {t(
          "Erst nach Abschluss des Vorgängers",
          "Wait until the predecessor is complete",
        )}
      </label>
      <label class="inline check">
        <input
          type="checkbox"
          checked={rule.enabled}
          onChange={(event) =>
            setRule({ ...rule, enabled: event.currentTarget.checked })
          }
        />
        {t("Regel aktiv", "Schedule enabled")}
      </label>
      <button
        class="secondary"
        disabled={!rule.template_id}
        onClick={async () => {
          await save({
            kind: "upsert_recurrence",
            value: { ...rule, id: rule.id || uuid() },
          });
          setRule(initial());
        }}
      >
        {t("Wiederholung speichern", "Save recurrence")}
      </button>
    </section>
  );
}
