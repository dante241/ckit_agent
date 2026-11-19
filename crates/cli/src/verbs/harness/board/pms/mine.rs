//! "Ticket của tôi": the boss's open tickets in each role, grouped by what needs
//! doing (`GET /api/board/pms/mine`), plus due dates on demand (`/pms/dates`).
//!
//! `getListTicket` returns no dates and no role ids, but it filters on both, so
//! every group is its own filtered query: roles × {all, changed, late, soon, stale}.
//! The rows a group shows first get their "Ngày mong muốn" from `retrieveTicket`.

use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use super::*;

/// Role → the ticket field holding that person.
pub(super) const ROLES: [(&str, &str); 3] = [("processing", "processing_user_id"), ("verification", "verification_user_id"), ("reception", "reception_user_id")];
/// More open tickets than this in one role are cut (the rest stay reachable through search).
const ROLE_ROWS: u64 = 500;
/// Overdue by more than this many days: "Treo lâu", out of "Quá hạn".
const LATE_DAYS: i64 = 90;
/// "Sắp hạn": due today or within this many days.
const SOON_DAYS: i64 = 3;
/// Rows per group that arrive with their due date; the client asks `/pms/dates` for more.
pub(super) const DATED: usize = 10;
/// Statuses where the verifier is the one holding the ticket up.
const VERIFY: [&str; 2] = ["Wait For Verifying", "Testing"];
const DATE_TTL: Duration = Duration::from_secs(600);

/// `YYYY-MM-DD` → days since 1970-01-01 (proleptic Gregorian).
pub(super) fn day_number(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * ((m + 9) % 12) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    Some(era * 146_097 + doe - 719_468)
}

