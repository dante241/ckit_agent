//! Goal tracking: requirements (UC) → phase ACs (CONTEXT) → PLAN tasks → commits →
//! VERIFICATION verdicts, and the three gates that keep them honest:
//! - **coverage** before the Reviewer: every phase AC has a task, every task an AC;
//! - **AC change** at phase close: an AC re-graded / reinterpreted / dropped needs
//!   the boss's approval (an `ac_change` card), never the team's own;
//! - **coordinator**: it may approve a plan for the boss only when the UC/AC set is
//!   what the boss last approved (`approved_snapshot`).
//!
//! Planning files are AI-written: parsing is lenient, unknown lines are skipped.
use std::collections::BTreeMap;

use super::ask::{AcChange, Ask};
use super::progress::{ac_rows, parse_plan, parse_roadmap, phase_dir, read_glob, tag, AcNote, SubTask};
use super::*;

// ── parsing ─────────────────────────────────────────────────────────────────

fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// FNV-1a 64 of the whitespace-normalized text: stable across builds (stored in task.json).
fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in norm(s).bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{h:016x}")
}

fn cells(row: &str) -> Vec<String> {
    let inner = row.trim().trim_start_matches('|');
    let inner = inner.strip_suffix('|').unwrap_or(inner);
    inner.split('|').map(|c| c.trim().to_string()).collect()
}

fn is_rule(c: &[String]) -> bool {
    c.iter().all(|x| !x.is_empty() && x.chars().all(|ch| matches!(ch, '-' | ':' | ' ')))
}

fn ac_id(c: &str) -> Option<String> {
    let c = c.trim().trim_matches('*').trim();
    let n = c.strip_prefix("AC-")?;
    (n.starts_with(|d: char| d.is_ascii_digit()) && n.chars().all(|ch| ch.is_ascii_alphanumeric())).then(|| c.to_string())
}

/// One AC of a phase CONTEXT table (`| AC | UC | Nguồn | GIVEN / WHEN / THEN | …`).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CtxAc {
    pub id: String,
    pub uc: Option<String>,
    pub source: String,
    pub text: String,
}

/// ACs of a phase from its CONTEXT AC table(s): the text column is the one headed
/// GIVEN/WHEN/THEN, tiêu chí, nội dung, mô tả…, else the longest cell.
pub(super) fn context_acs(md: &str) -> Vec<CtxAc> {
    let mut out: Vec<CtxAc> = Vec::new();
    let (mut header, mut prev): (Vec<String>, Vec<String>) = (Vec::new(), Vec::new());
    for l in md.lines() {
        let t = l.trim();
        if !t.starts_with('|') {
            header.clear();
            prev.clear();
            continue;
        }
        let c = cells(t);
        if is_rule(&c) {
            header = std::mem::take(&mut prev).iter().map(|h| h.to_lowercase()).collect();
            continue;
        }
        prev = c.clone();
        let Some(id) = c.first().and_then(|x| ac_id(x)) else { continue };
        let col = |names: &[&str]| header.iter().position(|h| names.iter().any(|n| h.contains(n)));
        let text_col = col(&["given", "then", "tiêu chí", "nội dung", "mô tả", "acceptance", "criteria", "điều kiện"])
            .or_else(|| (1..c.len()).max_by_key(|i| c[*i].chars().count()));
        let get = |i: Option<usize>| i.and_then(|i| c.get(i)).map(|s| s.trim().to_string()).filter(|s| !s.is_empty() && s != "—" && s != "-");
        let ac = CtxAc { id, uc: get(col(&["uc"])), source: get(col(&["nguồn", "source"])).unwrap_or_default(), text: get(text_col).unwrap_or_default() };
        match out.iter_mut().find(|o| o.id == ac.id) {
            Some(o) => *o = ac,
            None => out.push(ac),
        }
    }
    out
}

/// Requirement rows of REQUIREMENTS.md: table rows whose first cell is an id
/// (`UC1`, `UC-2`, `NFR1`, `FR3`, `BR-1`, `US-01`) and `- **UC1** …` / `### UC1 — …` lines.
pub(super) fn requirements(md: &str) -> Vec<(String, String)> {
    let is_id = |s: &str| {
        let s = s.trim().trim_matches('*').trim();
        let p = s.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end_matches('-');
        ["UC", "NFR", "FR", "BR", "US"].contains(&p) && s.len() > p.len() && s.ends_with(|c: char| c.is_ascii_digit())
    };
    let clean = |s: &str| norm(&s.replace("**", "")).trim_start_matches([':', '—', '-', '–', ' ']).to_string();
    let mut out: Vec<(String, String)> = Vec::new();
    for l in md.lines() {
        let t = l.trim();
        let row = if t.starts_with('|') {
            let c = cells(t);
            c.first().filter(|x| is_id(x)).map(|id| (id.trim_matches('*').to_string(), c.get(1).map(|x| clean(x)).unwrap_or_default()))
        } else {
            let s = t.trim_start_matches('#').trim_start_matches(['-', '*']).trim();
            let s2 = s.trim_start_matches("**");
            let word: String = s2.chars().take_while(|c| c.is_ascii_alphanumeric() || *c == '-').collect();
            (is_id(&word) && word.len() < s2.len()).then(|| (word.clone(), clean(&s2[word.len()..])))
        };
        if let Some((id, text)) = row.filter(|(_, x)| !x.is_empty()) {
            if !out.iter().any(|(i, _)| *i == id) {
                out.push((id, text));
            }
        }
    }
    out
}

