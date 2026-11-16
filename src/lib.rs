use openidea_plugin::{host, register_plugin, Error, Invocation, PickItem, Plugin, PluginRunConfiguration, Project, RunConfigurations};
use openidea_plugin::views::{Body, View, ViewEvent, Views};
use serde::{Deserialize, Serialize};
mod auto;
mod echo;

const CONFIG: &str = "project:";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Module {
    path: String,
    #[serde(default = "module_enabled_default")]
    enabled: bool,
    classes: Vec<String>,
    resources: Vec<String>,
    rebel_file: String,
}

fn module_enabled_default() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default = "legacy_engine")]
    engine: String,
    enabled: bool,
    // "maven-boot" adds the Agent to Spring Boot's forked JVM; "java" uses direct argv.
    launch: String,
    pom: String,
    java_args: Vec<String>,
    jvm_args: Vec<String>,
    app_args: Vec<String>,
    build_tool: String,
    compile_args: Vec<String>,
    modules: Vec<Module>,
}

fn legacy_engine() -> String { "jrebel".into() }

fn new_engine() -> String { host::setting_string("engine").unwrap_or_else(|| "echo".into()) }

fn relative(path: &str) -> Result<String, Error> {
    let path = path.replace('\\', "/");
    if path.starts_with('/') || path.contains(':') || path.chars().any(char::is_control)
        || path.split('/').any(|s| s == "..") {
        return Err("路径必须位于项目内，不能包含 .. 或盘符".into());
    }
    Ok(path.split('/').filter(|s| !s.is_empty() && *s != ".").collect::<Vec<_>>().join("/"))
}

fn join(a: &str, b: &str) -> String {
    format!("{}/{}", a.trim_end_matches('/'), b.trim_start_matches('/')).trim_end_matches('/').to_owned()
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
        .replace('"', "&quot;").replace('\'', "&apos;")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn agent_arg(path: &str) -> Result<String, Error> {
    let p = path.trim().replace('\\', "/");
    let b = p.as_bytes();
    let absolute = p.starts_with('/') || (b.len() > 2 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'/');
    if !absolute || p.contains('"') || p.chars().any(char::is_control)
        || !(p.ends_with(".dll") || p.ends_with(".so") || p.ends_with(".dylib")) {
        return Err("请在 Settings > Plugins > JRebel Integration 设置 native Agent 的绝对路径（.dll / .so / .dylib）".into());
    }
    Ok(format!("-agentpath:{p}"))
}

fn validate(c: &Config) -> Result<(), Error> {
    if !matches!(c.engine.as_str(), "echo" | "jrebel") { return Err("engine 必须为 echo 或 jrebel".into()); }
    if !matches!(c.launch.as_str(), "maven-boot" | "java") || !matches!(c.build_tool.as_str(), "mvn" | "gradle") {
        return Err("launch 必须为 maven-boot 或 java；build_tool 必须为 mvn 或 gradle".into());
    }
    if c.modules.is_empty() || !c.modules.iter().any(|m| m.enabled) || c.compile_args.is_empty() || (c.launch == "java" && c.java_args.is_empty()) {
        return Err("至少启用一个模块，并填写编译参数和 Java 启动参数".into());
    }
    if relative(&c.pom)?.is_empty() { return Err("pom 路径不能为空".into()); }
    for m in &c.modules {
        relative(&m.path)?;
        if m.classes.is_empty() || relative(&m.rebel_file)?.is_empty() || !m.rebel_file.ends_with("rebel.xml") {
            return Err("模块需要 classes 和以 rebel.xml 结尾的 rebel_file".into());
        }
        for p in m.classes.iter().chain(&m.resources) {
            if relative(p)?.is_empty() { return Err("classes/resources 路径不能为空".into()); }
        }
    }
    for a in c.java_args.iter().chain(&c.jvm_args).chain(&c.app_args).chain(&c.compile_args) {
        if a.contains('\0') { return Err("参数不能包含 NUL".into()); }
    }
    if c.jvm_args.iter().chain(&c.java_args).any(|a| a.starts_with("-agentpath:") || a.starts_with("-javaagent:")) {
        return Err("Agent 参数由插件添加，请勿重复填写".into());
    }
    Ok(())
}

fn rebel(root: &str, m: &Module) -> Result<String, Error> {
    let base = join(&root.replace('\\', "/"), &relative(&m.path)?);
    let mut s = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<application xmlns=\"http://www.zeroturnaround.com\">\n  <classpath>\n");
    if m.enabled {
        for p in m.classes.iter().chain(&m.resources) {
            s.push_str(&format!("    <dir name=\"{}\"/>\n", xml(&join(&base, &relative(p)?))));
        }
    }
    s.push_str("  </classpath>\n</application>\n");
    Ok(s)
}