/// Days since 1970-01-01 → `YYYY-MM-DD`.
pub(super) fn date_of(n: i64) -> String {
    let z = n + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// `YYYY-MM-DD HH:MM:SS`, digits where digits belong (it goes into a PMS filter).
pub(super) fn is_stamp(s: &str) -> bool {
    s.len() == 19 && s.bytes().enumerate().all(|(i, c)| match i {
        4 | 7 => c == b'-',
        10 => c == b' ',
        13 | 16 => c == b':',
        _ => c.is_ascii_digit(),
    })
}

/// The queries for one role: (group, extra filters, order column, direction).
/// `today` is the boss's local date; PMS compares `expected_date` as a plain date.
pub(super) fn group_queries(today: i64, since: Option<&str>) -> Vec<(&'static str, Vec<Value>, &'static str, &'static str)> {
    let cond = |name: &str, value: String, op: &str| json!({ "name": name, "value": value, "operator": op });
    let range = |a: i64, b: i64| format!("{},{}", date_of(a), date_of(b));
    let mut q = vec![
        ("all", vec![], "modifiedtime", "DESC"),
        ("late", vec![cond("expected_date", range(today - LATE_DAYS, today - 1), "bw")], "expected_date", "ASC"),
        ("soon", vec![cond("expected_date", range(today, today + SOON_DAYS), "bw")], "expected_date", "ASC"),
        ("stale", vec![cond("expected_date", date_of(today - LATE_DAYS), "l")], "expected_date", "ASC"),
    ];
    if let Some(s) = since {
        q.push(("changed", vec![cond("modifiedtime", s.to_string(), "a")], "modifiedtime", "DESC"));
    }
    q
}

/// Order of the groups on screen; a ticket sits in the first one it qualifies for.
pub(super) const GROUPS: [&str; 6] = ["changed", "late", "soon", "verify", "rest", "stale"];

#[derive(Serialize, Debug)]
pub(super) struct Row {
    #[serde(flatten)]
    pub(super) item: Item,
    /// The boss's roles on this ticket (`processing` / `verification` / `reception`).
    pub(super) roles: Vec<&'static str>,
}

#[derive(Serialize, Debug)]
pub(super) struct Group {
    pub(super) key: &'static str,
    pub(super) items: Vec<Row>,
}

/// One role's query results: group → ticket rows in PMS order.
pub(super) type RoleHits = HashMap<&'static str, Vec<Item>>;

/// Merge per-role results into the screen's groups. Roles merge in `ROLES` order;
/// within a group a ticket keeps the order of the first role that returned it.
pub(super) fn assemble(hits: Vec<(&'static str, RoleHits)>) -> Vec<Group> {
    let mut roles: HashMap<String, Vec<&'static str>> = HashMap::new();
    for (role, h) in &hits {
        for it in h.get("all").into_iter().flatten() {
            roles.entry(it.id.clone()).or_default().push(role);
        }
    }
    let mut taken: HashSet<String> = HashSet::new();
    let mut by_id: HashMap<String, Item> = HashMap::new();
    let mut order: HashMap<&'static str, Vec<String>> = HashMap::new();
    for (role, h) in hits {
        for (g, items) in h {
            for it in items {
                let ids = order.entry(g).or_default();
                if !ids.contains(&it.id) {
                    ids.push(it.id.clone());
                }
                if role == "verification" && g == "all" && VERIFY.contains(&it.status.as_str()) {
                    let v = order.entry("verify").or_default();
                    if !v.contains(&it.id) {
                        v.push(it.id.clone());
                    }
                }
                by_id.entry(it.id.clone()).or_insert(it);
            }
        }
    }
    // "Còn lại" is the base list minus every other group, so it claims last.
    let mut claimed: HashMap<&'static str, Vec<Row>> = HashMap::new();
    for key in ["changed", "late", "soon", "verify", "stale", "rest"] {
        let src = order.get(if key == "rest" { "all" } else { key }).cloned().unwrap_or_default();
        let items = src
            .into_iter()
            // Only tickets still open in some role: a date query may return one the base list cut.
            .filter(|id| roles.contains_key(id) && taken.insert(id.clone()))
            .filter_map(|id| {
                let it = by_id.remove(&id)?;
                Some(Row { roles: roles.get(&id).cloned().unwrap_or_default(), item: it })
            })
            .collect();
        claimed.insert(key, items);
    }
    GROUPS.iter().map(|&key| Group { key, items: claimed.remove(key).unwrap_or_default() }).collect()
}

fn dates_cache() -> &'static Mutex<HashMap<String, (Instant, String)>> {
    static C: OnceLock<Mutex<HashMap<String, (Instant, String)>>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(HashMap::new()))
}

/// "Ngày mong muốn" of each ticket id, from `retrieveTicket` 8 at a time, cached 10 min.
/// A failed lookup is left out.
pub(super) fn ticket_dates(c: &Creds, ids: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut need = Vec::new();
    {
        let g = dates_cache().lock().unwrap_or_else(|e| e.into_inner());
        for id in ids {
            match g.get(id) {
                Some((at, d)) if at.elapsed() < DATE_TTL => {
                    out.insert(id.clone(), d.clone());
                }
                _ => need.push(id.clone()),
            }
        }
    }
    for chunk in need.chunks(8) {
        let got: Vec<(String, Option<String>)> = std::thread::scope(|s| {
            let jobs: Vec<_> = chunk
                .iter()
                .map(|id| {
                    let n = id.parse::<u64>().unwrap_or(0);
                    (id.clone(), s.spawn(move || cloudwork(c, "retrieveTicket", json!({ "id": n })).ok().map(|d| field(d.get("data").unwrap_or(&d), "expected_date"))))
                })
                .collect();
            jobs.into_iter().map(|(id, j)| (id, j.join().ok().flatten())).collect()
        });
        let mut g = dates_cache().lock().unwrap_or_else(|e| e.into_inner());
        for (id, d) in got {
            if let Some(d) = d {
                g.insert(id.clone(), (Instant::now(), d.clone()));
                out.insert(id, d);
            }
        }
    }
    out
}

