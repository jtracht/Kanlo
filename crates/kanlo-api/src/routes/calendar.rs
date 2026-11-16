use super::*;
use kanlo_db::calendar::{Assignment, Draft, Entry, PlannedTicket, valid_day};
use reqwest::Url;
use serde::Serialize;
use std::time::Duration;

pub(super) async fn entries(State(db): State<Database>) -> Result<Json<Vec<Entry>>, ApiError> {
    Ok(Json(database(db, |store| store.calendar()).await?))
}

pub(super) async fn add(
    State(db): State<Database>,
    Json(draft): Json<Draft>,
) -> Result<Json<Entry>, ApiError> {
    Ok(Json(
        database(db, move |store| store.add_calendar_entry(draft)).await?,
    ))
}

pub(super) async fn planning_tickets(
    State(db): State<Database>,
) -> Result<Json<Vec<PlannedTicket>>, ApiError> {
    Ok(Json(database(db, |store| store.planning_tickets()).await?))
}

pub(super) async fn assign_ticket(
    State(db): State<Database>,
    Json(assignment): Json<Assignment>,
) -> Result<Json<Value>, ApiError> {
    database(db, move |store| store.assign_planning_ticket(assignment)).await?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct Milestone {
    id: u64,
    title: String,
    due_on: Option<chrono::DateTime<chrono::FixedOffset>>,
    state: String,
}

#[derive(Serialize, schemars::JsonSchema)]
pub(super) struct CalendarMilestone {
    id: u64,
    title: String,
    end: Option<String>,
    closed: bool,
}

#[derive(Serialize, schemars::JsonSchema)]
pub(in crate::routes) struct GiteaCalendar {
    configured: bool,
    milestones: Vec<CalendarMilestone>,
}

fn configuration_error() -> ApiError {
    ApiError(Error::Validation(
        "Invalid Gitea calendar configuration".into(),
    ))
}

pub(super) async fn gitea() -> Result<Json<GiteaCalendar>, ApiError> {
    let mut result = GiteaCalendar {
        configured: false,
        milestones: Vec::new(),
    };
    let Ok(origin) = std::env::var("KANLO_GITEA_URL") else {
        return Ok(Json(result));
    };
    let repository = std::env::var("KANLO_GITEA_REPOSITORY").map_err(|_| configuration_error())?;
    let (owner, repo) = repository.split_once('/').ok_or_else(configuration_error)?;
    let valid_segment = |segment: &str| {
        !segment.is_empty()
            && !matches!(segment, "." | "..")
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte))
    };
    let mut url = Url::parse(&origin).map_err(|_| configuration_error())?;
    if url.scheme() != "https"
        || url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !valid_segment(owner)
        || !valid_segment(repo)
    {
        return Err(configuration_error());
    }
    url.path_segments_mut()
        .map_err(|_| configuration_error())?
        .pop_if_empty()
        .extend(["api", "v1", "repos", owner, repo, "milestones"]);
    let token = std::env::var("KANLO_GITEA_TOKEN").unwrap_or_default();
    let upstream_error = |_| {
        ApiError(Error::Validation(
            "Gitea milestones could not be loaded".into(),
        ))
    };
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(upstream_error)?;
    result.configured = true;
    for page in 1..=10 {
        let mut request = client.get(url.clone()).query(&[
            ("state", "all"),
            ("limit", "50"),
            ("page", &page.to_string()),
        ]);
        if !token.is_empty() {
            request = request.header(reqwest::header::AUTHORIZATION, format!("token {token}"));
        }
        let mut response = request.send().await.map_err(upstream_error)?;
        if !response.status().is_success() {
            return Err(ApiError(Error::Validation(format!(
                "Gitea returned HTTP {}",
                response.status().as_u16()
            ))));
        }
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(upstream_error)? {
            if body.len() + chunk.len() > 2 * 1024 * 1024 {
                return Err(ApiError(Error::Validation(
                    "Gitea response is too large".into(),
                )));
            }
            body.extend_from_slice(&chunk);
        }
        let milestones: Vec<Milestone> = serde_json::from_slice(&body)
            .map_err(|_| ApiError(Error::Validation("Invalid Gitea milestone response".into())))?;
        let count = milestones.len();
        result.milestones.extend(milestones.into_iter().map(|item| {
            CalendarMilestone {
                id: item.id,
                title: item.title,
                end: item
                    .due_on
                    .map(|date| date.date_naive().to_string())
                    .filter(|day| day.as_str() > "1970-01-01" && valid_day(day)),
                closed: item.state == "closed",
            }
        }));
        if count < 50 {
            break;
        }
    }
    Ok(Json(result))
}
