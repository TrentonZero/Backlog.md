//! Task overview statistics shared by CLI, browser, and terminal surfaces.

use crate::records::TaskRecord;

const DAY_MS: i64 = 86_400_000;

#[derive(Clone, Debug, PartialEq)]
pub struct RecentActivity<'a> {
    pub created: Vec<&'a TaskRecord>,
    pub updated: Vec<&'a TaskRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectHealth<'a> {
    pub average_task_age: f64,
    pub stale_tasks: Vec<&'a TaskRecord>,
    pub blocked_tasks: Vec<&'a TaskRecord>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TaskStatistics<'a> {
    pub status_counts: Vec<(String, usize)>,
    pub priority_counts: Vec<(String, usize)>,
    pub no_priority_count: usize,
    pub total_tasks: usize,
    pub completed_tasks: usize,
    pub completion_percentage: usize,
    pub draft_count: usize,
    pub recent_activity: RecentActivity<'a>,
    pub project_health: ProjectHealth<'a>,
}

/// One task in the ordered statistics corpus, retaining its lifecycle origin.
#[derive(Clone, Copy, Debug)]
pub struct TaskStatisticsRecord<'a> {
    pub task: &'a TaskRecord,
    pub archived: bool,
}

/// Aggregate board statistics using the final configured status for archived-task inclusion.
///
/// `now_ms` is Unix time in milliseconds, supplied by the caller so date boundaries are testable.
pub fn get_task_statistics<'a>(
    records: &'a [TaskStatisticsRecord<'a>],
    draft_count: usize,
    statuses: &[String],
    priority_order: &[String],
    now_ms: i64,
) -> TaskStatistics<'a> {
    let terminal_status = statuses.last().filter(|status| !status.trim().is_empty());
    let mut status_counts = Vec::new();
    for status in statuses {
        set_count(&mut status_counts, status);
    }
    let configured_priorities = normalized_priorities(priority_order);
    let mut priority_counts = configured_priorities
        .iter()
        .map(|priority| (priority.clone(), 0))
        .collect::<Vec<_>>();
    let week_ago = now_ms - 7 * DAY_MS;
    let month_ago = now_ms - 30 * DAY_MS;
    let mut completed_tasks = 0;
    let mut no_priority_count = 0;
    let mut recently_created = Vec::new();
    let mut recently_updated = Vec::new();
    let mut stale_tasks = Vec::new();
    let mut blocked_tasks = Vec::new();
    let mut total_age_days = 0.0;
    let mut age_count = 0;

    for record in records.iter().filter(|record| {
        !record.archived
            || terminal_status.is_some_and(|terminal| {
                record
                    .task
                    .status
                    .trim()
                    .eq_ignore_ascii_case(terminal.trim())
            })
    }) {
        let task = record.task;
        if task.status.is_empty() {
            continue;
        }
        increment_count(&mut status_counts, &task.status);
        if task.status == "Done" {
            completed_tasks += 1;
        }

        if let Some(priority) = task.priority.as_deref().and_then(normalize_priority) {
            increment_count(&mut priority_counts, &priority);
        } else {
            no_priority_count += 1;
        }

        if !task.created_date.is_empty() {
            let created = parse_stored_utc_millis(&task.created_date);
            if let Some(date) = created.filter(|date| *date >= week_ago) {
                recently_created.push((task, date));
            }
            let age_days = if task.status == "Done" {
                if let Some(updated_date) =
                    task.updated_date.as_deref().filter(|date| !date.is_empty())
                {
                    parse_stored_utc_millis(updated_date)
                        .zip(created)
                        .map_or(f64::NAN, |(updated, created)| {
                            ((updated - created) as f64 / DAY_MS as f64).floor()
                        })
                } else {
                    created.map_or(f64::NAN, |created| {
                        ((now_ms - created) as f64 / DAY_MS as f64).floor()
                    })
                }
            } else {
                created.map_or(f64::NAN, |created| {
                    ((now_ms - created) as f64 / DAY_MS as f64).floor()
                })
            };
            // A malformed truthy date contributes NaN in the TypeScript calculation.
            total_age_days += age_days;
            age_count += 1;
        }

        if let Some(updated) = task
            .updated_date
            .as_deref()
            .filter(|date| !date.is_empty())
            .and_then(parse_stored_utc_millis)
            && updated >= week_ago
        {
            recently_updated.push((task, updated));
        }

        if task.status != "Done" {
            let last_date = task
                .updated_date
                .as_deref()
                .filter(|date| !date.is_empty())
                .unwrap_or(&task.created_date);
            if parse_stored_utc_millis(last_date).is_some_and(|date| date < month_ago) {
                stale_tasks.push(task);
            }
        }

        if !task.dependencies.is_empty()
            && task.status != "Done"
            && task.dependencies.iter().any(|dependency| {
                records
                    .iter()
                    .map(|record| record.task)
                    .find(|candidate| candidate.id == *dependency)
                    .is_some_and(|candidate| candidate.status != "Done")
            })
        {
            blocked_tasks.push(task);
        }
    }

    recently_created.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    recently_updated.sort_by_key(|entry| std::cmp::Reverse(entry.1));
    let total_tasks = status_counts.iter().map(|(_, count)| count).sum();
    let completion_percentage = if total_tasks == 0 {
        0
    } else {
        ((completed_tasks as f64 / total_tasks as f64) * 100.0).round() as usize
    };
    let average_task_age = if age_count == 0 {
        0.0
    } else {
        js_round(total_age_days / age_count as f64)
    };

    TaskStatistics {
        status_counts,
        priority_counts,
        no_priority_count,
        total_tasks,
        completed_tasks,
        completion_percentage,
        draft_count,
        recent_activity: RecentActivity {
            created: recently_created
                .into_iter()
                .take(5)
                .map(|(task, _)| task)
                .collect(),
            updated: recently_updated
                .into_iter()
                .take(5)
                .map(|(task, _)| task)
                .collect(),
        },
        project_health: ProjectHealth {
            average_task_age,
            stale_tasks: stale_tasks.into_iter().take(5).collect(),
            blocked_tasks: blocked_tasks.into_iter().take(5).collect(),
        },
    }
}