#[derive(Deserialize)]
pub(super) struct MineQ {
    /// `all` (default) | `processing` | `verification` | `reception`.
    #[serde(default)]
    role: String,
    /// The boss's local date, `YYYY-MM-DD`.
    #[serde(default)]
    today: String,
    /// Last look, `YYYY-MM-DD HH:MM:SS`; empty = no "Mới đổi" group.
    #[serde(default)]
    since: String,
}

pub(super) async fn api_mine(Query(q): Query<MineQ>) -> ApiRes<Value> {
    let today = day_number(&q.today).ok_or_else(|| err(StatusCode::BAD_REQUEST, "Ngày hôm nay không hợp lệ"))?;
    let role = match q.role.as_str() {
        "" | "all" => None,
        r => Some(ROLES.iter().find(|(n, _)| *n == r).map(|(n, _)| *n).ok_or_else(|| err(StatusCode::BAD_REQUEST, "Vai trò không hợp lệ"))?),
    };
    let since = Some(q.since.trim()).filter(|s| !s.is_empty());
    if since.is_some_and(|s| !is_stamp(s)) {
        return Err(err(StatusCode::BAD_REQUEST, "Mốc thời gian không hợp lệ"));
    }
    let since = since.map(String::from);
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || {
        let me = session(&c, false)?;
        let open = json!({ "name": "ticketstatus", "value": "Closed,Cancel,Reject", "operator": "n" });
        let queries = group_queries(today, since.as_deref());
        // Every role's "all" query runs for the chip counts; the group queries only for the roles shown.
        let jobs: Vec<(&'static str, &'static str, Value)> = ROLES
            .iter()
            .flat_map(|&(name, col)| {
                let who = json!({ "name": col, "value": me.uid, "operator": "e" });
                let open = open.clone();
                queries
                    .iter()
                    .filter(move |(g, ..)| *g == "all" || role.is_none_or(|r| r == name))
                    .map(move |(g, extra, by, dir)| {
                        let mut filters = vec![who.clone(), open.clone()];
                        filters.extend(extra.iter().cloned());
                        (name, *g, json!({ "filters": filters, "paging": { "offset": 0, "max_rows": ROLE_ROWS, "order_by": by, "sort_order": dir } }))
                    })
            })
            .collect();
        let results: Vec<(&'static str, &'static str, Result<Value, ApiErr>)> = std::thread::scope(|s| {
            let handles: Vec<_> = jobs.into_iter().map(|(r, g, p)| (r, g, s.spawn(|| cloudwork(&c, "getListTicket", p)))).collect();
            handles.into_iter().map(|(r, g, h)| (r, g, h.join().unwrap_or_else(|_| Err(no_answer())))).collect()
        });
        let mut counts = serde_json::Map::new();
        let mut hits: Vec<(&'static str, RoleHits)> = ROLES.iter().map(|(n, _)| (*n, RoleHits::new())).collect();
        for (r, g, res) in results {
            let data = res?;
            let (items, _) = parse_list("ticket", &c.base, &data);
            if g == "all" {
                let total = field(data.get("paging").unwrap_or(&Value::Null), "total_count").parse::<u64>().unwrap_or(items.len() as u64);
                counts.insert(r.to_string(), json!(total));
            }
            if role.is_none_or(|x| x == r) {
                if let Some((_, h)) = hits.iter_mut().find(|(n, _)| *n == r) {
                    h.insert(g, items);
                }
            }
        }
        let mut groups = assemble(hits);
        // The rows each group shows first come with their due date.
        let first: Vec<String> = groups.iter().filter(|g| g.key != "rest").flat_map(|g| g.items.iter().take(DATED).map(|r| r.item.id.clone())).collect();
        let dates = ticket_dates(&c, &first);
        for row in groups.iter_mut().flat_map(|g| g.items.iter_mut()) {
            if let Some(d) = dates.get(&row.item.id) {
                row.item.due = d.clone();
            }
        }
        Ok(json!({ "uid": me.uid, "name": me.name, "counts": counts, "groups": groups, "dated": DATED }))
    })
    .await
}