fn write_backed_up(p: &Project, path: &str, text: &str) -> Result<(), Error> {
    if p.exists(path)? {
        let old = p.read_file(path)?;
        if old == text { return Ok(()); }
        let mut n = 1;
        loop {
            let backup = format!("{path}.jrebel-backup-{n}");
            if !p.exists(&backup)? { p.write_file(&backup, &old)?; break; }
            n += 1;
            if n > 1000 { return Err("备份数量超过 1000，请先整理".into()); }
        }
    }
    p.write_file(path, text)
}

fn generate(p: &Project, c: &Config) -> Result<(), Error> {
    validate(c)?;
    if c.engine == "echo" { return echo::generate(p, c); }
    let root = p.root()?;
    // Prepare every mapping before changing files. Host also enforces symlink containment.
    let files: Vec<_> = c.modules.iter().map(|m| Ok((join(&relative(&m.path)?, &relative(&m.rebel_file)?).trim_start_matches('/').to_owned(), rebel(&root, m)?)))
        .collect::<Result<_, Error>>()?;
    for (path, text) in files { write_backed_up(p, &path, &text)?; }
    Ok(())
}

fn load(root: &str) -> Result<Option<Config>, Error> {
    host::storage::get(&project_key(root))?.map(|s| serde_json::from_str(&s).map_err(|e| Error::Failed(format!("项目配置 JSON 无效: {e}")))).transpose()
}

fn project_key(root: &str) -> String {
    format!("{CONFIG}{}", root.replace('\\', "/").trim_start_matches("//?/").trim_end_matches('/'))
}

fn save(root: &str, c: &Config) -> Result<(), Error> {
    host::storage::set(&project_key(root), &serde_json::to_string(c).map_err(|e| e.to_string())?)
}

fn global_agent(root: &str, c: &Config) -> Result<String, Error> {
    if c.engine == "echo" { echo::agent_arg(root) }
    else { agent_arg(&host::setting_string("agentLibrary").unwrap_or_default()) }
}

// Spring Boot's JVM argument parser receives this string after process argv parsing.
fn jvm_words(args: &[String]) -> Result<String, Error> {
    if args.iter().any(|a| a.contains('"') || a.chars().any(char::is_control)) {
        return Err("Maven JVM 参数暂不支持内嵌双引号或控制字符，请使用直接 Java 启动".into());
    }
    Ok(args.iter().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(" "))
}

fn configs(root: &str, c: &Config, agent: &str) -> Result<Vec<PluginRunConfiguration>, Error> {
    validate(c)?;
    if !c.enabled { return Ok(vec![]); }
    let (program, args) = if c.launch == "maven-boot" {
        let mut jvm = vec![agent.to_owned()];
        jvm.extend(c.jvm_args.clone());
        let mut args = vec!["-f".into(), relative(&c.pom)?, "spring-boot:run".into(), format!("-Dspring-boot.run.jvmArguments={}", jvm_words(&jvm)?)];
        // application arguments are configured separately from JVM arguments.
        if !c.app_args.is_empty() { args.push(format!("-Dspring-boot.run.arguments={}", jvm_words(&c.app_args)?)); }
        ("mvn", args)
    } else {
        let mut args = vec![agent.to_owned()];
        args.extend(c.jvm_args.clone());
        args.extend(c.java_args.clone());
        args.extend(c.app_args.clone());
        ("java", args)
    };
    let entry = |id: &str, name: &str, program: &str, args: Vec<String>| PluginRunConfiguration {
        id: id.into(), name: name.into(), program: program.into(), args, cwd: Some(root.into()), ..Default::default()
    };
    Ok(vec![
        entry("app", if c.engine == "echo" { "Echo: Run Application" } else { "JRebel: Run Application" }, program, args),
        entry("compile", "Echo: Compile Updates", &c.build_tool, c.compile_args.clone()),
    ])
}