/// ACs a PLAN `[AC: …]` tag names (`AC-01, AC-02`, `AC-01…AC-04`, `AC-16(d)`);
/// `tất cả` / `all` → every AC of the phase (`true`).
pub(super) fn expand_acs(s: &str) -> (Vec<String>, bool) {
    let low = s.to_lowercase();
    if ["tất cả", "all", "mọi ac"].iter().any(|w| low.contains(w)) {
        return (Vec::new(), true);
    }
    let mut out: Vec<String> = Vec::new();
    let mut prev: Option<(u32, usize, usize)> = None; // (number, width, end byte)
    for (i, _) in s.match_indices("AC-") {
        let digits: String = s[i + 3..].chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(n) = digits.parse::<u32>() else { continue };
        let w = digits.len();
        if let Some((p, pw, end)) = prev {
            let between = &s[end..i];
            if n > p && ["…", "..", "–", "đến", " to ", "-"].iter().any(|r| between.contains(r)) && !between.contains(',') {
                for k in p + 1..n {
                    out.push(format!("AC-{k:0pw$}"));
                }
            }
        }
        let id = format!("AC-{digits}");
        if !out.contains(&id) {
            out.push(id);
        }
        prev = Some((n, w, i + 3 + w));
    }
    (out, false)
}

/// A task may go without ACs only when marked `[tier: verify-only]` / infra.
fn exempt(task: &SubTask) -> bool {
    let tier = tag(&task.detail, "tier").to_lowercase();
    tier.contains("verify-only") || tier.contains("infra")
}

struct PhaseFacts {
    acs: Vec<CtxAc>,
    verdicts: Vec<AcNote>,
    tasks: Vec<SubTask>,
}

fn phase_facts(fdir: &FsPath, id: &str) -> Option<PhaseFacts> {
    let d = phase_dir(fdir, id)?;
    Some(PhaseFacts {
        acs: context_acs(&read_glob(&d, "-CONTEXT")),
        verdicts: ac_rows(&read_glob(&d, &format!("{id}-VERIFICATION"))),
        tasks: parse_plan(&read_glob(&d, "-PLAN")),
    })
}

fn covers(task: &SubTask, ac: &str) -> bool {
    let (ids, all) = expand_acs(&task.ac);
    all || ids.iter().any(|i| i == ac)
}

// ── coverage gate ───────────────────────────────────────────────────────────

/// Gaps of a phase plan: ACs no task tags, tasks tagging no AC (unless exempt).
pub(super) fn coverage_gaps(fdir: &FsPath, phase: &str) -> Vec<String> {
    let Some(p) = phase_facts(fdir, phase) else { return Vec::new() };
    if p.acs.is_empty() {
        return Vec::new(); // no AC table to check against
    }
    if p.tasks.is_empty() {
        return vec!["PLAN chưa có task nào".into()];
    }
    let mut gaps: Vec<String> = p.acs.iter().filter(|a| !p.tasks.iter().any(|t| covers(t, &a.id))).map(|a| format!("{} chưa có task", a.id)).collect();
    gaps.extend(p.tasks.iter().filter(|t| expand_acs(&t.ac) == (Vec::new(), false) && !exempt(t)).map(|t| format!("{} không gắn AC", t.key)));
    gaps
}

/// After a Kế hoạch run passed: a plan with coverage gaps goes back to Kế hoạch
/// (a review round spent) before the Reviewer ever sees it.
pub(super) fn coverage_gate(root: &FsPath, t: &mut Task) {
    if t.workflow.is_some() || !matches!((t.stage, t.status), (Stage::Review, Status::Queued) | (Stage::Approval, Status::Waiting)) {
        return;
    }
    let (Some(fdir), Some(phase)) = (feature_dir(root, t), active_phase(root, t)) else { return };
    let gaps = coverage_gaps(&fdir, &phase);
    if gaps.is_empty() {
        return;
    }
    let list = gaps.join("; ");
    t.say("system", &format!("Kế hoạch {phase} thiếu độ phủ AC ↔ task (Reviewer chưa chạy): {list}"));
    t.rounds.solution += 1;
    if t.rounds.solution >= MAX_SOLUTION_ROUNDS {
        if t.has(Stage::Approval) {
            t.set(Stage::Approval, Status::Waiting);
            t.note = Some(format!("Kế hoạch {phase} vẫn thiếu độ phủ sau {MAX_SOLUTION_ROUNDS} lần: {list}"));
        } else {
            t.set(Stage::Solution, Status::Failed);
            t.note = Some(format!("Kế hoạch {phase} thiếu độ phủ AC ↔ task sau {MAX_SOLUTION_ROUNDS} lần: {list}"));
        }
    } else {
        t.set(Stage::Solution, Status::Queued);
        t.note = None;
    }
}

// ── boss-approved snapshot / coordinator ────────────────────────────────────

/// Hashes of what the boss approved: REQUIREMENTS ids and each approved phase's ACs.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub(super) struct Snapshot {
    pub at: String,
    pub requirements: BTreeMap<String, String>,
    pub phases: BTreeMap<String, BTreeMap<String, String>>,
}

