export type Command = { kind: string; [key: string]: unknown };
export interface Board {
  id: string;
  name: string;
  prefix: string;
  revision: number;
  next_ticket_number: number;
}
export interface Column {
  id: string;
  board_id: string;
  name: string;
  position: number;
  is_done: boolean;
  wip_limit: number | null;
  wip_strict: boolean;
}
export interface Details {
  parent_id: string | null;
  type_id: string | null;
  labels: string[];
  custom_fields: Record<string, string>;
  deadline: string | null;
  archived: boolean;
  trashed: boolean;
  deleted_at: string | null;
  completed_at: string | null;
  rank: number;
  last_active_column: string | null;
  created_at: string;
  updated_at: string;
}
export interface Ticket {
  id: string;
  public_id: string;
  board_id: string;
  column_id: string | null;
  title: string;
  description: string;
  importance: number;
  done: boolean;
  revision: number;
  details: Details;
}
export interface TicketType {
  id: string;
  name: string;
  color: string;
  icon: string;
  rank: number;
  allow_children: boolean;
}
export interface Label {
  id: string;
  name: string;
  color: string;
}
export interface Comment {
  id: string;
  ticket_id: string;
  markdown: string;
  created_at: string;
}
export interface Dependency {
  ticket_id: string;
  prerequisite_id: string;
}
export interface Link {
  ticket_id: string;
  target_id: string;
}
export interface BoardOptions {
  enabled_types: string[];
  color: string;
  column_colors: Record<string, string>;
  undo_depth: number;
  importance_weight: number;
  type_weight: number;
  deadline_weight: number;
  deadline_warning_days: number;
  auto_archive_days: number | null;
  trash_retention_days: number | null;
}
export interface View {
  board_id: string;
  query: string;
  labels: string[];
  type_id: string | null;
  status: string;
  sort: string;
  descending: boolean;
  scroll_x: number;
  scroll_y: number;
  importance_min: number | null;
  importance_max: number | null;
  deadline_from: string | null;
  deadline_to: string | null;
  custom_fields: Record<string, string>;
}
export interface Settings {
  language: "de" | "en";
  theme: "light" | "dark" | "system";
  timezone: string;
  animation: "off" | "subtle" | "fluid" | "dynamic";
  accent_color: string;
  backup_every_hours: number | null;
  backup_keep: number;
}
export interface Recurrence {
  id: string;
  template_id: string;
  frequency: string;
  interval: number;
  weekdays: number[];
  month_day: number | null;
  nth_weekday: [number, number] | null;
  time: string;
  timezone: string;
  next_at: string;
  anchor_at: string | null;
  after_completion: boolean;
  last_created: string | null;
  enabled: boolean;
}
export interface Workspace {
  boards: Board[];
  columns: Column[];
  tickets: Ticket[];
  types: TicketType[];
  labels: Label[];
  comments: Comment[];
  dependencies: Dependency[];
  links: Link[];
  board_options: Record<string, BoardOptions>;
  views: Record<string, View>;
  settings: Settings;
  recurrences: Recurrence[];
}
export interface Bootstrap {
  boards: Board[];
  types: TicketType[];
  labels: Label[];
  settings: Settings;
  board_options: Record<string, BoardOptions>;
  views: Record<string, View>;
  revision: number;
  instance_id: string;
  epoch: string;
  passcode_configured: boolean;
}
export interface Snapshot {
  workspace: Workspace;
  external_references: { id: string; public_id: string }[];
  revision: number;
  settings: Settings;
  view_states: Record<string, View>;
  epoch: string;
}
export interface EntityChange {
  kind: string;
  id: string;
  board_id: string | null;
  body: unknown | null;
}
export interface Receipt {
  op_id: string;
  revision: number;
  result: {
    entity_id: string | null;
    affected: string[];
    boards: string[];
    warnings: string[];
  };
  temp_ids: Record<string, string>;
  changes: EntityChange[];
  epoch?: string;
}
export interface FieldConflict {
  field: string;
  base: unknown;
  server: unknown;
  local: unknown;
}
export interface QueueEntry {
  seq?: number;
  op_id: string;
  endpoint: string;
  body: Record<string, unknown>;
  board_id: string;
  action: Command;
  created_at: string;
  state: "pending" | "conflict" | "epoch_review" | "confirmation";
  conflicts?: FieldConflict[];
  message?: string;
}
export const defaultView = (board_id: string): View => ({
  board_id,
  query: "",
  labels: [],
  type_id: null,
  status: "active",
  sort: "manual",
  descending: false,
  scroll_x: 0,
  scroll_y: 0,
  importance_min: null,
  importance_max: null,
  deadline_from: null,
  deadline_to: null,
  custom_fields: {},
});
export const fields = (ticket: Ticket) => ({
  title: ticket.title,
  description: ticket.description,
  importance: ticket.importance,
  type_id: ticket.details.type_id,
  labels: ticket.details.labels,
  custom_fields: ticket.details.custom_fields,
  deadline: ticket.details.deadline,
});
export const defaultOptions = (): BoardOptions => ({
  enabled_types: [],
  color: "#24533e",
  column_colors: {},
  undo_depth: 100,
  importance_weight: 1,
  type_weight: 1,
  deadline_weight: 1,
  deadline_warning_days: 3,
  auto_archive_days: null,
  trash_retention_days: null,
});
export const emptySnapshot = (bootstrap: Bootstrap): Snapshot => ({
  workspace: {
    boards: [],
    columns: [],
    tickets: [],
    types: bootstrap.types,
    labels: bootstrap.labels,
    comments: [],
    dependencies: [],
    links: [],
    board_options: bootstrap.board_options,
    views: bootstrap.views,
    settings: bootstrap.settings,
    recurrences: [],
  },
  external_references: [],
  revision: bootstrap.revision,
  settings: bootstrap.settings,
  view_states: bootstrap.views,
  epoch: bootstrap.epoch,
});
