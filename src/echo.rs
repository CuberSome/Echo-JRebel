//! Echo Agent integration. No JRebel library or rebel.xml required.
use super::*;

pub(super) const CONFIG_FILE: &str = ".echo-agent/agent.properties";
const STATUS_FILE: &str = ".echo-agent/status.json";

fn file_uri(path: &str) -> String {
    let path = path.replace('\\', "/");
    let mut encoded = String::from(if path.starts_with('/') { "file://" } else { "file:///" });
    for b in path.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) { encoded.push(b as char); }
        else { encoded.push_str(&format!("%{b:02X}")); }
    }
    encoded
}

pub(super) fn agent_arg(root: &str) -> Result<String, Error> {
    let jar = host::setting_string("echoAgentJar").unwrap_or_default().trim().replace('\\', "/");
    let absolute = jar.starts_with('/') || (jar.as_bytes().get(1) == Some(&b':') && jar.as_bytes().get(2) == Some(&b'/'));
    if !absolute || !jar.ends_with(".jar") || jar.contains(['"', '=']) || jar.chars().any(char::is_control) {
        return Err("请在 Settings > Plugins > Echo JRebel 设置 Echo Agent JAR 的绝对路径，例如 C:/Echo/echo-agent.jar".into());
    }
    Ok(format!("-javaagent:{jar}={}", file_uri(&join(root, CONFIG_FILE))))
}

pub(super) fn generate(p: &Project, c: &Config) -> Result<(), Error> {
    let root = p.root()?.replace('\\', "/");
    let mut roots = std::collections::BTreeSet::new();
    for module in c.modules.iter().filter(|m| m.enabled) {
        for output in &module.classes { roots.insert(join(&join(&root, &relative(&module.path)?), &relative(output)?)); }
    }
    let mut content = String::from("# Echo Agent: UTF-8 class output directories; restart service after changing mappings.\n");
    for (i, dir) in roots.iter().enumerate() {
        // Properties.load treats backslash as escape, hence normalized forward slashes.
        content.push_str(&format!("root.{i}={dir}\n"));
    }
    write_backed_up(p, CONFIG_FILE, &content)
}

pub(super) fn status(p: &Project) -> Result<String, Error> {
    if !p.exists(STATUS_FILE)? { return Ok("尚无 Echo Agent 重载记录，请使用 Echo: Run Application 启动服务。".into()); }
    let value: serde_json::Value = serde_json::from_str(&p.read_file(STATUS_FILE)?).map_err(|e| format!("重载记录读取失败：{e}"))?;
    let s = |key: &str| value.get(key).and_then(|v| v.as_str()).unwrap_or("未知").to_owned();
    let state = match s("state").as_str() {
        "ready" => "已启动，等待类变更", "reloaded" => "类重载成功", "restart_required" => "需要重启服务",
        "reload_failed" => "类重载失败", "stopped" => "服务已停止", "not_loaded" => "类尚未加载", _ => "Agent 错误",
    };
    Ok(format!("{state}\n记录的 JVM PID：{}\n类：{}\n成功重载次数：{}\n上次重载：{}\n记录更新：{}\n{}\n尚未应用的变更：{}\n{}\n这是 Agent 写入的记录，不代表 PID 目前仍在运行。", s("pid"), s("class"), value["reload_count"],
        auto::compile_time_text(value["last_reload_at"].as_u64().filter(|v| *v > 0)),
        auto::compile_time_text(value["updated_at"].as_u64()), s("message"), value["pending_failures"], value["failures"]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use openidea_plugin::mock::MockHost;

    #[test]
    fn echo_launch_needs_no_jrebel_and_does_not_write_rebel_xml() {
        let mock = MockHost::install().with_capabilities(&["project.read", "project.write", "storage"]);
        mock.set_setting("echoAgentJar", "\"C:/Echo Agent/echo-agent.jar\"");
        let p = mock.invocation().project().unwrap();
        let mut c = crate::tests::config(); c.engine = "echo".into();
        generate(&p, &c).unwrap();
        assert!(p.read_file(CONFIG_FILE).unwrap().contains("root.0=/project/app/target/classes"));
        assert!(!p.exists("app/src/main/resources/rebel.xml").unwrap());
        let arg = global_agent("C:/项目 space", &c).unwrap();
        assert!(arg.contains("file:///C:/%E9%A1%B9%E7%9B%AE%20space/"));
        let configs = configs("C:/项目 space", &c, &arg).unwrap();
        assert!(configs[0].args.iter().any(|a| a.contains("-javaagent:")));
        assert!(!configs[0].args.iter().any(|a| a.contains("-agentpath:")));
    }

    #[test]
    fn old_config_keeps_jrebel_engine() {
        let mut value = serde_json::to_value(crate::tests::config()).unwrap();
        value.as_object_mut().unwrap().remove("engine");
        let old: Config = serde_json::from_value(value).unwrap();
        assert_eq!(old.engine, "jrebel");
    }
}