fn req_hashes(fdir: &FsPath) -> BTreeMap<String, String> {
    requirements(&std::fs::read_to_string(fdir.join("REQUIREMENTS.md")).unwrap_or_default()).into_iter().map(|(id, text)| (id, hash(&text))).collect()
}

fn phase_hashes(fdir: &FsPath, phase: &str) -> BTreeMap<String, String> {
    phase_dir(fdir, phase).map(|d| context_acs(&read_glob(&d, "-CONTEXT"))).unwrap_or_default().into_iter().map(|a| (a.id, hash(&a.text))).collect()
}

/// The boss approved the plan of the active phase: remember its UC/AC texts.
pub(super) fn record_snapshot(root: &FsPath, t: &mut Task) {
    let Some(fdir) = feature_dir(root, t) else { return };
    let mut s = t.approved_snapshot.take().unwrap_or_default();
    s.at = iso_now();
    s.requirements = req_hashes(&fdir);
    if let Some(phase) = active_phase(root, t) {
        s.phases.insert(phase.clone(), phase_hashes(&fdir, &phase));
    }
    t.approved_snapshot = Some(s);
}

fn diff(old: &BTreeMap<String, String>, new: &BTreeMap<String, String>) -> Vec<String> {
    let mut out: Vec<String> = new
        .iter()
        .filter_map(|(k, h)| match old.get(k) {
            None => Some(format!("{k} (thêm)")),
            Some(o) if o != h => Some(format!("{k} (sửa)")),
            _ => None,
        })
        .collect();
    out.extend(old.keys().filter(|k| !new.contains_key(*k)).map(|k| format!("{k} (bỏ)")));
    out
}

/// AC source cells other than the requirements themselves (`REQUIREMENTS + PO
/// REVIEW (NB-1)`, `PO Q-4`): ACs the team added or changed.
fn changed_source(source: &str) -> bool {
    let s = source.to_lowercase();
    !s.is_empty() && ["+", "q-", "review", "po", "boss", "sếp", "đổi", "mới", "thêm", "leader"].iter().any(|k| s.contains(k))
}

/// Why the coordinator may not approve this plan for the boss (None = it may).
pub(super) fn coordinator_blocker(root: &FsPath, t: &Task) -> Option<String> {
    let fdir = feature_dir(root, t)?;
    let Some(snap) = &t.approved_snapshot else {
        return Some("sếp chưa duyệt kế hoạch nào của việc này, chưa có mốc UC/AC để so".into());
    };
    let mut why: Vec<String> = Vec::new();
    let req = diff(&snap.requirements, &req_hashes(&fdir));
    if !req.is_empty() {
        why.push(format!("REQUIREMENTS đổi so với lần sếp duyệt: {}", req.join(", ")));
    }
    if let Some(phase) = active_phase(root, t) {
        let now = phase_hashes(&fdir, &phase);
        if let Some(old) = snap.phases.get(&phase) {
            let d = diff(old, &now);
            if !d.is_empty() {
                why.push(format!("AC của {phase} đổi so với lần sếp duyệt: {}", d.join(", ")));
            }
        }
        let added: Vec<String> = phase_dir(&fdir, &phase)
            .map(|d| context_acs(&read_glob(&d, "-CONTEXT")))
            .unwrap_or_default()
            .into_iter()
            .filter(|a| changed_source(&a.source))
            .map(|a| format!("{} ({})", a.id, a.source))
            .collect();
        if !added.is_empty() {
            why.push(format!("kế hoạch thêm/đổi AC ngoài REQUIREMENTS: {}", added.join(", ")));
        }
    }
    let open: Vec<String> = t.asks.iter().filter(|a| a.is_ac_change() && a.status == "boss").map(|a| a.id.clone()).collect();
    if !open.is_empty() {
        why.push(format!("còn yêu cầu đổi AC chờ sếp: {}", open.join(", ")));
    }
    (!why.is_empty()).then(|| why.join("; "))
}

// ── AC change gate ──────────────────────────────────────────────────────────

/// A Tester pass waiting for the boss to decide its AC changes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub(super) struct AcGate {
    pub phase: Option<String>,
    /// The Tester's route (`next_phase` / `done`), applied once approved.
    pub route: Option<String>,
    pub asks: Vec<String>,
}

/// An AC whose verdict the team bent: (id, why, the row).
pub(super) struct Bent {
    pub id: String,
    pub what: String,
    pub row: Option<AcNote>,
    pub text: String,
}