#[derive(Deserialize)]
pub(super) struct DatesQ {
    /// Comma-separated ticket ids, at most 40.
    #[serde(default)]
    ids: String,
}

pub(super) async fn api_dates(Query(q): Query<DatesQ>) -> ApiRes<HashMap<String, String>> {
    let ids: Vec<String> = q.ids.split(',').map(str::trim).filter(|s| !s.is_empty()).map(String::from).collect();
    if ids.len() > 40 || ids.iter().any(|i| !is_id(i)) {
        return Err(err(StatusCode::BAD_REQUEST, "Danh sách ticket không hợp lệ"));
    }
    let c = creds().ok_or_else(not_configured)?;
    blocking(move || Ok(ticket_dates(&c, &ids))).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_dates_roundtrip() {
        assert_eq!(day_number("1970-01-01"), Some(0));
        assert_eq!(date_of(day_number("2026-11-19").unwrap() + 12), "2026-12-01");
        assert_eq!(date_of(day_number("2024-03-01").unwrap() - 1), "2024-02-29");
        assert_eq!(date_of(day_number("2027-01-02").unwrap() - 2), "2026-12-31");
        assert_eq!(day_number("2026-13-01"), None);
        assert_eq!(day_number("2026-1-01"), None);
    }

    #[test]
    fn stamp_check() {
        assert!(is_stamp("2026-11-18 17:42:00"));
        assert!(!is_stamp("2026-11-18 17:42"));
        assert!(!is_stamp("2026-11-18'17:42:00"));
    }

    #[test]
    fn date_windows() {
        let q = group_queries(day_number("2026-11-19").unwrap(), Some("2026-11-18 17:42:00"));
        let f = |g: &str| q.iter().find(|x| x.0 == g).map(|x| x.1.clone()).unwrap();
        assert_eq!(f("late")[0]["value"], json!("2026-08-21,2026-11-18"));
        assert_eq!(f("soon")[0]["value"], json!("2026-11-19,2026-11-22"));
        assert_eq!(f("stale")[0], json!({ "name": "expected_date", "value": "2026-08-21", "operator": "l" }));
        assert_eq!(f("changed")[0]["operator"], json!("a"));
        assert!(group_queries(0, None).iter().all(|x| x.0 != "changed"));
    }

    fn it(id: &str, status: &str) -> Item {
        Item { kind: "ticket", id: id.into(), status: status.into(), ..Item::default() }
    }

    #[test]
    fn each_ticket_lands_in_its_first_group() {
        let p: RoleHits = [
            ("all", vec![it("1", "Assigned"), it("2", "Assigned"), it("3", "Assigned"), it("4", "Assigned")]),
            ("changed", vec![it("2", "Assigned")]),
            ("late", vec![it("2", "Assigned"), it("3", "Assigned")]),
            // A date query hit on a ticket not open in any role is dropped.
            ("soon", vec![it("9", "Assigned")]),
            ("stale", vec![it("4", "Assigned")]),
        ]
        .into_iter()
        .collect();
        let v: RoleHits = [("all", vec![it("5", "Testing"), it("1", "Assigned")])].into_iter().collect();
        let g = assemble(vec![("processing", p), ("verification", v)]);
        let ids = |k: &str| g.iter().find(|x| x.key == k).unwrap().items.iter().map(|r| r.item.id.as_str()).collect::<Vec<_>>();
        assert_eq!(ids("changed"), ["2"]);
        assert_eq!(ids("late"), ["3"]);
        assert!(ids("soon").is_empty());
        assert_eq!(ids("verify"), ["5"]);
        assert_eq!(ids("rest"), ["1"]);
        assert_eq!(ids("stale"), ["4"]);
        let one = &g.iter().find(|x| x.key == "rest").unwrap().items[0];
        assert_eq!(one.roles, ["processing", "verification"]);
    }
}
