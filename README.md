# Echo JRebel

Echo JRebel 是一款非官方 OpenIDEA 插件，用于接入本机已安装并授权的 JRebel Agent，让 Java 项目在 OpenIDEA 中使用热部署。插件不附带、下载或激活 JRebel。

## 主要功能

- 识别 Java 模块，为选定模块生成 JRebel `rebel.xml` 映射。
- 提供运行服务和编译更新入口，启动服务时加载本机 JRebel Agent。
- 停止修改约 5 秒后自动保存并编译；连续修改会重新计时，编译期间的新修改合并为后续一次编译。
- 串行执行编译请求，避免插件同时启动多个 Maven 或 Gradle 编译进程。
- 显示自动编译状态和上一次成功编译的时间。
- 0.4.5 新增顶部工具栏动态状态项，点击可切换自动编译，悬停可查看上次成功编译时间。

## 版本兼容要求

**0.4.5 需要支持顶部状态项扩展的 OpenIDEA 本体。** 插件声明要求 Plugin API 1.9，并使用状态项的 `placement = "mainToolbar"` 扩展；仅有 API 1.9 版本号不足以确认本体已包含这项改动。

- OpenIDEA 0.1.44 / Plugin API 1.7 无法加载此版本。
- 顶部状态项接口改动目前在配套 OpenIDEA 源码中维护，尚未完成本体构建与界面联调；安装前需确认所用 OpenIDEA 已包含该扩展。
- 本仓库包含插件源码；OpenIDEA 本体的接口改动不在此仓库内。

需要单独安装并授权 JRebel，然后在插件设置中填写 native Agent 动态库的绝对路径。Windows x64 示例：`C:/JRebel/lib/jrebel64.dll`。应填写动态库文件路径，Agent 架构需与运行服务的 JDK 匹配。

## 在 Windows 上构建

安装支持 Rust edition 2024 的稳定版 Rust 后，在仓库根目录执行：

```powershell
rustup target add wasm32-wasip2
.\build.ps1
```

脚本会运行 Rust 测试、构建 `dist/plugin.wasm`，并根据 `plugin.toml` 中的版本号生成安装包。当前版本输出为：

```text
release/echo-jrebel-0.4.5.oiplugin
```

在 OpenIDEA 的 **Settings → Plugins → 齿轮菜单 → Install Plugin from Package File…** 中选择安装包。安装时按实际需要授予 Java、Maven 或 Gradle 的执行权限。

## 首次使用

1. 安装兼容版本的插件，在 **Settings → Plugins → Echo JRebel** 中设置 JRebel Agent 动态库路径。
2. 打开 Java 项目，通过 **Tools → 运行服务（JRebel）** 或 Echo JRebel 工具窗口开始配置。插件识别模块并生成映射；有多个启动模块时，根据提示选择服务。
3. 首次配置完成后，在右上角选择生成的 **JRebel: Run Application** 运行配置，启动服务。
4. 新项目默认开启自动编译，也可以通过工具栏、状态栏或工具窗口切换开关。修改已启用模块的源码或资源后，停止编辑约 5 秒，插件执行 **Save All** 并启动编译。
5. 编译完成后，在状态栏或工具窗口查看最近成功时间；需要立即编译时，点击 **编译更新**。

编译范围由项目配置的编译命令决定；插件没有实现 IDEA 的“仅编译改动文件”编译器。Save All 会保存其他已修改的编辑器文件。

编译成功表示编译进程成功结束；Maven 也可能报告没有需要编译的文件。具体修改能否被运行中的 JVM 热加载，取决于 JRebel、JDK、框架及变更类型。插件记录的成功时间是编译完成时间，不是确认 JRebel 重载完成的时间。

## 权限与生成文件

插件使用以下权限：

| 权限 | 用途 |
| --- | --- |
| 项目读写 | 识别模块，生成和更新 `rebel.xml` |
| 存储 | 保存各项目的配置、自动编译开关和成功时间 |
| 事件与编辑器读取 | 检测源码修改，判断是否已停止编辑 |
| IDE 命令 | 自动编译前调用 Save All |
| Java / Maven / Gradle 进程 | 检查 Agent、启动服务或执行编译；按实际使用方式授权 |

插件没有声明网络访问权限。JRebel Agent 自身的授权和网络行为由 JRebel 管理。

生成的 `rebel.xml` 使用本机绝对路径；已有文件被替换前会备份。提交项目代码前请检查这些文件是否适合加入版本控制。

## 项目状态与发布

这是处于开发阶段的非官方集成插件。为延续已有本地插件配置，当前插件 ID 保留为 `local.jrebel-integration`。

提交 OpenIDEA 社区插件市场前，还需确定发布者 ID、规划插件 ID 迁移、选择并添加项目开源许可证、签名安装包，以及创建 Release。顶部状态项依赖的本体接口也需要进入官方支持的版本。

随附 OpenIDEA SDK 的许可证保留在 `vendor/openidea-plugin/LICENSE` 中。构建及验证的实际范围见 [验证记录](VERIFICATION.md)。

旧版实现说明和使用记录保存在 [历史说明](docs/LEGACY_GUIDE.md) 中；使用 0.4.5 时，以本页的兼容要求和构建步骤为准。
