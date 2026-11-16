## 0.4.5 顶部工具栏状态

新增顶部 Echo JRebel 状态项，显示自动编译开启、暂停、等待或编译中；点击可切换自动编译，悬停查看最近成功编译时间。需要支持 status item `placement = "mainToolbar"` 的 OpenIDEA Plugin API 1.9 或更新版本。底部状态栏继续保留。

## 0.4.4 默认开启自动编译

新项目默认开启自动编译。旧版本没有区分“默认关闭”和“用户手动暂停”，因此升级后旧状态统一按新默认开启；在 0.4.4 及之后手动暂停的项目会记住该选择。项目配置后，修改启用模块的源码并停止编辑约5秒会自动保存和编译。

## 0.4.3 控制按钮样式

工具窗口采用更清晰的操作层级：运行服务作为主按钮，编译和模块配置作为次按钮，自动编译显示实际开关状态；模块按钮增加启停状态语义、焦点样式和按下反馈。配色沿用 OpenIDEA 主题变量。

## 0.4.2 模块启停控制

Echo JRebel 工具窗口逐个列出已配置的热部署模块，可直接启用或停用。切换会同步更新模块的 `rebel.xml`：停用后映射保留空 classpath，重新启用会恢复 classes/resources 映射；被覆盖的旧文件继续按 `.jrebel-backup-N` 备份。自动编译只监听启用模块的源码和资源。至少保留一个启用模块。旧项目配置缺少模块状态时按启用处理，升级后无需重配。

## 0.4.1 工具窗口样式优化

Echo JRebel 窗口使用 OpenIDEA 主题变量绘制品牌标题、自动编译/最近成功状态、热部署模块卡片和紧凑操作区。按钮在沙箱 HTML 中通过白名单消息触发插件命令；项目路径和模块内容经过 HTML 转义。

## 0.4.0 Echo JRebel 工具窗口

新增原生 Echo JRebel 工具窗口：查看项目热部署模块和自动编译/上次成功状态；工具栏按钮可运行服务、编译更新、切换自动编译、配置模块或重置项目。首次初始化仍由现有自动识别流程处理。

## 0.3.9 多启动模块初始化优化

Maven 项目自动收集有 Java 源码的部署模块。检测到多个 Spring Boot 启动模块时，只显示这些启动模块供单选；不再回退到全量模块多选和 JSON 编辑。

## 0.3.8 状态栏时间显示

状态栏改为紧凑格式 Echo JRebel · MM/DD HH:mm，避免完整中文日期在右下角被截断。悬停提示仍显示完整年月日和秒。只有成功编译后才会出现时间；安装升级后需有一次成功编译记录。

## 0.3.7 重置项目配置

Tools → 重置项目配置，经确认后删除当前项目配置和自动编译状态；保留 rebel.xml、备份和其他项目数据。

## 0.3.6 菜单调整

Tools 菜单恢复为直接展开的中文命令列表，不再弹出操作对话框。插件显示名保持 Echo JRebel。OpenIDEA 此菜单类型不支持子菜单层级。

## 0.3.5 Echo JRebel 操作整合

插件显示名改为 Echo JRebel，内部 ID 保留以延续已有项目配置。Tools 菜单将 JRebel 操作收纳到单一 Echo JRebel… 入口，点击后用 OpenIDEA 原生选择列表执行。OpenIDEA 不支持 Tools 子菜单；顶部运行/编译/自动编译命令仍保留。

## 0.3.3 首次运行提示修复

新项目第一次点 Tools → JRebel: Run Application 时，向导保存配置后明确提示去右上角运行配置启动。避免在运行配置列表刷新前立刻启动而无响应。

## 0.3.1 首次运行引导与状态栏时间

每个新版本使用独立版本号。工具栏的 JRebel: Run Application 首次点击会引导配置，再启动应用。状态栏显示最近一次编译成功时间；完成通知显示北京时间和耗时。

# JRebel Integration for OpenIDEA（非官方）

## 0.2.0 自动编译

升级前先等正在进行的编译结束。安装 `local.jrebel-integration-0.2.0.oiplugin` 覆盖旧版，同一插件 ID 的项目配置继续使用。新增所需权限是 `events`（修改和编译完成事件）、`editor.read`（读取未保存代码以判断是否还在修改）、`ide.commands`（调用 IDE 的 Save All）；按使用方式保留 `java` / `mvn` 授权。无需改正在运行服务的 JRebel Agent 配置。