/// ACs of a phase changed without the plan's wording: a PASS carrying a regrade
/// note, or an AC text that differs from (or is missing versus) the boss's snapshot.
pub(super) fn bent_acs(fdir: &FsPath, phase: &str, snap: Option<&Snapshot>) -> Vec<Bent> {
    let Some(p) = phase_facts(fdir, phase) else { return Vec::new() };
    let text_of = |id: &str| p.acs.iter().find(|a| a.id == id).map(|a| a.text.clone()).unwrap_or_default();
    let mut out: Vec<Bent> = p
        .verdicts
        .iter()
        // Structured signal only: a PASS cell that carries a qualifier (`PASS (AC chấm
        // lại)`, `PASS (… → staging)`) passed something other than the AC as written.
        // A plain PASS never flags, whatever the evidence column says.
        .filter(|r| r.status == "PASS" && !r.note.is_empty())
        .map(|r| Bent { id: r.id.clone(), what: format!("PASS ({})", r.note), row: Some(r.clone()), text: text_of(&r.id) })
        .collect();
    if let Some(old) = snap.and_then(|s| s.phases.get(phase)) {
        let now: BTreeMap<String, String> = p.acs.iter().map(|a| (a.id.clone(), hash(&a.text))).collect();
        for (id, h) in old {
            if out.iter().any(|b| b.id == *id) {
                continue;
            }
            match now.get(id) {
                None => out.push(Bent { id: id.clone(), what: "AC bị bỏ khỏi CONTEXT".into(), row: None, text: String::new() }),
                Some(n) if n != h => out.push(Bent { id: id.clone(), what: "câu chữ AC đổi so với lần sếp duyệt".into(), row: p.verdicts.iter().find(|r| r.id == *id).cloned(), text: text_of(id) }),
                _ => {}
            }
        }
    }
    out
}

fn same_phase(a: &Ask, phase: Option<&str>) -> bool {
    a.phase.is_none() || phase.is_none() || a.phase.as_deref() == phase
}

/// The boss's latest decided AC change card for (phase, AC).
fn decided<'a>(t: &'a Task, phase: Option<&str>, ac: &str) -> Option<&'a Ask> {
    t.asks.iter().rev().find(|a| a.is_ac_change() && a.approved().is_some() && a.ac.as_ref().is_some_and(|c| c.ac == ac) && same_phase(a, phase))
}

/// At a Tester pass: hold the phase close while ACs changed without the boss's
/// approval (open one card per AC, reuse cards already open). Returns whether held.
pub(super) fn hold(root: &FsPath, t: &mut Task, route: Option<&str>) -> bool {
    let Some(fdir) = feature_dir(root, t) else { return false };
    let phase = timeline::run_phases(root, t).last().cloned().flatten();
    let Some(ph) = phase.clone() else { return false };
    let bent = bent_acs(&fdir, &ph, t.approved_snapshot.as_ref());
    let mut asks: Vec<String> = t.asks.iter().filter(|a| a.is_ac_change() && a.status == "boss" && same_phase(a, Some(&ph))).map(|a| a.id.clone()).collect();
    let mut acs: Vec<String> = Vec::new();
    for b in bent {
        if decided(t, Some(&ph), &b.id).is_some_and(|a| a.approved() == Some(true)) {
            continue;
        }
        acs.push(b.id.clone());
        let open = t.asks.iter().any(|a| a.is_ac_change() && a.status == "boss" && same_phase(a, Some(&ph)) && a.ac.as_ref().is_some_and(|c| c.ac == b.id));
        if open {
            continue;
        }
        let change = AcChange {
            ac: b.id.clone(),
            old: clip(&b.text, 400),
            new: b.what.clone(),
            reason: b.row.as_ref().map(|r| r.source.clone()).unwrap_or_default(),
            evidence: b.row.as_ref().map(|r| clip(&r.evidence, 300)).unwrap_or_default(),
        };
        if let Ok(id) = ask::open_ac_change(root, t, change, Some(ph.clone())) {
            asks.push(id);
        }
    }
    for a in t.asks.iter().filter(|a| asks.contains(&a.id)) {
        if let Some(c) = a.ac.as_ref().filter(|c| !acs.contains(&c.ac)) {
            acs.push(c.ac.clone());
        }
    }
    if asks.is_empty() {
        return false;
    }
    t.note = Some(format!("Chờ bạn duyệt thay đổi AC: {} ({ph})", acs.join(", ")));
    t.status = Status::Waiting;
    t.ac_gate = Some(AcGate { phase, route: route.map(str::to_string), asks });
    true
}

/// The boss answered an AC change card: once every card of the held close is
/// decided, approved → the Tester's pass applies; any rejected → back to Dev.
pub(super) fn resolve_gate(root: &FsPath, t: &mut Task) {
    let Some(gate) = t.ac_gate.clone() else { return };
    if t.status != Status::Waiting {
        return;
    }
    let cards: Vec<&Ask> = t.asks.iter().filter(|a| gate.asks.contains(&a.id)).collect();
    let open: Vec<String> = cards.iter().filter(|a| a.status == "boss").filter_map(|a| a.ac.as_ref().map(|c| c.ac.clone())).collect();
    let ph = gate.phase.clone().unwrap_or_default();
    if !open.is_empty() {
        t.note = Some(format!("Chờ bạn duyệt thay đổi AC: {} ({ph})", open.join(", ")));
        return;
    }
    let line = |a: &Ask| {
        let ac = a.ac.as_ref().map(|c| c.ac.clone()).unwrap_or_default();
        let answer = a.answer.as_deref().unwrap_or("").trim().to_string();
        format!("{ac} ({answer})")
    };
    // A card closed without the boss (cancel / old engine) counts as not approved.
    let rejected: Vec<String> = cards.iter().filter(|a| a.approved() != Some(true)).map(|a| line(a)).collect();
    let approved: Vec<String> = cards.iter().filter(|a| a.approved() == Some(true)).map(|a| line(a)).collect();
    t.ac_gate = None;
    if !approved.is_empty() {
        if let (Some(fdir), Some(s), Some(p)) = (feature_dir(root, t), t.approved_snapshot.as_mut(), gate.phase.as_ref()) {
            // The approved wording is the new baseline for this phase.
            s.phases.insert(p.clone(), phase_hashes(&fdir, p));
        }
    }
    if !rejected.is_empty() {
        t.say("system", &format!("Bạn không duyệt đổi AC {ph}: {} → Dev làm lại cho đạt đúng AC.", rejected.join("; ")));
        t.set(Stage::Dev, Status::Queued);
        t.note = None;
        return;
    }
    t.say("system", &format!("Bạn đã duyệt đổi AC {ph}: {}.", approved.join("; ")));
    t.note = None;
    t.stage = Stage::Test;
    after_run(t, "pass", gate.route.as_deref());
    park_if_finished(root, t);
}