fn set_count(counts: &mut Vec<(String, usize)>, key: &str) {
    if !counts.iter().any(|(existing, _)| existing == key) {
        counts.push((key.to_owned(), 0));
    }
}

fn increment_count(counts: &mut Vec<(String, usize)>, key: &str) {
    if let Some((_, count)) = counts.iter_mut().find(|(existing, _)| existing == key) {
        *count += 1;
    } else {
        counts.push((key.to_owned(), 1));
    }
}

fn normalized_priorities(priorities: &[String]) -> Vec<String> {
    let defaults = ["high", "medium", "low"];
    if priorities.is_empty() {
        return defaults.into_iter().map(str::to_owned).collect();
    }
    let mut normalized = Vec::new();
    for priority in priorities
        .iter()
        .filter_map(|priority| normalize_priority(priority))
    {
        if !normalized.contains(&priority) {
            normalized.push(priority);
        }
    }
    if normalized.is_empty() {
        defaults.into_iter().map(str::to_owned).collect()
    } else {
        normalized
    }
}

fn normalize_priority(priority: &str) -> Option<String> {
    let value = priority
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    (!value.is_empty()).then_some(value)
}

fn js_round(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Parse the stored UTC date forms accepted by `parseStoredUtcDate` in the TypeScript core.
pub fn parse_stored_utc_millis(value: &str) -> Option<i64> {
    let value = value.trim();
    let bytes = value.as_bytes();
    if bytes.len() < 10
        || !digits(bytes, 0..4)
        || bytes[4] != b'-'
        || !digits(bytes, 5..7)
        || bytes[7] != b'-'
        || !digits(bytes, 8..10)
    {
        return None;
    }
    let year = number(bytes, 0..4)? as i64;
    let month = number(bytes, 5..7)? as i64;
    let day = number(bytes, 8..10)? as i64;
    if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
        return None;
    }
    let mut index = 10;
    let (hour, minute, second, millis) = if index == bytes.len() {
        (0_i64, 0_i64, 0_i64, 0_i64)
    } else {
        if bytes[index] == b'T' || bytes[index] == b't' {
            index += 1;
        } else if bytes[index] == b' ' || bytes[index] == b'\t' {
            while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
                index += 1;
            }
        } else {
            return None;
        }
        let hour = number(bytes, index..index + 2)? as i64;
        index += 2;
        if bytes.get(index) != Some(&b':') {
            return None;
        }
        index += 1;
        let minute = number(bytes, index..index + 2)? as i64;
        index += 2;
        let mut second = 0;
        let mut millis = 0;
        if bytes.get(index) == Some(&b':') {
            index += 1;
            second = number(bytes, index..index + 2)? as i64;
            index += 2;
            if bytes.get(index) == Some(&b'.') {
                index += 1;
                let fraction_start = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if fraction_start == index {
                    return None;
                }
                let fraction = &value[fraction_start..index];
                let fraction = format!("{fraction:0<3}");
                millis = fraction[..3].parse().ok()?;
            }
        }
        if hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        (hour, minute, second, millis)
    };
    while index < bytes.len() && (bytes[index] == b' ' || bytes[index] == b'\t') {
        index += 1;
    }
    let mut offset_minutes = 0_i64;
    if index < bytes.len() {
        if bytes[index] == b'Z' || bytes[index] == b'z' {
            index += 1;
        } else if bytes[index] == b'+' || bytes[index] == b'-' {
            let sign = if bytes[index] == b'+' { 1 } else { -1 };
            index += 1;
            let offset_hour = number(bytes, index..index + 2)? as i64;
            index += 2;
            if bytes.get(index) == Some(&b':') {
                index += 1;
            }
            let offset_minute = number(bytes, index..index + 2)? as i64;
            index += 2;
            if offset_hour > 23 || offset_minute > 59 {
                return None;
            }
            offset_minutes = sign * (offset_hour * 60 + offset_minute);
        } else {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * DAY_MS + ((hour * 60 + minute - offset_minutes) * 60 + second) * 1000 + millis)
}

fn digits(bytes: &[u8], range: std::ops::Range<usize>) -> bool {
    bytes
        .get(range)
        .is_some_and(|part| part.iter().all(u8::is_ascii_digit))
}

fn number(bytes: &[u8], range: std::ops::Range<usize>) -> Option<u32> {
    if !digits(bytes, range.clone()) {
        return None;
    }
    std::str::from_utf8(bytes.get(range)?).ok()?.parse().ok()
}

fn days_in_month(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = year - i64::from(month <= 2);
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let adjusted_month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * adjusted_month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}
