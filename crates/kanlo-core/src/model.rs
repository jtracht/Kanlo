use crate::{Board, Column, Ticket};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketDetails {
    pub parent_id: Option<String>,
    pub type_id: Option<String>,
    pub labels: BTreeSet<String>,
    pub custom_fields: BTreeMap<String, String>,
    pub deadline: Option<String>,
    pub archived: bool,
    pub trashed: bool,
    pub deleted_at: Option<String>,
    #[serde(default)]
    pub completed_at: Option<String>,
    pub rank: i64,
    pub last_active_column: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketType {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: String,
    pub rank: u8,
    pub allow_children: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub id: String,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Comment {
    pub id: String,
    pub ticket_id: String,
    pub markdown: String,
    pub created_at: String,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, schemars::JsonSchema,
)]
pub struct Dependency {
    pub ticket_id: String,
    pub prerequisite_id: String,
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, schemars::JsonSchema,
)]
pub struct TicketLink {
    pub ticket_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct BoardOptions {
    pub enabled_types: BTreeSet<String>,
    pub color: String,
    pub column_colors: BTreeMap<String, String>,
    pub undo_depth: usize,
    pub importance_weight: u16,
    pub type_weight: u16,
    pub deadline_weight: u16,
    pub deadline_warning_days: u16,
    pub auto_archive_days: Option<u16>,
    pub trash_retention_days: Option<u16>,
}
impl Default for BoardOptions {
    fn default() -> Self {
        Self {
            enabled_types: BTreeSet::new(),
            color: "#24533e".into(),
            column_colors: BTreeMap::new(),
            undo_depth: 100,
            importance_weight: 1,
            type_weight: 1,
            deadline_weight: 1,
            deadline_warning_days: 3,
            auto_archive_days: None,
            trash_retention_days: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct ViewState {
    pub board_id: String,
    pub query: String,
    pub labels: BTreeSet<String>,
    pub type_id: Option<String>,
    pub status: String,
    pub importance_min: Option<u8>,
    pub importance_max: Option<u8>,
    pub deadline_from: Option<String>,
    pub deadline_to: Option<String>,
    pub custom_fields: BTreeMap<String, String>,
    pub sort: String,
    pub descending: bool,
    pub scroll_x: f64,
    pub scroll_y: f64,
}
impl Default for ViewState {
    fn default() -> Self {
        Self {
            board_id: String::new(),
            query: String::new(),
            labels: BTreeSet::new(),
            type_id: None,
            status: "active".into(),
            importance_min: None,
            importance_max: None,
            deadline_from: None,
            deadline_to: None,
            custom_fields: BTreeMap::new(),
            sort: "manual".into(),
            descending: false,
            scroll_x: 0.0,
            scroll_y: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub language: String,
    pub theme: String,
    pub timezone: String,
    pub animation: String,
    pub accent_color: String,
    pub backup_every_hours: Option<u16>,
    pub backup_keep: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "de".into(),
            theme: "light".into(),
            timezone: "Europe/Berlin".into(),
            animation: "fluid".into(),
            accent_color: "#24533e".into(),
            backup_every_hours: None,
            backup_keep: 7,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recurrence {
    pub id: String,
    pub template_id: String,
    pub frequency: String,
    pub interval: u16,
    pub weekdays: BTreeSet<u8>,
    pub month_day: Option<u8>,
    pub nth_weekday: Option<(i8, u8)>,
    pub time: String,
    pub timezone: String,
    pub next_at: String,
    #[serde(default)]
    pub anchor_at: Option<String>,
    pub after_completion: bool,
    pub last_created: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[derive(Default)]
pub struct Workspace {
    pub boards: Vec<Board>,
    pub columns: Vec<Column>,
    pub tickets: Vec<Ticket>,
    pub types: Vec<TicketType>,
    pub labels: Vec<Label>,
    pub comments: Vec<Comment>,
    pub dependencies: BTreeSet<Dependency>,
    pub links: BTreeSet<TicketLink>,
    pub board_options: BTreeMap<String, BoardOptions>,
    pub views: BTreeMap<String, ViewState>,
    pub settings: Settings,
    pub recurrences: Vec<Recurrence>,
    pub issued_prefixes: BTreeSet<String>,
    pub issued_ticket_ids: BTreeSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    CreateBoard {
        name: String,
        prefix: String,
    },
    UpdateBoard {
        id: String,
        name: String,
        base_revision: i64,
    },
    DeleteBoard {
        id: String,
        base_revision: i64,
    },
    CreateColumn {
        board_id: String,
        name: String,
        is_done: bool,
        wip_limit: Option<u32>,
        wip_strict: bool,
    },
    UpdateColumn {
        id: String,
        name: String,
        is_done: bool,
        wip_limit: Option<u32>,
        wip_strict: bool,
        color: String,
    },
    DeleteColumn {
        id: String,
    },
    ReorderColumns {
        board_id: String,
        ids: Vec<String>,
    },
    CreateTicket {
        board_id: String,
        title: String,
        description: String,
        importance: u8,
        column_id: Option<String>,
        parent_id: Option<String>,
        type_id: Option<String>,
    },
    PatchTicket {
        id: String,
        base_revision: i64,
        fields: BTreeMap<String, serde_json::Value>,
    },
    MoveTicket {
        id: String,
        base_revision: i64,
        board_id: String,
        column_id: Option<String>,
        activate_types: bool,
        convert_types: BTreeMap<String, Option<String>>,
    },
    SetDone {
        id: String,
        base_revision: i64,
        done: bool,
    },
    ReorderTickets {
        board_id: String,
        column_id: Option<String>,
        ids: Vec<String>,
    },
    UpsertType {
        value: TicketType,
    },
    DeleteType {
        id: String,
    },
    UpsertLabel {
        value: Label,
    },
    DeleteLabel {
        id: String,
    },
    SetBoardOptions {
        board_id: String,
        options: BoardOptions,
    },
    AddDependency {
        ticket_id: String,
        prerequisite_id: String,
    },
    RemoveDependency {
        ticket_id: String,
        prerequisite_id: String,
    },
    AddLink {
        ticket_id: String,
        target_id: String,
    },
    RemoveLink {
        ticket_id: String,
        target_id: String,
    },
    AddComment {
        ticket_id: String,
        markdown: String,
    },
    DeleteComment {
        id: String,
    },
    Trash {
        ids: Vec<String>,
    },
    Restore {
        ids: Vec<String>,
    },
    Purge {
        ids: Vec<String>,
    },
    Archive {
        ids: Vec<String>,
        archived: bool,
    },
    CloneTicket {
        id: String,
    },
    SetSettings {
        settings: Settings,
    },
    SetView {
        context: String,
        view: ViewState,
    },
    UpsertRecurrence {
        value: Recurrence,
    },
    DeleteRecurrence {
        id: String,
    },
    Batch {
        commands: Vec<Command>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, schemars::JsonSchema)]
pub struct ChangeResult {
    pub entity_id: Option<String>,
    pub affected: BTreeSet<String>,
    pub boards: BTreeSet<String>,
    pub warnings: Vec<String>,
}