fn setup_manual(p: &Project) -> Result<Config, Error> {
    let root = p.root()?.replace('\\', "/");
    let mut candidates = vec![String::new()];
    for glob in ["**/pom.xml", "**/build.gradle", "**/build.gradle.kts"] {
        for path in p.find_files(glob, 1000)? {
            let path = path.replace('\\', "/");
            if let Some(rel) = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))) {
                if let Some((dir, _)) = rel.rsplit_once('/') { candidates.push(relative(dir)?); }
            }
        }
    }
    candidates.sort(); candidates.dedup();
    let items: Vec<_> = candidates.iter().map(|m| PickItem::new(if m.is_empty() { "." } else { m }, if m.is_empty() { ". (项目根目录)" } else { m })).collect();
    let selected = host::pick_many("选择热更新模块（至少一个）", &items)?.ok_or(Error::Cancelled)?;
    if selected.is_empty() { return Err("至少选择一个模块".into()); }
    let maven = p.exists("pom.xml")?;
    let mode = host::pick("选择启动方式", &[
        PickItem::new("maven-boot", "Maven Spring Boot（Boot 2.x/3.x）"),
        PickItem::new("java", "直接 Java（需填写主类/类路径或 -jar）"),
    ])?.ok_or(Error::Cancelled)?;
    let mut c = Config {
        engine: new_engine(),
        enabled: true, launch: mode, pom: "pom.xml".into(), java_args: vec!["-cp".into(), "target/classes".into(), "com.example.Application".into()],
        jvm_args: vec![], app_args: vec![], build_tool: if maven { "mvn" } else { "gradle" }.into(),
        compile_args: if maven { vec!["-DskipTests".into(), "compile".into()] } else { vec!["classes".into()] },
        modules: selected.iter().map(|m| Module {
            path: if m == "." { String::new() } else { m.clone() },
            enabled: true,
            classes: if maven { vec!["target/classes".into()] } else { vec!["build/classes/java/main".into(), "build/resources/main".into()] },
            resources: vec!["src/main/resources".into()], rebel_file: "src/main/resources/rebel.xml".into(),
        }).collect(),
    };
    let edited = host::input_multiline("检查项目配置：多模块请设置应用 pom；直接 Java 请替换 java_args 示例。每个数组元素是一个参数。", Some(&serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?))?.ok_or(Error::Cancelled)?;
    c = serde_json::from_str(&edited).map_err(|e| format!("配置 JSON 无效: {e}"))?;
    validate(&c)?;
    Ok(c)
}

fn setup(p: &Project) -> Result<Config, Error> {
    let root = p.root()?.replace('\\', "/");
    if !p.exists("pom.xml")? { return setup_manual(p); }
    let mut poms = vec![String::new()];
    for path in p.find_files("**/pom.xml", 1000)? {
        let path = path.replace('\\', "/");
        if let Some(rel) = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))) {
            if let Some((dir, _)) = rel.rsplit_once('/') { poms.push(relative(dir)?); }
        }
    }
    poms.sort(); poms.dedup();
    let mut launch = Vec::new();
    let mut source_modules = std::collections::BTreeSet::new();
    for path in p.find_files("**/*.java", 5000)? {
        let path = path.replace('\\', "/");
        let Some(rel) = path.strip_prefix(&format!("{}/", root.trim_end_matches('/'))) else { continue; };
        if !rel.contains("/src/main/java/") { continue; }
        if let Some(module) = poms.iter().filter(|m| {
            let prefix = if m.is_empty() { "src/main/java/".to_owned() } else { format!("{m}/src/main/java/") };
            rel.starts_with(&prefix)
        }).max_by_key(|m| m.len()) {
            source_modules.insert(module.clone());
            if let Ok(text) = p.read_file(&path) {
                if text.contains("@SpringBootApplication") || text.contains("@SpringBootConfiguration") { launch.push(module.clone()); }
            }
        }
    }
    launch.sort(); launch.dedup();
    if launch.is_empty() || source_modules.is_empty() { return setup_manual(p); }
    let app = if launch.len() == 1 {
        launch[0].clone()
    } else {
        let choices: Vec<_> = launch.iter().map(|module| PickItem::new(module, module)).collect();
        host::pick("发现多个 Spring Boot 启动模块，请选择要运行的服务", &choices)?.ok_or(Error::Cancelled)?
    };
    let modules = source_modules.into_iter().map(|path| Module {
        rebel_file: "src/main/resources/rebel.xml".into(),
        path: path.clone(), enabled: true, classes: vec!["target/classes".into()], resources: vec!["src/main/resources".into()],
    }).collect::<Vec<_>>();
    Ok(Config {
        engine: new_engine(),
        enabled: true, launch: "maven-boot".into(),
        pom: if app.is_empty() { "pom.xml".into() } else { format!("{app}/pom.xml") },
        java_args: vec![], jvm_args: vec![], app_args: vec![], build_tool: "mvn".into(),
        compile_args: if app.is_empty() { vec!["-DskipTests".into(), "compile".into()] } else { vec!["-pl".into(), app, "-am".into(), "-DskipTests".into(), "compile".into()] },
        modules,
    })
}

#[derive(Default)]
struct Integration;

impl Plugin for Integration {
    fn activate(&mut self, _: &openidea_plugin::Context) -> Result<(), Error> {
        auto::subscribe()
    }

    fn handle_events(&mut self, events: &[openidea_plugin::Event], ctx: &Invocation) {
        auto::events(events, ctx);
    }

