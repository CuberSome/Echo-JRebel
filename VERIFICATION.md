# 验证记录

日期：2026-11-14。

0.2.0：16 项测试通过（包含一次真实5秒等待的原生宿主模拟测试），覆盖5秒边界与连续修改重新计时、保存回声去重、编译中多个改动合并、过期完成事件忽略、暂停清除排队但不中断当前编译、保存失败不启动编译、失败结果提示、项目隔离和编译状态持久化、所选模块路径过滤、首次开启不保存已有工作，以及原有7项测试。新增自动编译未在用户 IDE 内安装联调；之前 PID 19260 的 JRebel V1→V2 验证属于旧版手动编译链路，不能当作新版自动编译已验证。

0.1.1 修正：用户实际安装 0.1.0 时，宿主拒绝 Tools 菜单中的 `submenu` 字段。已删除全部六处；命令直接显示在 Tools 菜单中。新增 TOML 配置回归测试，7 项测试通过，重新构建和 WASM 验证通过，并检查最终 0.1.1 安装包确实不含 `submenu`。仍未在 IDE 内重新安装验证。

- `cargo test --locked`：6 项通过，0 项失败。测试包含路径越界和 XML 转义、Agent 参数/Maven fork 参数、已有映射备份和幂等生成、项目存储隔离与禁用、无效配置不写文件、写入权限拒绝时保留旧文件且不启用项目。
- `cargo build --locked --release --target wasm32-wasip2`：成功。编译器 Rust 1.100.0，官方 SDK 来自 OpenIDEA v0.1.44，WIT API 1.7.0。
- `wasm-tools validate dist/plugin.wasm`：成功，wasm-tools 1.263.0。
- `wasm-tools component wit`：确认导出 `openidea:plugin/run-configurations@1.7.0`、`lifecycle`、`commands`、`event-handler`；OpenIDEA 接口导入均为 1.7.0。
- 安装包 ZIP 内容检查：根目录含 `plugin.toml`、`settings.schema.json`、`README.md`，以及 `dist/plugin.wasm`。

以上是源码测试、构建、WASM 格式与打包检查，不等同于 IDE 运行验证。没有运行官方 `openidea-plugin check`，没有修改现有 OpenIDEA 安装/授权/项目配置，也没有安装插件或使用 JRebel 加载真实应用。界面、宿主加载、运行配置刷新、Windows Maven 启动、真实授权和热更新都需进一步联调。不能据此承诺与 IDEA 等同或覆盖所有框架、JDK 和构建布局。

工具链和中间文件保存在本次工作目录的 `work/`，没有把 Rust 加入用户全局 PATH。源码附带官方 SDK，构建时无需依赖该工作目录。

## 0.4.5 顶部工具栏状态（2026-11-16）

- `cargo build --offline --release --target wasm32-wasip2`：成功，使用随本次工作区提供的 Rust 工具链。
- `wasm-tools validate dist/plugin.wasm`：成功。
- `.oiplugin` 包已生成并检查 ZIP 条目：包含 `plugin.toml`、`settings.schema.json`、`README.md` 和 `dist/plugin.wasm`。
- Echo JRebel 的插件 API 运行组件仍基于随源码附带的 API 1.7 SDK；manifest 新增的 `placement = "mainToolbar"` 需要 OpenIDEA 本体支持 API 1.9 的对应声明。
- 当前环境未能完成 OpenIDEA 本体构建：前端依赖未安装，Rust 离线依赖缓存缺少一个 Git 依赖。没有在 IDE 中安装、启动或验证顶部状态项。
- 本次整理后的 `build.ps1` 尚未在干净环境执行；它会运行 Rust 检查、构建 WASM 并生成版本化 `.oiplugin`。
