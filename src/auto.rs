//! Project-scoped idle compilation; state survives plugin instance reclamation.
use super::*;
use openidea_plugin::{Event, EventKind, NotificationLevel, StatusItem};
use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const IDLE_MS: u64 = 5_000;

#[derive(Debug, Serialize, Deserialize)]
pub(super) struct State {
    enabled: bool,
    #[serde(default)]
    manual_override: Option<bool>,
    dirty_at: Option<u64>,
    running: Option<String>,
    fingerprints: BTreeMap<String, Option<u64>>,
    last_compile_at: Option<u64>,
    compile_started_at: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            enabled: true,
            manual_override: None,
            dirty_at: None,
            running: None,
            fingerprints: BTreeMap::new(),
            last_compile_at: None,
            compile_started_at: None,
        }
    }
}

impl State {
    fn changed(&mut self, path: String, hash: Option<u64>, now: u64) {
        if self.fingerprints.get(&path) == Some(&hash) { return; }
        self.fingerprints.insert(path, hash);
        if self.enabled { self.dirty_at = Some(now); }
    }

    fn due(&self, now: u64) -> bool {
        self.enabled && self.running.is_none()
            && self.dirty_at.is_some_and(|t| now.saturating_sub(t) >= IDLE_MS)
    }

    fn started(&mut self, id: String) {
        self.running = Some(id);
        self.compile_started_at = Some(now());
        self.dirty_at = None;
    }

    fn finished(&mut self, id: &str) -> bool {
        if self.running.as_deref() != Some(id) { return false; }
        self.running = None;
        true
    }

    fn pause(&mut self) {
        self.enabled = false;
        self.dirty_at = None;
    }
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64
}

fn key(root: &str) -> String { format!("auto:{}", project_key(root)) }

pub(super) fn reset(root: &str) -> Result<(), Error> {
    host::storage::delete(&key(root))
}

pub(super) fn view_status(root: &str) -> Result<String, Error> {
    let s = read(root)?;
    let state = if s.running.is_some() { "编译中" } else if s.enabled { "自动编译已开启" } else { "自动编译已暂停" };
    let last = s.last_compile_at.map(|t| compile_time_text(Some(t))).unwrap_or_else(|| "暂无成功记录".into());
    Ok(format!("{state} · 上次成功：{last}"))
}

pub(super) fn view_enabled(root: &str) -> Result<bool, Error> {
    Ok(read(root)?.enabled)
}

fn read(root: &str) -> Result<State, Error> {
    let Some(json) = host::storage::get(&key(root))? else { return Ok(State::default()); };
    let mut state: State = serde_json::from_str(&json).map_err(|e| Error::Failed(e.to_string()))?;
    // Older versions persisted the default-off state, so that value cannot
    // distinguish a deliberate pause from a project that was never enabled.
    state.enabled = state.manual_override.unwrap_or(true);
    Ok(state)
}

fn persist(root: &str, s: &State) -> Result<(), Error> {
    host::storage::set(&key(root), &serde_json::to_string(s).map_err(|e| e.to_string())?)
}

fn path_key(path: &str) -> String {
    path.replace('\\', "/").trim_start_matches("//?/").to_owned()
}

fn relevant(root: &str, path: &str, c: &Config) -> bool {
    let root = path_key(root);
    let path = path_key(path);
    let Some(rel) = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))) else { return false; };
    c.modules.iter().filter(|m| m.enabled).any(|m| {
        let module = relative(&m.path).unwrap_or_default();
        let prefix = if module.is_empty() { String::new() } else { format!("{module}/") };
        let Some(file) = rel.strip_prefix(&prefix) else { return false; };
        if file.contains("/target/") || file.starts_with("target/") || file.starts_with("build/")
            || file.ends_with("rebel.xml") || file.contains(".jrebel-backup-") { return false; }
        (file.starts_with("src/main/java/") && file.ends_with(".java"))
            || (file.starts_with("src/main/kotlin/") && file.ends_with(".kt"))
            || m.resources.iter().any(|r| file.starts_with(&format!("{}/", r.trim_end_matches('/'))))
    })
}