    fn run_command(&mut self, id: &str, ctx: &Invocation, _: Option<&str>) -> Result<Option<String>, Error> {
        let p = ctx.project().ok_or("请先打开项目；Agent 路径在 Settings > Plugins > Echo JRebel 设置")?;
        let root = p.root()?;
        match id {
            "auto-toggle" => {
                let c = load(&root)?.ok_or("请先初始化热更新配置")?;
                auto::toggle(&p, &c)
            }
            "setup" | "configure" => {
                let c = if id == "configure" {
                    let c = load(&root)?.ok_or("请先选择热部署模块以初始化项目")?;
                    let text = host::input_multiline("编辑 JRebel 项目配置", Some(&serde_json::to_string_pretty(&c).map_err(|e| e.to_string())?))?.ok_or(Error::Cancelled)?;
                    serde_json::from_str(&text).map_err(|e| format!("配置 JSON 无效: {e}"))?
                } else { setup(&p)? };
                validate(&c)?;
                // Preflight launch arguments before writing mappings or enabling the project.
                configs(&root, &c, &global_agent(&root, &c)?)?;
                generate(&p, &c)?;
                save(&root, &c)?;
                auto::refresh_status(&root)?;
                Ok(Some("配置已保存，所选引擎的映射已生成；修改映射后请重启服务。先编译，再选择对应 Run Application 启动。".into()))
            }
            "disable" => {
                auto::pause(&root)?;
                if let Some(mut c) = load(&root)? { c.enabled = false; save(&root, &c)?; }
                Ok(Some("已禁用此项目的 JRebel 启动入口。正在运行的服务需手动停止；rebel.xml 和备份文件保留。".into()))
            }
            "reset" => {
                if !host::confirm("重置项目配置", "将清除当前项目保存的 Echo JRebel 配置和自动编译状态；不会删除项目文件、rebel.xml 或备份。确定继续吗？")? {
                    return Err(Error::Cancelled);
                }
                auto::reset(&root)?;
                host::storage::delete(&project_key(&root))?;
                Ok(Some("项目配置已重置。再次运行「运行服务（JRebel）」即可重新初始化。".into()))
            }
            "run" | "compile" => {
                let Some(c) = load(&root)? else {
                    if id == "run" {
                        let c = setup(&p)?;
                        validate(&c)?;
                        configs(&root, &c, &global_agent(&root, &c)?)?;
                        generate(&p, &c)?;
                        save(&root, &c)?;
                        auto::refresh_status(&root)?;
                        return Ok(Some("项目配置已保存。运行配置刷新后，请在右上角选择对应引擎的 Run Application 并启动服务。".into()));
                    }
                    return Err("请先选择热部署模块以初始化项目".into());
                };
                if !c.enabled { return Err("此项目尚未启用热更新".into()); }
                if id == "compile" { return auto::manual(&p, &c); }
                configs(&root, &c, &global_agent(&root, &c)?)?;
                generate(&p, &c)?;
                host::start_run(if id == "run" { "app" } else { "compile" }, &[], false)?;
                Ok(None)
            }
            "reload-status" => {
                let c = load(&root)?.ok_or("请先初始化项目")?;
                if c.engine != "echo" { return Err("当前使用 JRebel，请在其应用日志中查看重载结果".into()); }
                Ok(Some(echo::status(&p)?))
            }
            "engine-select" => {
                let mut c = load(&root)?.ok_or("请先初始化项目")?;
                c.engine = host::pick("选择热更新引擎（切换后需手动重启服务）", &[
                    PickItem::new("echo", "Echo Agent（自研：已有方法逻辑热更新）"),
                    PickItem::new("jrebel", "JRebel（需要自备 Agent 和授权）"),
                ])?.ok_or(Error::Cancelled)?;
                global_agent(&root, &c)?;
                generate(&p, &c)?;
                save(&root, &c)?;
                host::refresh_view("modules");
                Ok(Some("引擎已切换，请停止旧服务并选择新的 Run Application 启动。".into()))
            }
            "check" => {
                let c = load(&root)?.ok_or("请先初始化项目并设置 Agent 路径")?;
                global_agent(&root, &c)?;
                generate(&p, &c)?;
                host::start_run("agent-check", &[], false)?;
                Ok(None)
            }
            _ => Err(Error::NotFound(id.into())),
        }
    }
}

impl RunConfigurations for Integration {
    fn configurations(&mut self, root: &str) -> Vec<PluginRunConfiguration> {
        let result = (|| {
            let mut list = Vec::new();
            if let Some(c) = load(root)? {
                let agent = global_agent(root, &c)?;
                list.push(PluginRunConfiguration {
                    id: "agent-check".into(), name: "Echo: Check Agent".into(), program: "java".into(),
                    args: vec![agent.clone(), "-version".into()], cwd: Some(root.into()), ..Default::default()
                });
                list.extend(configs(root, &c, &agent)?);
            } else {
                list.push(PluginRunConfiguration {
                    id: "compile".into(), name: "Echo: Compile Updates (configure project first)".into(),
                    program: "mvn".into(), args: vec!["-DskipTests".into(), "compile".into()],
                    cwd: Some(root.into()), ..Default::default()
                });
            }
            Ok::<_, Error>(list)
        })();
        match result {
            Ok(list) => list,
            Err(e) => { host::log(openidea_plugin::LogLevel::Warn, &format!("JRebel: {e}")); vec![] }
        }
    }
}

