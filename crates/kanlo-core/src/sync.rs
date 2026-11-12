use crate::*;
use chrono::{
    DateTime, Datelike, Days, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub op_id: String,
    pub client_id: String,
    pub server_instance_id: String,
    pub base_epoch: String,
    pub entity_id: String,
    pub base_revision: i64,
    pub base_fields: BTreeMap<String, Value>,
    pub changed_fields: BTreeMap<String, Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FieldConflict {
    pub field: String,
    pub base: Value,
    pub server: Value,
    pub local: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MergeResult {
    pub fields: BTreeMap<String, Value>,
    pub conflicts: Vec<FieldConflict>,
}

/// A field merges only if server still equals base, or both writers chose the same value.
pub fn merge(
    base: &BTreeMap<String, Value>,
    local: &BTreeMap<String, Value>,
    server: &BTreeMap<String, Value>,
) -> Result<MergeResult> {
    let mut result = MergeResult {
        fields: BTreeMap::new(),
        conflicts: vec![],
    };
    for (key, value) in local {
        let original = base
            .get(key)
            .ok_or_else(|| Error::Validation(format!("Missing base field: {key}")))?;
        let current = server
            .get(key)
            .ok_or_else(|| Error::Validation(format!("Unknown field: {key}")))?;
        if current == original || current == value {
            result.fields.insert(key.clone(), value.clone());
        } else {
            result.conflicts.push(FieldConflict {
                field: key.clone(),
                base: original.clone(),
                server: current.clone(),
                local: value.clone(),
            });
        }
    }
    Ok(result)
}

pub fn deadline(value: &Option<String>, timezone: &str) -> Option<DateTime<Utc>> {
    let value = value.as_ref()?;
    if let Ok(value) = DateTime::parse_from_rfc3339(value) {
        return Some(value.with_timezone(&Utc));
    }
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
    let zone = timezone.parse::<chrono_tz::Tz>().ok()?;
    let next_day = date.checked_add_days(Days::new(1))?;
    let midnight = next_day.and_hms_opt(0, 0, 0)?;
    resolve_local(zone, midnight).map(|dt| dt - chrono::Duration::nanoseconds(1))
}

fn resolve_local(zone: chrono_tz::Tz, local: NaiveDateTime) -> Option<DateTime<Utc>> {
    // DST gaps move to the first valid minute; ambiguous times use the earlier occurrence.
    for minute in 0..=180 {
        let local = local.checked_add_signed(chrono::Duration::minutes(minute))?;
        match zone.from_local_datetime(&local) {
            LocalResult::Single(dt) => return Some(dt.with_timezone(&Utc)),
            LocalResult::Ambiguous(a, b) => return Some(a.min(b).with_timezone(&Utc)),
            LocalResult::None => {}
        }
    }
    None
}

pub fn validate_recurrence(rule: &Recurrence) -> Result<()> {
    let fail = || Error::Validation("Invalid recurrence schedule".into());
    if !["daily", "weekly", "monthly", "yearly"].contains(&rule.frequency.as_str())
        || rule.interval == 0
        || rule.interval > 1000
    {
        return Err(fail());
    }
    if rule.weekdays.iter().any(|d| *d > 6)
        || rule.month_day.is_some_and(|d| !(1..=31).contains(&d))
        || rule
            .nth_weekday
            .is_some_and(|(nth, weekday)| nth == 0 || !(-5..=5).contains(&nth) || weekday > 6)
    {
        return Err(fail());
    }
    NaiveTime::parse_from_str(&rule.time, "%H:%M").map_err(|_| fail())?;
    rule.timezone.parse::<chrono_tz::Tz>().map_err(|_| fail())?;
    DateTime::parse_from_rfc3339(&rule.next_at).map_err(|_| fail())?;
    if let Some(anchor) = &rule.anchor_at {
        DateTime::parse_from_rfc3339(anchor).map_err(|_| fail())?;
    }
    Ok(())
}

pub fn recurrence_next(rule: &Recurrence, after: DateTime<Utc>) -> Result<DateTime<Utc>> {
    validate_recurrence(rule)?;
    let zone: chrono_tz::Tz = rule
        .timezone
        .parse()
        .map_err(|_| Error::Validation("Invalid timezone".into()))?;
    let anchor = DateTime::parse_from_rfc3339(rule.anchor_at.as_deref().unwrap_or(&rule.next_at))
        .map_err(|_| Error::Validation("Invalid next_at".into()))?
        .with_timezone(&zone);
    let time = NaiveTime::parse_from_str(&rule.time, "%H:%M")
        .map_err(|_| Error::Validation("Invalid time".into()))?;
    let start = after
        .with_timezone(&zone)
        .date_naive()
        .max(anchor.date_naive());
    for day in 0..=366000 {
        let Some(date) = start.checked_add_days(Days::new(day)) else {
            break;
        };
        let days = date.signed_duration_since(anchor.date_naive()).num_days();
        let months =
            (date.year() - anchor.year()) * 12 + date.month() as i32 - anchor.month() as i32;
        let interval = rule.interval as i64;
        let weekday = date.weekday().num_days_from_monday() as u8;
        let month_last = date
            .with_day(28)
            .and_then(|d| d.checked_add_days(Days::new(4)))
            .and_then(|d| d.with_day(1))
            .and_then(|d| d.pred_opt())
            .map_or(28, |d| d.day());
        let selected_day = rule.month_day.unwrap_or(anchor.day() as u8) as u32;
        let day_matches = if let Some((nth, wanted)) = rule.nth_weekday {
            weekday == wanted
                && if nth > 0 {
                    ((date.day() - 1) / 7 + 1) as i8 == nth
                } else {
                    -(((month_last - date.day()) / 7 + 1) as i8) == nth
                }
        } else {
            date.day() == selected_day.min(month_last)
        };
        let matches = match rule.frequency.as_str() {
            "daily" => days >= 0 && days % interval == 0,
            "weekly" => {
                let anchor_monday = anchor.date_naive()
                    - chrono::Duration::days(anchor.weekday().num_days_from_monday() as i64);
                let weeks = date.signed_duration_since(anchor_monday).num_days() / 7;
                weeks >= 0
                    && weeks % interval == 0
                    && if rule.weekdays.is_empty() {
                        weekday == anchor.weekday().num_days_from_monday() as u8
                    } else {
                        rule.weekdays.contains(&weekday)
                    }
            }
            "monthly" => months >= 0 && months as i64 % interval == 0 && day_matches,
            "yearly" => {
                date.month() == anchor.month()
                    && (date.year() - anchor.year()) as i64 % interval == 0
                    && day_matches
            }
            _ => false,
        };
        if matches
            && let Some(candidate) = resolve_local(zone, date.and_time(time))
            && candidate > after
        {
            return Ok(candidate);
        }
    }
    Err(Error::Validation(
        "No recurrence within supported 1000-year range".into(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn independent_fields_merge_but_same_field_conflicts() {
        let base = BTreeMap::from([
            ("title".into(), json!("old")),
            ("importance".into(), json!(1)),
        ]);
        let server = BTreeMap::from([
            ("title".into(), json!("server")),
            ("importance".into(), json!(1)),
        ]);
        let result = merge(
            &base,
            &BTreeMap::from([("importance".into(), json!(2))]),
            &server,
        )
        .unwrap();
        assert!(result.conflicts.is_empty());
        assert_eq!(result.fields["importance"], 2);
        let result = merge(
            &base,
            &BTreeMap::from([("title".into(), json!("local"))]),
            &server,
        )
        .unwrap();
        assert_eq!(result.conflicts.len(), 1);
        assert_eq!(result.conflicts[0].server, "server");
    }

    #[test]
    fn date_only_deadline_is_end_of_local_dst_day() {
        assert_eq!(
            deadline(&Some("2026-03-29".into()), "Europe/Berlin")
                .unwrap()
                .to_rfc3339(),
            "2026-03-29T21:59:59.999999999+00:00"
        );
        assert_eq!(
            deadline(&Some("2026-10-25".into()), "Europe/Berlin")
                .unwrap()
                .to_rfc3339(),
            "2026-10-25T22:59:59.999999999+00:00"
        );
    }

    #[test]
    fn monthly_second_monday_and_dst_gap() {
        let mut rule = Recurrence {
            id: "r".into(),
            template_id: "t".into(),
            frequency: "monthly".into(),
            interval: 1,
            weekdays: Default::default(),
            month_day: None,
            nth_weekday: Some((2, 0)),
            time: "09:00".into(),
            timezone: "Europe/Berlin".into(),
            next_at: "2026-11-09T08:00:00Z".into(),
            anchor_at: None,
            after_completion: false,
            last_created: None,
            enabled: true,
        };
        assert_eq!(
            recurrence_next(&rule, "2026-11-09T08:00:00Z".parse().unwrap())
                .unwrap()
                .to_rfc3339(),
            "2026-12-14T08:00:00+00:00"
        );
        rule.frequency = "daily".into();
        rule.nth_weekday = None;
        rule.time = "02:30".into();
        rule.next_at = "2026-03-28T01:30:00Z".into();
        assert_eq!(
            recurrence_next(&rule, "2026-03-28T01:30:00Z".parse().unwrap())
                .unwrap()
                .to_rfc3339(),
            "2026-03-29T01:00:00+00:00"
        );
    }
}