// ── /goals ──────────────────────────────────────────────────────────────────

/// `M1/T4` named as a word in a commit subject (`feat(M1/T4): …`, not `M1/T40`).
fn names_scoped(subject: &str, key: &str) -> bool {
    subject.match_indices(key).any(|(i, _)| !subject[i + key.len()..].starts_with(|c: char| c.is_ascii_digit()))
}

/// The traceability view: requirements → ACs (with tasks, commits, verdicts and
/// changes) → orphans; plus the held AC gate.
pub(super) fn goals(root: &FsPath, t: &Task) -> serde_json::Value {
    let empty = serde_json::json!({ "requirements": [], "acs": [], "orphans": { "acs_without_task": [], "tasks_without_ac": [] }, "gate": t.ac_gate });
    let Some(fdir) = feature_dir(root, t) else { return empty };
    let reqs: Vec<serde_json::Value> = requirements(&std::fs::read_to_string(fdir.join("REQUIREMENTS.md")).unwrap_or_default())
        .into_iter()
        .map(|(id, text)| serde_json::json!({ "id": id, "text": text }))
        .collect();
    let commits = timeline::task_commits(root, t);
    let roadmap = std::fs::read_to_string(fdir.join("ROADMAP.md")).unwrap_or_default();
    let phase_ids: Vec<String> = parse_roadmap(&roadmap).into_iter().map(|p| p.id).collect();
    let (mut acs, mut no_task, mut no_ac) = (Vec::new(), Vec::new(), Vec::new());
    for ph in &phase_ids {
        let Some(p) = phase_facts(&fdir, ph) else { continue };
        let bent = bent_acs(&fdir, ph, t.approved_snapshot.as_ref());
        let mut ids: Vec<String> = p.acs.iter().map(|a| a.id.clone()).collect();
        ids.extend(p.verdicts.iter().map(|r| r.id.clone()).filter(|i| !p.acs.iter().any(|a| a.id == *i)));
        for id in ids {
            let ctx = p.acs.iter().find(|a| a.id == id);
            let row = p.verdicts.iter().find(|r| r.id == id);
            let tasks: Vec<&SubTask> = p.tasks.iter().filter(|tk| covers(tk, &id)).collect();
            let task_shas: Vec<serde_json::Value> = {
                let mut out: Vec<serde_json::Value> = Vec::new();
                for tk in &tasks {
                    let scoped = format!("{ph}/{}", tk.key);
                    let mut push = |sha: &str, subject: &str| {
                        if !out.iter().any(|o| o["sha"].as_str().is_some_and(|s| s.starts_with(sha) || sha.starts_with(s))) {
                            out.push(serde_json::json!({ "sha": sha, "subject": subject }));
                        }
                    };
                    if let Some(sha) = &tk.sha {
                        let subject = commits.iter().find(|c| c.sha.starts_with(sha.as_str()) || sha.starts_with(c.sha.as_str())).map(|c| c.subject.clone()).unwrap_or_default();
                        push(sha, &subject);
                    }
                    for c in commits.iter().rev().filter(|c| names_scoped(&c.subject, &scoped)) {
                        push(&c.sha, &c.subject);
                    }
                }
                out
            };
            let card = t.asks.iter().rev().find(|a| a.is_ac_change() && a.ac.as_ref().is_some_and(|c| c.ac == id) && same_phase(a, Some(ph)));
            let is_bent = bent.iter().any(|b| b.id == id);
            let changed = (is_bent || card.is_some()).then(|| {
                let decision = card.map(|a| match (a.status.as_str(), a.approved()) {
                    (_, Some(true)) => "approved",
                    (_, Some(false)) => "rejected",
                    ("boss", _) => "pending",
                    _ => "rejected",
                });
                serde_json::json!({
                    "approved_by": card.filter(|a| a.approved() == Some(true)).map(|_| "boss"),
                    "at": card.and_then(|a| a.answered_at.clone()),
                    "ask": card.map(|a| a.id.clone()),
                    "decision": decision,
                })
            });
            let status = match row.map(|r| r.status) {
                Some("PASS") => "pass",
                Some("FAIL") => "fail",
                Some(_) => "needs_confirm",
                None => "pending",
            };
            if ctx.is_some() && tasks.is_empty() && !p.tasks.is_empty() {
                no_task.push(format!("{ph}/{id}"));
            }
            acs.push(serde_json::json!({
                "id": id, "phase": ph,
                "text": ctx.map(|a| a.text.clone()).unwrap_or_default(),
                "uc": ctx.and_then(|a| a.uc.clone()),
                "source": ctx.map(|a| a.source.clone()).filter(|s| !s.is_empty()).or_else(|| row.map(|r| r.source.clone()).filter(|s| !s.is_empty())),
                "status": status,
                "note": row.map(|r| r.note.clone()).filter(|n| !n.is_empty()),
                "changed": changed,
                "tasks": tasks.iter().map(|tk| format!("{ph}/{}", tk.key)).collect::<Vec<_>>(),
                "commits": task_shas,
                "evidence": row.map(|r| r.evidence.clone()).filter(|e| !e.is_empty()),
            }));
        }
        no_ac.extend(p.tasks.iter().filter(|tk| expand_acs(&tk.ac) == (Vec::new(), false) && !exempt(tk)).map(|tk| format!("{ph}/{}", tk.key)));
    }
    serde_json::json!({
        "requirements": reqs, "acs": acs,
        "orphans": { "acs_without_task": no_task, "tasks_without_ac": no_ac },
        "gate": t.ac_gate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTEXT: &str = "## AC\n| AC | UC | Nguồn | GIVEN / WHEN / THEN (đo được) | Cách verify | Task |\n|---|---|---|---|---|---|\n\
| AC-01 | UC2 | REQUIREMENTS | WHEN list THEN nq ≤ 39 | bash | T7 |\n\
| AC-02 | UC2 | REQUIREMENTS | WHEN calendar THEN nq ≤ 51 | bash | T8 |\n\
| AC-03 | UC3 | REQUIREMENTS + PO REVIEW (NB-1) | WHEN more THEN 8 record giống | bash | T6 |\n";

    #[test]
    fn context_requirements_and_ac_tags_parse() {
        let a = context_acs(CONTEXT);
        assert_eq!(a.len(), 3);
        assert_eq!((a[0].id.as_str(), a[0].uc.as_deref(), a[0].source.as_str(), a[0].text.as_str()), ("AC-01", Some("UC2"), "REQUIREMENTS", "WHEN list THEN nq ≤ 39"));
        let req = "| ID | Ràng buộc |\n|---|---|\n| NFR1 | Không đổi hành vi |\n\n| UC | Mô tả | Phase |\n|---|---|---|\n| UC1 | **Harness phủ luồng core.** Thêm kịch bản | M0 |\n- **UC9** — bullet form\n";
        assert_eq!(requirements(req), [("NFR1".into(), "Không đổi hành vi".into()), ("UC1".into(), "Harness phủ luồng core. Thêm kịch bản".into()), ("UC9".into(), "bullet form".into())]);
        assert_eq!(expand_acs("AC-04, AC-05, AC-16(d)"), (vec!["AC-04".into(), "AC-05".into(), "AC-16".into()], false));
        assert_eq!(expand_acs("AC-01…AC-03, AC-07").0, ["AC-01", "AC-02", "AC-03", "AC-07"]);
        assert_eq!(expand_acs("tất cả"), (Vec::new(), true));
        assert_eq!(expand_acs("—"), (Vec::new(), false));
    }

    fn fixture(tag: &str, plan: &str, verification: &str) -> (PathBuf, Task) {
        let root = std::env::temp_dir().join(format!("ckit-goals-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let f = root.join("agents/planning/board-t-001");
        std::fs::create_dir_all(f.join("phases/M1-read")).unwrap();
        std::fs::write(f.join("ROADMAP.md"), "- [~] **M1 Read** — x\n").unwrap();
        std::fs::write(f.join("STATE.md"), "---\nactive_phase: M1\n---\n").unwrap();
        std::fs::write(f.join("REQUIREMENTS.md"), "| UC | Mô tả |\n|---|---|\n| UC2 | List core |\n| UC3 | Detail core |\n").unwrap();
        std::fs::write(f.join("phases/M1-read/M1-CONTEXT.md"), CONTEXT).unwrap();
        std::fs::write(f.join("phases/M1-read/M1-01-PLAN.md"), plan).unwrap();
        std::fs::write(f.join("phases/M1-read/M1-VERIFICATION.md"), verification).unwrap();
        let mut t = super::super::tests::task(Stage::Solution, Status::Running);
        t.feature = Some("board-t-001".into());
        (root, t)
    }

    const PLAN_OK: &str = "- [x] T1 (abc1234): list [AC: AC-01]\n- [x] T2: calendar [AC: AC-02, AC-03]\n- [x] T3: đo lại [tier: verify-only] [AC: —]\n";
    const VERIF: &str = "| UC | AC | Nguồn | Verdict | Bằng chứng |\n|---|---|---|---|---|\n| UC2 | AC-01 | REQUIREMENTS | PASS | nq 38 |\n| UC2 | AC-02 | REQUIREMENTS + PO Q-4 | PASS (AC chấm lại) | nq 63 |\n";

    #[test]
    fn coverage_gate_sends_a_plan_with_gaps_back_without_the_reviewer() {
        let (root, mut t) = fixture("cov", "- [ ] T1: list [AC: AC-01]\n- [ ] T2: dọn code\n- [ ] T3: đo [tier: verify-only] [AC: —]\n", "");
        let fdir = feature_dir(&root, &t).unwrap();
        assert_eq!(coverage_gaps(&fdir, "M1"), ["AC-02 chưa có task", "AC-03 chưa có task", "T2 không gắn AC"]);
        after_run(&mut t, "pass", None); // Solution pass → Review queued
        assert_eq!(t.stage, Stage::Review);
        coverage_gate(&root, &mut t);
        assert_eq!((t.stage, t.status, t.rounds.solution), (Stage::Solution, Status::Queued, 1));
        assert!(t.thread.last().unwrap().text.contains("AC-02 chưa có task"), "{:?}", t.thread.last());
        // A covered plan passes untouched.
        std::fs::write(fdir.join("phases/M1-read/M1-01-PLAN.md"), PLAN_OK).unwrap();
        t.set(Stage::Review, Status::Queued);
        coverage_gate(&root, &mut t);
        assert_eq!(t.stage, Stage::Review);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ac_change_gate_holds_close_then_approve_continues_and_reject_goes_to_dev() {
        let (root, mut t) = fixture("gate", PLAN_OK, VERIF);
        t.set(Stage::Test, Status::Running);
        t.runs.push(Run { stage: Stage::Test, n: 1, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: "test-1.jsonl".into(), phase: Some("M1".into()), round: None, title: None });
        assert!(hold(&root, &mut t, Some("next_phase")));
        assert_eq!((t.stage, t.status, t.note.as_deref()), (Stage::Test, Status::Waiting, Some("Chờ bạn duyệt thay đổi AC: AC-02 (M1)")));
        let q = t.ac_gate.clone().unwrap().asks[0].clone();
        assert_eq!(t.asks[0].ac.as_ref().map(|c| c.ac.as_str()), Some("AC-02"));
        assert!(t.asks[0].question.contains("PASS (AC chấm lại)") && t.asks[0].question.contains("nq 63"), "{}", t.asks[0].question);
        // Run end does not close it; no answer → still held.
        ask::close_open(&root, &mut t);
        assert_eq!(t.asks[0].status, "boss");
        // Approve → the Tester's next_phase route applies.
        let mut ok = t.clone();
        ask::answer_as(&root, &mut ok, &q, "Duyệt, đúng tinh thần AC", "boss").unwrap();
        assert_eq!((ok.stage, ok.status, ok.ac_gate.is_none()), (Stage::Solution, Status::Queued, true));
        // A second close of the same phase does not ask again.
        ok.set(Stage::Test, Status::Running);
        assert!(!hold(&root, &mut ok, Some("done")));
        // Reject → back to Dev with the comment.
        let (root2, mut t2) = fixture("gate2", PLAN_OK, VERIF);
        t2.set(Stage::Test, Status::Running);
        t2.runs = t.runs.clone();
        assert!(hold(&root2, &mut t2, Some("next_phase")));
        let q2 = t2.ac_gate.clone().unwrap().asks[0].clone();
        ask::answer_as(&root2, &mut t2, &q2, "Không duyệt: phải đạt nq ≤ 51", "boss").unwrap();
        assert_eq!((t2.stage, t2.status), (Stage::Dev, Status::Queued));
        assert!(t2.thread.last().unwrap().text.contains("phải đạt nq ≤ 51"));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&root2);
    }

    /// Real-data shapes that must NOT open an AC change card: free text with
    /// "không đổi hành vi" in the evidence, a plain `✅ PASS`, a cell passing only
    /// in part (graded NEEDS-CONFIRM, which keeps the phase open by itself), and a
    /// rounds table listing ACs in prose. Only `PASS (<qualifier>)` flags.
    #[test]
    fn only_qualified_pass_cells_flag_an_ac_change() {
        let verif = "| UC | AC | Nguồn | Verdict | Bằng chứng |\n|---|---|---|---|---|\n\
| UC2 | AC-01 | REQUIREMENTS | PASS | không đổi hành vi, bỏ 2 query thay bằng 1 MGET, nới timeout test |\n\
| UC2 | AC-02 | REQUIREMENTS | ✅ PASS | ok |\n\
| UC3 | **AC-03** — More 8 record | REQUIREMENTS | ✅ PASS (BE) / ⏳ NEEDS-CONFIRM (FE E2E) | x |\n\n\
| Round | AC/finding | Task | Kết quả |\n|---|---|---|---|\n| — | AC-01, AC-02: câu chữ đổi | không | PASS lại |\n";
        let (root, mut t) = fixture("fp", PLAN_OK, verif);
        let fdir = feature_dir(&root, &t).unwrap();
        assert!(bent_acs(&fdir, "M1", None).is_empty());
        t.set(Stage::Test, Status::Running);
        t.runs.push(Run { stage: Stage::Test, n: 1, started: iso_now(), ended: None, exit: None, cost: 0.0, verdict: None, log: "test-1.jsonl".into(), phase: Some("M1".into()), round: None, title: None });
        assert!(!hold(&root, &mut t, Some("done")), "no card, no hold");
        let rows = ac_rows(verif);
        let got: Vec<(&str, &str)> = rows.iter().map(|r| (r.id.as_str(), r.status)).collect();
        assert_eq!(got, [("AC-01", "PASS"), ("AC-02", "PASS"), ("AC-03", "NEEDS-CONFIRM")], "label after the id is read, prose lists are not rows");
        // A deferral written as a PASS qualifier does flag.
        std::fs::write(fdir.join("phases/M1-read/M1-VERIFICATION.md"), "| AC | Verdict |\n|---|---|\n| AC-01 | ✅ PASS (positive live-render → staging) |\n").unwrap();
        let b = bent_acs(&fdir, "M1", None);
        assert_eq!((b.len(), b[0].id.as_str(), b[0].what.as_str()), (1, "AC-01", "PASS (positive live-render → staging)"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn ac_change_never_times_out() {
        let (root, mut t) = fixture("timeout", PLAN_OK, VERIF);
        t.set(Stage::Test, Status::Running);
        let dir = ask::ask_dir(&root, &t.id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("Q-1.json"), serde_json::json!({ "id": "Q-1", "to": "leader", "kind": "ac_change", "ac": { "ac": "AC-02", "old": "nq ≤ 51", "new": "nq ≤ 65", "reason": "đo được", "evidence": "nq 63" } }).to_string()).unwrap();
        std::fs::write(dir.join("A-1.json"), serde_json::json!({ "answer": "", "by": "timeout" }).to_string()).unwrap();
        ask::close_open(&root, &mut t);
        assert_eq!((t.asks[0].to.as_str(), t.asks[0].status.as_str(), t.asks[0].options.clone()), ("boss", "boss", vec!["Duyệt".to_string(), "Không duyệt".to_string()]), "routed to the boss, timeout ignored, not closed");
        assert!(!dir.join("A-1.json").exists(), "the timeout answer is cleared for the boss");
        assert!(t.asks[0].question.starts_with("[Đổi AC] AC-02\nĐang là: nq ≤ 51\nĐề xuất: nq ≤ 65"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn coordinator_may_approve_only_what_the_boss_approved() {
        let (root, mut t) = fixture("coord", PLAN_OK, "");
        assert!(coordinator_blocker(&root, &t).unwrap().contains("chưa duyệt kế hoạch nào"));
        record_snapshot(&root, &mut t);
        // AC-03 comes from a PO review, not the requirements.
        assert!(coordinator_blocker(&root, &t).unwrap().contains("AC-03 (REQUIREMENTS + PO REVIEW (NB-1))"));
        let f = feature_dir(&root, &t).unwrap();
        let plain = CONTEXT.replace("REQUIREMENTS + PO REVIEW (NB-1)", "REQUIREMENTS");
        std::fs::write(f.join("phases/M1-read/M1-CONTEXT.md"), &plain).unwrap();
        record_snapshot(&root, &mut t);
        assert_eq!(coordinator_blocker(&root, &t), None);
        std::fs::write(f.join("phases/M1-read/M1-CONTEXT.md"), plain.replace("nq ≤ 51", "nq ≤ 65")).unwrap();
        std::fs::write(f.join("REQUIREMENTS.md"), "| UC | Mô tả |\n|---|---|\n| UC2 | List core |\n| UC3 | Detail core |\n| UC8 | Mới |\n").unwrap();
        let why = coordinator_blocker(&root, &t).unwrap();
        assert!(why.contains("UC8 (thêm)") && why.contains("AC-02 (sửa)"), "{why}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn goals_trace_requirements_acs_tasks_commits_and_orphans() {
        let (root, mut t) = fixture("goals", "- [x] T1 (abc1234): list [AC: AC-01]\n- [x] T2: calendar [AC: AC-02]\n- [x] T4: dọn code\n", VERIF);
        t.asks.push(Ask {
            id: "Q-3".into(), from: Stage::Test, to: "boss".into(), question: "x".into(), options: Vec::new(), at: iso_now(), status: "answered".into(),
            answer: Some("Duyệt".into()), by: Some("boss".into()), answered_at: Some("2026-10-15T22:13:50Z".into()),
            kind: Some("ac_change".into()), ac: Some(AcChange { ac: "AC-02".into(), ..Default::default() }), phase: Some("M1".into()),
        });
        let g = goals(&root, &t);
        assert_eq!(g["requirements"][0], serde_json::json!({ "id": "UC2", "text": "List core" }));
        let acs = g["acs"].as_array().unwrap();
        assert_eq!(acs.len(), 3);
        assert_eq!((acs[0]["status"].as_str(), acs[0]["tasks"].clone(), acs[0]["commits"][0]["sha"].as_str(), acs[0]["evidence"].as_str()), (Some("pass"), serde_json::json!(["M1/T1"]), Some("abc1234"), Some("nq 38")));
        assert_eq!(acs[1]["changed"], serde_json::json!({ "approved_by": "boss", "at": "2026-10-15T22:13:50Z", "ask": "Q-3", "decision": "approved" }));
        assert_eq!((acs[2]["status"].as_str(), acs[2]["changed"].is_null()), (Some("pending"), true));
        assert_eq!(g["orphans"], serde_json::json!({ "acs_without_task": ["M1/AC-03"], "tasks_without_ac": ["M1/T4"] }));
        let _ = std::fs::remove_dir_all(&root);
    }
}