impl Views for Integration {
    fn render(&mut self, view: &str, _viewport: Option<openidea_plugin::views::Viewport>, ctx: &Invocation) -> Result<View, Error> {
        if view != "modules" { return Ok(View::message("未知的 Echo JRebel 视图")); }
        let Some(project) = ctx.project() else { return Ok(View::message("打开项目以管理热更新模块")); };
        let root = project.root()?;
        let config = load(&root)?;
        let status = html_escape(&match config.as_ref() {
            Some(c) if !c.enabled => "当前项目的热更新已禁用".to_owned(),
            Some(c) => format!("引擎：{} · {}", c.engine, auto::view_status(&root)?),
            None => "先初始化项目以启用热部署".to_owned(),
        });
        let modules = config.as_ref().map(|c| c.modules.iter().map(|m| {
            let name = if m.path.is_empty() { "项目根目录" } else { &m.path };
            let action = if m.enabled { "停用" } else { "启用" };
            let state = if m.enabled { "已启用" } else { "已停用" };
            format!("<li class=\"{}\"><span class=\"module-dot\"></span><span class=\"module-info\"><span class=\"module-name\">{}</span><span class=\"module-path\">{}</span></span><span class=\"module-state\">{}</span><button type=\"button\" class=\"module-toggle\" data-module=\"{}\" aria-pressed=\"{}\" aria-label=\"{} {}\">{}</button></li>", if m.enabled { "enabled" } else { "disabled" }, html_escape(name), html_escape(&m.classes.join(", ")), state, html_escape(&m.path), m.enabled, action, html_escape(name), action)
        }).collect::<String>()).unwrap_or_default();
        let count = config.as_ref().map(|c| c.modules.len()).unwrap_or(0);
        let active_count = config.as_ref().map(|c| c.modules.iter().filter(|m| m.enabled).count()).unwrap_or(0);
        let auto_enabled = config.as_ref().is_some_and(|c| c.enabled) && auto::view_enabled(&root)?;
        let empty = if config.is_none() { "<div class=\"empty\"><span class=\"empty-mark\">E</span><strong>还没有配置项目</strong><span>自动识别启动模块并生成热部署映射</span><button data-action=\"setup\" class=\"primary\">初始化项目</button></div>" } else if count == 0 { "<div class=\"empty compact\">没有已选热部署模块</div>" } else { "" };
        let initialized = config.is_some();
        let actions = if initialized {
            format!(r#"<button type="button" data-action="run" class="primary run"><svg aria-hidden="true" viewBox="0 0 20 20"><path d="M6 3.8c0-.8.9-1.3 1.6-.9l9 5.2a2.2 2.2 0 0 1 0 3.8l-9 5.2c-.7.4-1.6-.1-1.6-.9z"/></svg><span>运行服务</span><span class="button-hint">热更新</span></button><button type="button" data-action="compile" class="secondary"><svg aria-hidden="true" viewBox="0 0 20 20"><path d="M4 4.5h12M4 8h8M4 11.5h10M4 15h6"/><path d="m13 14 2 2 3-4"/></svg><span>编译更新</span></button><button type="button" data-action="setup" class="secondary"><svg aria-hidden="true" viewBox="0 0 20 20"><path d="M10 3.5v13M3.5 10h13"/></svg><span>配置模块</span></button><button type="button" data-action="auto-toggle" class="auto-toggle {}" aria-pressed="{}"><svg aria-hidden="true" viewBox="0 0 20 20"><path d="M11.5 2.5 4.5 11h4l-.5 6.5 7.5-9h-4z"/></svg><span>自动编译</span><span class="button-state">{}</span></button>"#,
                if auto_enabled { "active" } else { "" }, auto_enabled, if auto_enabled { "已开启" } else { "已暂停" })
        } else { String::new() };
        let actions = if initialized { format!("{actions}<button data-action=\"engine-select\" class=\"secondary\">切换引擎</button><button data-action=\"reload-status\" class=\"secondary\">查看重载记录</button>") } else { actions };
        let reset = if initialized { "<button data-action=\"reset\" class=\"quiet danger\">重置配置</button>" } else { "" };
        let html = format!(r#"<!doctype html><html><head><meta charset="utf-8"><style>
*{{box-sizing:border-box}}body{{margin:0;padding:16px;color:var(--openidea-fg);background:transparent;font:var(--openidea-font-size,13px)/1.45 var(--openidea-font-ui,system-ui,sans-serif)}}
.panel{{max-width:520px;margin:auto}}.head{{display:flex;align-items:center;gap:11px;margin:2px 0 15px}}.mark{{width:34px;height:34px;display:grid;place-items:center;border-radius:10px;background:var(--openidea-accent);color:var(--openidea-fg-on-accent);font-weight:750;font-size:16px}}h1{{font-size:15px;line-height:1.2;margin:0;color:var(--openidea-fg-strong);font-weight:650}}.sub{{font-size:11px;color:var(--openidea-fg-muted);margin-top:3px}}
.status{{display:flex;align-items:center;gap:8px;border:1px solid var(--openidea-border);background:var(--openidea-bg-elevated);border-radius:8px;padding:9px 10px;font-size:12px;color:var(--openidea-fg-muted)}}.live{{width:7px;height:7px;border-radius:50%;background:var(--openidea-success);flex:none}}.section{{margin-top:18px}}.section-title{{display:flex;align-items:baseline;justify-content:space-between;margin:0 1px 8px;color:var(--openidea-fg-strong);font-size:12px;font-weight:650}}.count{{font-size:11px;color:var(--openidea-fg-muted);font-weight:450}}
ul{{list-style:none;padding:0;margin:0;border:1px solid var(--openidea-border);border-radius:8px;overflow:hidden}}li{{display:flex;align-items:center;gap:8px;padding:7px 8px 7px 10px;min-height:42px;background:var(--openidea-bg-elevated)}}li+li{{border-top:1px solid var(--openidea-border)}}li.disabled{{opacity:.72}}.module-dot{{width:7px;height:7px;border-radius:2px;background:var(--openidea-accent);flex:none}}li.disabled .module-dot{{background:var(--openidea-fg-muted)}}.module-info{{min-width:0;display:flex;flex:1;flex-direction:column;gap:2px}}.module-name{{font-size:12px;color:var(--openidea-fg-strong);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}.module-path{{font:10px var(--openidea-font-mono,monospace);color:var(--openidea-fg-muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}}.module-state{{font-size:9px;color:var(--openidea-success);white-space:nowrap}}li.disabled .module-state{{color:var(--openidea-fg-muted)}}button.module-toggle{{padding:5px 8px;min-height:28px;min-width:48px;font-size:10px;color:var(--openidea-fg-muted)}}li.enabled button.module-toggle{{color:var(--openidea-danger)}}li.disabled button.module-toggle{{color:var(--openidea-success)}}
.empty{{display:flex;flex-direction:column;align-items:center;gap:6px;padding:22px 12px;border:1px dashed var(--openidea-border);border-radius:9px;color:var(--openidea-fg-muted);text-align:center}}.empty strong{{color:var(--openidea-fg-strong);font-size:13px}}.empty-mark{{display:grid;place-items:center;width:28px;height:28px;border-radius:8px;background:var(--openidea-bg-hover);color:var(--openidea-accent);font-weight:700;margin-bottom:3px}}.compact{{padding:15px}}
.actions{{display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:14px}}button{{min-height:34px;display:inline-flex;align-items:center;justify-content:center;gap:7px;border:1px solid var(--openidea-border);border-radius:8px;background:var(--openidea-bg-elevated);color:var(--openidea-fg);font:inherit;font-size:11px;padding:7px 10px;cursor:pointer;transition:background .14s,border-color .14s,transform .12s,filter .14s}}button svg{{width:14px;height:14px;flex:none;fill:none;stroke:currentColor;stroke-width:1.7;stroke-linecap:round;stroke-linejoin:round}}button:hover{{background:var(--openidea-bg-hover);border-color:var(--openidea-focus-ring)}}button:active{{transform:translateY(1px)}}button:focus-visible{{outline:2px solid var(--openidea-focus-ring);outline-offset:2px}}button.primary{{grid-column:1/-1;min-height:40px;border-color:transparent;background:var(--openidea-accent);color:var(--openidea-fg-on-accent);font-weight:650;box-shadow:0 1px 3px var(--openidea-border)}}button.primary svg{{fill:currentColor;stroke:none}}button.primary:hover{{filter:brightness(1.06)}}button.secondary{{font-weight:550}}button.secondary:hover{{color:var(--openidea-fg-strong)}}.button-hint{{margin-left:auto;padding:2px 6px;border-radius:999px;background:transparent;border:1px solid currentColor;opacity:.78;font-size:9px;font-weight:500}}button.auto-toggle{{grid-column:1/-1;justify-content:flex-start;min-height:38px;padding:7px 10px}}button.auto-toggle svg{{color:var(--openidea-fg-muted)}}button.auto-toggle .button-state{{margin-left:auto;padding:3px 8px;border:1px solid var(--openidea-border);border-radius:999px;color:var(--openidea-fg-muted);font-size:10px}}button.auto-toggle.active{{border-color:var(--openidea-success);background:var(--openidea-bg-elevated)}}button.auto-toggle.active svg{{color:var(--openidea-success)}}button.auto-toggle.active .button-state{{border-color:var(--openidea-success);color:var(--openidea-success)}}.foot{{display:flex;align-items:center;justify-content:space-between;gap:8px;margin-top:14px;padding-top:10px;border-top:1px solid var(--openidea-border);font-size:10px;color:var(--openidea-fg-muted)}}.quiet{{padding:3px 6px;border-color:transparent;background:transparent}}.danger:hover{{color:var(--openidea-danger)}}
</style></head><body><main class="panel"><header class="head"><div class="mark">E</div><div><h1>Echo JRebel</h1><div class="sub">Java 热部署控制台</div></div></header><div class="status"><span class="live"></span><span>{status}</span></div><section class="section"><h2 class="section-title"><span>热部署模块</span><span class="count">{active_count}/{count} 已启用</span></h2>{list}{empty}</section><div class="actions">{actions}</div><footer class="foot"><span>停止修改 5 秒后自动保存并编译</span>{reset}</footer></main><script>
document.querySelectorAll('[data-action]').forEach(b=>b.addEventListener('click',()=>window.openidea.postMessage(JSON.stringify({{action:b.dataset.action}}))));document.querySelectorAll('[data-module]').forEach(b=>b.addEventListener('click',()=>window.openidea.postMessage(JSON.stringify({{action:'module-toggle',module:b.dataset.module}}))));window.openidea.onThemeChange(()=>{{}});
</script></body></html>"#, list=if count>0 { format!("<ul>{modules}</ul>") } else { String::new() });
        Ok(View::new(Body::Html(html)))
    }

    fn handle(&mut self, view: &str, event: ViewEvent, ctx: &Invocation) -> Result<(), Error> {
        if view == "modules" {
            if let ViewEvent::Message(message) = event {
                let parsed = serde_json::from_str::<serde_json::Value>(&message).ok();
                let action = parsed.as_ref().and_then(|v| v.get("action").and_then(|s| s.as_str()).map(str::to_owned));
                if let Some(id) = action {
                    let result = if id == "module-toggle" {
                        match parsed.as_ref().and_then(|v| v.get("module")).and_then(|v| v.as_str()) {
                            Some(path) => toggle_module(ctx, path),
                            None => Err("模块路径无效".into()),
                        }
                    } else if matches!(id.as_str(), "run" | "compile" | "auto-toggle" | "setup" | "reset" | "engine-select" | "reload-status") {
                        <Self as Plugin>::run_command(self, &id, ctx, None).map(|message| {
                            if let Some(message) = message { host::notify(openidea_plugin::NotificationLevel::Info, "Echo", &message); }
                        })
                    } else {
                        Ok(())
                    };
                    if let Err(error) = result {
                        host::notify(openidea_plugin::NotificationLevel::Error, "Echo JRebel", &error.to_string());
                    }
                }
                host::refresh_view("modules");
            }
        }
        Ok(())
    }
}

register_plugin!(Integration, run_configurations, views);

fn toggle_module(ctx: &Invocation, path: &str) -> Result<(), Error> {
    let project = ctx.project().ok_or("请先打开项目")?;
    let root = project.root()?;
    let mut config = load(&root)?.ok_or("请先初始化项目配置")?;
    let Some(index) = config.modules.iter().position(|m| m.path == path) else {
        return Err("找不到该模块，请重新初始化项目配置".into());
    };
    config.modules[index].enabled = !config.modules[index].enabled;
    if !config.modules.iter().any(|m| m.enabled) {
        config.modules[index].enabled = true;
        return Err("至少保留一个已启用的热部署模块".into());
    }
    validate(&config)?;
    generate(&project, &config)?;
    save(&root, &config)?;
    host::notify(openidea_plugin::NotificationLevel::Info, "Echo JRebel", "模块状态已更新，映射已同步。Echo Agent 需重启服务才能应用模块范围变化。");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn tools_menus_do_not_use_context_only_submenu() {
        let manifest: toml::Value = toml::from_str(include_str!("../plugin.toml")).unwrap();
        let contributes = &manifest["contributes"];
        let commands = contributes["commands"].as_array().unwrap();
        let menus = contributes["menus"].as_array().unwrap();
        assert_eq!(menus.len(), 13);
        for menu in menus {
            assert!(matches!(menu["menu"].as_str(), Some("tools" | "mainToolbar")));
            assert!(menu.get("submenu").is_none(), "OpenIDEA only supports submenu for context menus");
            assert!(commands.iter().any(|c| c["id"] == menu["command"]));
        }
        let schema: serde_json::Value = serde_json::from_str(include_str!("../settings.schema.json")).unwrap();
        assert_eq!(schema["properties"]["agentLibrary"]["type"], "string");
    }

    pub(crate) fn config() -> Config {
        Config { engine: "jrebel".into(), enabled: true, launch: "maven-boot".into(), pom: "app/pom.xml".into(), java_args: vec!["-jar".into(), "app.jar".into()], jvm_args: vec![], app_args: vec![], build_tool: "mvn".into(), compile_args: vec!["compile".into()], modules: vec![Module { path: "app".into(), enabled: true, classes: vec!["target/classes".into()], resources: vec!["src/main/resources".into()], rebel_file: "src/main/resources/rebel.xml".into() }] }
    }

    #[test]
    fn paths_and_xml_are_safe() {
        for p in ["../secret", "a/../../b", "C:/outside", "/outside", "a\\..\\b", "a\n"] { assert!(relative(p).is_err(), "{p}"); }
        assert_eq!(relative("./a\\b").unwrap(), "a/b");
        let text = rebel("C:/Work & Test/\"X\"", &config().modules[0]).unwrap();
        assert!(text.contains("C:/Work &amp; Test/&quot;X&quot;/app/target/classes"));
        assert!(text.contains("http://www.zeroturnaround.com"));
    }

    #[test]
    fn native_agent_and_forked_maven_jvm_have_correct_quoting() {
        assert!(agent_arg("relative/jrebel.jar").is_err());
        assert!(agent_arg("C:/x\"/jrebel64.dll").is_err());
        let agent = agent_arg("C:\\Program Files\\JRebel\\lib\\jrebel64.dll").unwrap();
        let list = configs("C:/My Project", &config(), &agent).unwrap();
        assert_eq!(list[0].program, "mvn");
        assert_eq!(list[0].args[3], "-Dspring-boot.run.jvmArguments=\"-agentpath:C:/Program Files/JRebel/lib/jrebel64.dll\"");
        assert!(!list[1].args.iter().any(|s| s.contains("agentpath")));
        let mut c = config(); c.launch = "java".into();
        assert_eq!(configs("/p", &c, &agent).unwrap()[0].args, vec![agent, "-jar".into(), "app.jar".into()]);
    }

    #[test]
    fn mappings_keep_backups_and_are_idempotent() {
        let mock = MockHost::install().with_capabilities(&["project.read", "project.write", "storage"]);
        let p = mock.invocation().project().unwrap();
        let path = "app/src/main/resources/rebel.xml";
        mock.add_file(path, "original");
        mock.add_file(&format!("{path}.jrebel-backup-1"), "older");
        generate(&p, &config()).unwrap();
        assert_eq!(p.read_file(&format!("{path}.jrebel-backup-1")).unwrap(), "older");
        assert_eq!(p.read_file(&format!("{path}.jrebel-backup-2")).unwrap(), "original");
        generate(&p, &config()).unwrap();
        assert!(!p.exists(&format!("{path}.jrebel-backup-3")).unwrap());
    }

    #[test]
    fn project_settings_are_isolated_and_disable_removes_launch() {
        let mock = MockHost::install();
        mock.set_setting("agentLibrary", "\"C:/JRebel/lib/jrebel64.dll\"");
        save("/one", &config()).unwrap();
        assert!(load("/two").unwrap().is_none());
        assert_eq!(Integration.configurations("/one").len(), 3);
        let mut c = config(); c.enabled = false; save("/one", &c).unwrap();
        assert_eq!(Integration.configurations("/one").len(), 1);
        save("D:/workspace/Back", &config()).unwrap();
        assert!(load("D:\\workspace\\Back").unwrap().is_some());
        assert!(load("\\\\?\\D:\\workspace\\Back").unwrap().is_some());
        assert_eq!(Integration.configurations("D:\\workspace\\Back").len(), 3);
    }

    #[test]
    fn invalid_config_does_not_write_files() {
        let mock = MockHost::install();
        let p = mock.invocation().project().unwrap();
        let mut c = config(); c.modules[0].classes = vec!["../outside".into()];
        assert!(generate(&p, &c).is_err());
        assert!(!p.exists("app/src/main/resources/rebel.xml").unwrap());
        c = config(); c.jvm_args = vec!["-agentpath:duplicate".into()]; assert!(validate(&c).is_err());
        c = config(); c.jvm_args = vec!["-Dx=\"bad\"".into()]; assert!(configs("/p", &c, "-agentpath:/x.so").is_err());
    }

    #[test]
    fn denied_write_keeps_original_and_does_not_enable_project() {
        let mock = MockHost::install().with_capabilities(&["project.read", "storage"]);
        mock.set_setting("agentLibrary", "\"C:/JRebel/lib/jrebel64.dll\"");
        let mut old = config(); old.enabled = false; save("/project", &old).unwrap();
        mock.add_file("app/src/main/resources/rebel.xml", "user mapping");
        let edited = serde_json::to_string(&config()).unwrap();
        mock.answer_next_prompt(Some(&[&edited]));
        assert!(matches!(Integration.run_command("configure", &mock.invocation(), None), Err(Error::Denied(_))));
        assert!(!load("/project").unwrap().unwrap().enabled);
        assert_eq!(mock.invocation().project().unwrap().read_file("app/src/main/resources/rebel.xml").unwrap(), "user mapping");
    }
}