fn fingerprint(p: &Project, path: &str) -> Result<Option<u64>, Error> {
    match p.read_file(path) {
        Ok(text) => {
            let mut hash = std::collections::hash_map::DefaultHasher::new();
            text.hash(&mut hash);
            Ok(Some(hash.finish()))
        }
        Err(Error::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
    }
}

fn snapshot(p: &Project, root: &str, c: &Config, s: &mut State, seed: bool) -> Result<(), Error> {
    for editor in host::open_editors()? {
        if relevant(root, &editor.path, c) {
            let path = path_key(&editor.path);
            let hash = fingerprint(p, &editor.path)?;
            if seed { s.fingerprints.insert(path, hash); }
            else { s.changed(path, hash, now()); }
        }
    }
    Ok(())
}

pub(super) fn compile_time_text(timestamp: Option<u64>) -> String {
    let Some(timestamp) = timestamp else { return "暂无".into(); };
    let seconds = timestamp / 1000 + 8 * 3600;
    let days = seconds / 86400;
    let (year, month, day) = civil_date(days as i64);
    format!("{year}年{month}月{day}日 {:02}:{:02}:{:02}（北京时间）", seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}

fn compile_time_short(timestamp: u64) -> String {
    let seconds = timestamp / 1000 + 8 * 3600;
    let days = seconds / 86400;
    let (_, month, day) = civil_date(days as i64);
    format!("{month:02}/{day:02} {:02}:{:02}", seconds / 3600 % 24, seconds / 60 % 60)
}

fn civil_date(days: i64) -> (i64, i64, i64) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    (y + i64::from(m <= 2), m, d)
}

fn status(s: &State, detail: &str) {
    let state = if s.running.is_some() { "Echo: 编译中" }
        else if !s.enabled { "Echo 自动: 关" }
        else if s.dirty_at.is_some() { "Echo 自动: 等待5秒" }
        else { "Echo 自动: 开" };
    let text = match s.last_compile_at {
        Some(at) => format!("Echo JRebel · {}", compile_time_short(at)),
        None => state.to_owned(),
    };
    let _ = host::set_window_status_item("auto", &StatusItem {
        text, tooltip: Some(format!("{detail}\n上次编译时间：{}\n点击开启/暂停。开启后空闲5秒执行 Save All，仅编译所选模块。", compile_time_text(s.last_compile_at))),
        command: Some("auto-toggle".into()),
    });
}

pub(super) fn refresh_status(root: &str) -> Result<(), Error> {
    let s = read(root)?;
    status(&s, "项目自动编译状态");
    Ok(())
}

pub(super) fn subscribe() -> Result<(), Error> {
    for kind in ["document-changed", "file-saved", "files-changed"] {
        host::subscribe_paths(kind, &["**/src/main/**"])?;
    }
    host::subscribe(EventKind::FileOpened)?;
    host::subscribe(EventKind::RunExited)?;
    Ok(())
}

pub(super) fn toggle(p: &Project, c: &Config) -> Result<Option<String>, Error> {
    let root = p.root()?;
    let mut s = read(&root)?;
    if s.enabled { s.pause(); }
    else {
        if !c.enabled { return Err("先启用当前项目的 JRebel 配置".into()); }
        s.enabled = true;
        // Seed open buffers so enabling alone does not build or save existing work.
        snapshot(p, &root, c, &mut s, true)?;
    }
    s.manual_override = Some(s.enabled);
    persist(&root, &s)?;
    status(&s, "自动编译状态已更新");
    Ok(Some(if s.enabled { "自动编译已开启：停止修改5秒后保存全部并编译所选模块。" }
        else { "自动编译已暂停；已启动的编译会正常结束。" }.into()))
}

pub(super) fn pause(root: &str) -> Result<(), Error> {
    let mut s = read(root)?;
    s.pause();
    persist(root, &s)?;
    status(&s, "已暂停自动编译");
    Ok(())
}

fn start(p: &Project, root: &str, c: &Config, s: &mut State) -> Result<(), Error> {
    // Failure to save must prevent Maven from compiling stale disk contents.
    host::execute_command("file.save", None)?;
    generate(p, c)?;
    let id = host::start_run("compile", &[], false)?;
    s.started(id);
    persist(root, s)?;
    status(s, "正在编译；期间修改会在本次完成后补编译一次");
    Ok(())
}

pub(super) fn manual(p: &Project, c: &Config) -> Result<Option<String>, Error> {
    let root = p.root()?;
    let mut s = read(&root)?;
    if s.running.is_some() {
        // Merge any number of manual clicks and source edits into one follow-up.
        s.dirty_at = Some(now().saturating_sub(IDLE_MS));
        persist(&root, &s)?;
        return Ok(Some("编译正在运行，已合并为一次后续编译。".into()));
    }
    start(p, &root, c, &mut s)?;
    Ok(None)
}

/// Keep the originating window context for Save All and run.start. OpenIDEA's
/// scheduled events have no window and their minimum interval is 60 seconds.
/// Bounded observation of live buffers gives a 5-second debounce without a daemon;
/// queued edit events continue the wait when typing lasts longer than this batch.
fn wait_and_start(p: &Project, root: &str, c: &Config, s: &mut State) -> Result<(), Error> {
    if !s.enabled || s.running.is_some() || s.dirty_at.is_none() { return Ok(()); }
    let task = host::begin_task("JRebel：等待停止修改5秒", true);
    let budget = Instant::now();
    loop {
        if task.is_cancelled() { s.pause(); s.manual_override = Some(false); persist(root, s)?; status(s, "等待已取消，自动编译已暂停"); return Ok(()); }
        snapshot(p, root, c, s, false)?;
        if s.due(now()) { return start(p, root, c, s); }
        // Yield to queued commands/edits, bounding toggle latency during continuous typing.
        if budget.elapsed() >= Duration::from_millis(5_500) { persist(root, s)?; return Ok(()); }
        std::thread::sleep(Duration::from_millis(100));
    }
}

pub(super) fn events(events: &[Event], ctx: &Invocation) {
    let Some(p) = ctx.project() else { return; };
    let result = (|| {
        let root = p.root()?;
        let Some(c) = load(&root)? else { return Ok(()); };
        let mut s = read(&root)?;
        if !c.enabled { s.pause(); }
        for event in events {
            if let Some(done) = event.run_exited() {
                if s.finished(&done.id) {
                    let ok = done.status == Some(0) && !done.killed;
                    if ok { s.last_compile_at = Some(now()); }
                    let elapsed = s.compile_started_at.take().map(|t| now().saturating_sub(t) / 1000).unwrap_or(0);
                    if ok {
                        host::notify(NotificationLevel::Info, "✅ 热编译成功", &format!("完成时间：{} · 耗时：{} 秒", compile_time_text(s.last_compile_at), elapsed));
                    } else {
                        host::notify(NotificationLevel::Error, "JRebel 编译失败", "编译失败或已停止，请查看 Run 中的错误。修复后可再次编译。");
                    }
                    // Manual queued builds also work while automatic compilation is paused.
                    if c.enabled && !s.enabled && s.dirty_at.take().is_some() { start(&p, &root, &c, &mut s)?; }
                }
                continue;
            }
            if !s.enabled { continue; }
            let paths: Vec<&str> = match event {
                Event::DocumentChanged(d) => vec![&d.path],
                Event::FileSaved(path) => vec![path],
                Event::FilesChanged(f) => f.created.iter().chain(&f.modified).chain(&f.removed).map(String::as_str).collect(),
                _ => vec![],
            };
            for path in paths {
                if relevant(&root, path, &c) { s.changed(path_key(path), fingerprint(&p, path)?, now()); }
            }
        }
        persist(&root, &s)?;
        status(&s, "只监控所选模块的源码与资源，忽略编译输出与 rebel.xml");
        if let Err(e) = wait_and_start(&p, &root, &c, &mut s) {
            // A failed launch/save is one failure, never an automatic retry loop.
            s.dirty_at = None;
            persist(&root, &s)?;
            status(&s, &format!("上次启动编译失败：{e}"));
            return Err(e);
        }
        Ok::<_, Error>(())
    })();
    if let Err(e) = result { host::notify(NotificationLevel::Error, "JRebel 自动编译失败", &e.to_string()); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn debounce_resets_on_changes_but_not_save_echoes() {
        let mut s = State { enabled: true, ..Default::default() };
        s.changed("a".into(), Some(1), 0);
        assert!(!s.due(4_999));
        s.changed("a".into(), Some(2), 4_000);
        assert!(!s.due(8_999));
        s.changed("a".into(), Some(2), 8_500);
        assert!(s.due(9_000));
    }

    #[test]
    fn edits_during_build_merge_into_one_follow_up() {
        let mut s = State { enabled: true, ..Default::default() };
        s.started("first".into());
        s.changed("a".into(), Some(1), 100);
        s.changed("b".into(), Some(2), 200);
        assert!(!s.due(10_000));
        assert!(!s.finished("app"));
        assert!(s.finished("first"));
        assert!(s.due(10_000));
        s.started("second".into());
        assert!(s.finished("second"));
        assert!(!s.due(20_000));
    }

    #[test]
    fn pause_clears_pending_work_but_does_not_kill_running_build() {
        let mut s = State { enabled: true, dirty_at: Some(0), running: Some("build".into()), ..Default::default() };
        s.pause();
        assert_eq!(s.running.as_deref(), Some("build"));
        assert!(s.finished("build"));
        assert!(!s.due(10_000));
    }

    #[test]
    fn state_survives_reclamation_and_projects_are_isolated() {
        let _mock = MockHost::install();
        let s = State { enabled: true, dirty_at: Some(10), running: Some("run-1".into()), ..Default::default() };
        persist("D:/project", &s).unwrap();
        assert_eq!(read("D:\\project").unwrap().running.as_deref(), Some("run-1"));
        assert!(read("D:/other").unwrap().enabled);
    }

    #[test]
    fn selected_modules_exclude_siblings_and_generated_files() {
        let c = crate::tests::config();
        assert!(relevant("/project", "/project/app/src/main/java/Test.java", &c));
        assert!(relevant("/project", "/project/app/src/main/resources/mapper/X.xml", &c));
        for p in ["/project/other/src/main/java/Test.java", "/project/app2/src/main/java/Test.java", "/project/app/target/classes/Test.class", "/project/app/src/main/resources/rebel.xml", "/project/app/src/main/resources/rebel.xml.jrebel-backup-1"] {
            assert!(!relevant("/project", p, &c), "{p}");
        }
    }

    #[test]
    fn failed_save_prevents_launch_and_manual_clicks_do_not_overlap() {
        let mock = MockHost::install();
        let p = mock.invocation().project().unwrap();
        let c = crate::tests::config();
        mock.on_command("file.save", |_| Err("save rejected".into()));
        assert!(manual(&p, &c).is_err());
        assert!(mock.configuration_runs().is_empty());
        assert!(read("/project").unwrap().running.is_none());
        persist("/project", &State { running: Some("existing".into()), ..Default::default() }).unwrap();
        assert!(manual(&p, &c).unwrap().unwrap().contains("后续"));
        assert!(mock.configuration_runs().is_empty());
    }

    #[test]
    fn failed_build_reports_error_and_runs_only_one_queued_build() {
        let mock = MockHost::install().with_capabilities(&["project.read", "project.write", "storage", "editor.read", "ide.commands", "process:mvn"]);
        let c = crate::tests::config();
        save("/project", &c).unwrap();
        mock.set_run_configurations(vec![PluginRunConfiguration { id: "compile".into(), program: "mvn".into(), ..Default::default() }]);
        let saves = std::rc::Rc::new(std::cell::Cell::new(0));
        let saved = saves.clone();
        mock.on_command("file.save", move |_| { saved.set(saved.get() + 1); Ok(None) });
        persist("/project", &State { enabled: true, running: Some("old".into()), dirty_at: Some(now().saturating_sub(5_001)), ..Default::default() }).unwrap();
        let event = Event::Other { kind: "run-exited".into(), data: r#"{"id":"old","config":"compile","status":1,"killed":false}"#.into() };
        events(&[event.clone()], &mock.invocation());
        assert_eq!(mock.configuration_runs().len(), 1);
        assert_eq!(saves.get(), 1);
        assert!(mock.notifications().iter().any(|n| n.level == NotificationLevel::Error && n.body.contains("编译失败")));
        events(&[event], &mock.invocation());
        assert_eq!(mock.configuration_runs().len(), 1, "duplicate completion must not start another build");
        assert!(read("/project").unwrap().dirty_at.is_none());
    }

    #[test]
    fn enabled_state_seeds_existing_buffers_without_saving_or_building() {
        let mock = MockHost::install();
        persist("/project", &State { enabled: false, manual_override: Some(false), ..Default::default() }).unwrap();
        mock.open_editor("/project/app/src/main/java/Test.java", "java", "unsaved existing work", &[]);
        let p = mock.invocation().project().unwrap();
        toggle(&p, &crate::tests::config()).unwrap();
        let s = read("/project").unwrap();
        assert!(s.enabled);
        assert!(s.dirty_at.is_none());
        assert!(mock.configuration_runs().is_empty());
    }

    #[test]
    fn native_idle_wait_saves_then_starts_one_build_after_five_seconds() {
        let mock = MockHost::install().with_capabilities(&["project.read", "project.write", "storage", "editor.read", "ide.commands", "process:mvn"]);
        let p = mock.invocation().project().unwrap();
        mock.set_run_configurations(vec![PluginRunConfiguration { id: "compile".into(), program: "mvn".into(), ..Default::default() }]);
        let saved = std::rc::Rc::new(std::cell::Cell::new(false));
        let flag = saved.clone();
        mock.on_command("file.save", move |_| { flag.set(true); Ok(None) });
        let mut s = State { enabled: true, dirty_at: Some(now()), ..Default::default() };
        let started = Instant::now();
        wait_and_start(&p, "/project", &crate::tests::config(), &mut s).unwrap();
        assert!(started.elapsed() >= Duration::from_millis(4_990));
        assert!(saved.get());
        assert_eq!(mock.configuration_runs().len(), 1);
        assert!(s.running.is_some());
        assert!(s.dirty_at.is_none());
    }
}