首次默认关闭。点击工具栏或状态栏的自动编译开关，或搜索 **JRebel: Toggle Auto Compile (5s)** 开启当前项目。此后修改所选模块的源码/资源，停止修改约5秒后，插件先执行 **Save All**，再启动一次编译。Save All 会保存其他已修改的编辑器文件，但编译范围仍由本项目的 `compile_args` 决定。开启开关本身不会保存/编译已有编辑内容。

编译期间的多次修改会合并成一次后续编译，等待上一次完成且空闲达到5秒后执行；手动点击 Compile Updates 也使用相同串行队列。暂停会清除待编译请求，但不会中断已开始的 Maven；编译失败在通知和 Run 输出中显示，不无限重试。下次修改会重新触发。插件只负责保存/编译，不把编译成功当成 JRebel 已实际重载成功。

默认监控所选模块的 `src/main/java`、`src/main/kotlin` 和配置的资源目录。忽略 `target` / `build`、生成的 `rebel.xml` 与备份，保存回声不会再触发第二次编译。文件路径在 Windows 下统一处理普通和扩展路径前缀。

OpenIDEA 0.1.44 的原生周期任务最短60秒且没有窗口上下文，无法直接用于5秒保存。插件使用有界的后台任务观察当前编辑器内容；每100毫秒重新检查，单批最多约5.5秒，持续输入时由已排队的修改事件接续等待，不阻塞编辑器。连续输入期间插件自身的开关命令可能等待本批返回；也可取消底部的“等待停止修改5秒”后台任务立即暂停。IDE 本身的自动编译如果已开启，仍可能先于本插件的5秒计时生成 class。

0.2.0 已通过状态机/宿主模拟测试与 WASM 构建检查，尚未在你的 IDE 中完成新版安装和实际5秒自动编译联调。

全局安装、全局设置 Agent 路径，各项目分别启用。面向 OpenIDEA **0.1.44 / Plugin API 1.7** 构建，尚未在 IDE 内安装联调，也未进行持有效授权的 JRebel 实际热更新验证。

## 使用说明（0.4.2）

### 首次安装与 Agent 设置

1. 在 **Settings → Plugins → 齿轮 → Install Plugin from Disk…** 选择 `echo-jrebel-0.4.2-module-controls.oiplugin`。插件是本地开发的未签名插件，不是 JRebel 官方插件。
2. 按需授予 `process:java` 和 `process:mvn`（或 `process:gradle`）权限；插件需要它们来检查 Agent、启动服务和执行编译。项目读写、存储权限用于生成映射和保存项目配置。
3. 在 **Settings → Plugins → Echo JRebel** 设置 JRebel native Agent 的完整路径。Windows x64 示例：`C:/JRebel/lib/jrebel64.dll`。这是 `.dll` 文件路径，不是安装目录或 `jrebel.jar`；架构需与运行服务的 JDK 匹配。插件不附带 Agent 或许可证。
4. 可从 **Tools → 检查 Agent** 检查启动输出。确认 JRebel 加载信息及许可证状态；该操作只检查，不会替你激活许可证。

### 在项目里运行

1. 打开 Java 项目，从右侧工具窗口 **Echo JRebel** 或 **Tools → 运行服务（JRebel）** 开始。首次运行会识别 Maven Spring Boot 启动模块和 Java 源码模块，并生成项目配置及 `rebel.xml`。多个启动类时选择要运行的服务。
2. 首次配置完成后，按提示在右上角选择 **JRebel: Run Application** 并启动。后续可以直接从 Echo JRebel 面板点击 **运行服务**。
3. 面板的模块列表里可逐个启用或停用热部署映射。停用会清空该模块 `rebel.xml` 的 classpath；重新启用会恢复映射。已有 `rebel.xml` 被覆盖时，旧文件会备份为 `.jrebel-backup-N`。至少保留一个模块启用。
4. 点击 **自动编译 开 / 关** 开启当前项目的自动编译。修改启用模块的源码或资源后，停止编辑约 5 秒，插件会执行 Save All，再运行项目配置中的编译命令。编译期间的修改会合并为后续一次编译。
5. 也可以点击 **编译更新** 手动编译。编译成功后面板和状态栏更新最近成功时间；成功编译表示新 class 已编译，具体变更是否能被 JRebel 热加载取决于 JRebel、JDK 和变更类型。

每个项目分别保存模块和自动编译设置；Agent 路径是全局设置。编译范围由项目的 `compile_args` 决定。IDEA 与 OpenIDEA 同时运行同一服务时，避免它们并行写同一编译输出目录。

## 项目配置字段

Tools → JRebel: **Edit Project Configuration** 可修改配置。每个参数数组元素是一个进程参数，不需要加 shell 转义。

| 字段 | 作用 |
| --- | --- |
| `enabled` | 是否为项目提供应用和编译入口 |
| `launch` | `maven-boot` 或 `java` |
| `pom` | 启动模块的 pom，相对项目根目录，仅 Maven 启动使用 |
| `jvm_args` | 额外 JVM 参数，例如 `-Xmx1g`；Agent 参数由插件添加 |
| `java_args` | 直接 Java 启动参数，例如 `["-cp", "target/classes;lib/*", "com.acme.Main"]` 或 `["-jar", "target/app.jar"]` |
| `app_args` | 应用参数，例如 `["--server.port=8081"]` |
| `build_tool` | `mvn` 或 `gradle`，使用 IDE 已授权的可执行文件 |
| `compile_args` | 编译命令参数，默认 Maven `-DskipTests compile` / Gradle `classes` |
| `modules[].path` | 模块相对路径，空字符串为根模块 |
| `modules[].classes` | 相对模块目录的编译输出路径数组 |
| `modules[].resources` | 相对模块目录的资源路径数组 |
| `modules[].rebel_file` | 相对模块目录的配置输出位置，默认 `src/main/resources/rebel.xml` |

对于自定义输出目录、Kotlin 或混合构建项目，请修改映射和编译参数。直接 Java 启动的依赖类路径也需要自行填写。Maven 多模块依赖需按项目正常方式先完成构建/安装，插件不自动改变 reactor 配置。

生成的 `rebel.xml` 使用本机绝对目录；请自行决定是否加入版本控制。构建必须把它复制到应用 classpath：默认 Maven/Gradle Java resources 约定会完成此事，自定义 resources 配置需自行调整。热更新要求 `.class` 真正写入映射目录，只保存 `.java` 不会生效。

## 第一版边界

- 新增插件自己的运行配置，**不会给原有全部 Run/Debug 配置自动注入 Agent**。
- 提供停止修改约5秒后的自动 Save All 和编译、串行队列以及工具栏/状态栏开关；尚无启动前自动等待编译。
- 没有实现 Gradle `bootRun` 的 Agent 参数注入；Gradle 项目目前可用编译入口配合直接 Java 启动。
- Maven Spring Boot 启动参数按 Boot 2.x/3.x 的形式实现；其他版本尚未验证。
- 不提供远程服务器、Docker、SSH 或框架专用配置，也未实现与 IDEA 同等的调试集成。
- Agent 路径的格式会校验；是否存在、架构匹配、授权成功，需要 Check Agent 的实际结果确认。
- 不包含、下载或分发 JRebel，不管理授权。需使用自己的合法安装与授权；重载范围由 JRebel、JDK 和框架版本决定。
- 本插件依赖 IDE 的项目可信与进程授权机制。安装后若运行入口缺失，先检查授权和 Agent 路径，然后 Reload 插件。

## 禁用、回滚和卸载

**Disable for This Project** 撤下此项目的应用和编译入口，不停止已经运行的 JVM，也不删除文件。手动停止服务后，用原有启动配置即可不加载 Agent。

已有映射被修改前，保存在原文件旁的 `rebel.xml.jrebel-backup-N`。回滚时停止服务，将所需备份恢复为 `rebel.xml` 后重新编译。若文件是插件新建的且不再需要，可自行删除再重新编译，以移除输出目录中的映射。IDE 中禁用/卸载插件不会删除生成的映射或备份。

## 从源码构建

需要支持 Rust edition 2024 的稳定版 Rust，以及 `wasm32-wasip2` target：

```powershell
rustup target add wasm32-wasip2
cargo test --locked
cargo build --locked --release --target wasm32-wasip2
New-Item -ItemType Directory -Force dist
Copy-Item target/wasm32-wasip2/release/openidea_jrebel_integration.wasm dist/plugin.wasm
```

可用官方 `openidea-plugin check` / `pack` 进一步检查与打包。交付源代码内附 v0.1.44 的官方 Rust SDK，保留其 Apache-2.0 LICENSE；插件无需依赖本次工作目录才能重新构建。

## 参考

- [OpenIDEA v0.1.44 插件 SDK 与接口说明](https://github.com/l0ng-ai/openidea/blob/v0.1.44/plugins/README.md)
- [JRebel native Agent 启动方式](https://manuals.jrebel.com/jrebel/misc/switch-agents.html)
- [JRebel 与 Spring Boot](https://manuals.jrebel.com/jrebel/standalone/springboot.html)
- [rebel.xml 配置](https://manuals.jrebel.com/jrebel/standalone/config.html)
