# Echo JRebel

Echo JRebel is an unofficial OpenIDEA plugin that connects OpenIDEA projects to a JRebel Agent already installed and licensed on your computer. It does not include, download, or activate JRebel.

## What it does

- Detects Java modules and creates JRebel `rebel.xml` mappings for selected modules.
- Adds OpenIDEA run and compile actions that launch with your local JRebel Agent.
- Can save and compile selected modules after edits have been idle for about five seconds. It serializes compile requests so it does not start concurrent Maven or Gradle builds.
- Shows the automatic compile state and the last successful compile time in the status area.
- Version 0.4.5 adds a live status item to the main toolbar.

## Compatibility

Version 0.4.5 requires **OpenIDEA Plugin API 1.9 or later**, including support for `placement = "mainToolbar"` on plugin status items. OpenIDEA 0.1.44 / Plugin API 1.7 cannot load this version. The main-toolbar placement change is currently developed in the OpenIDEA source alongside this plugin; wait for a compatible OpenIDEA build before installing 0.4.5.

JRebel itself must be installed and licensed separately. In OpenIDEA plugin settings, set the absolute path to the native Agent library, such as `C:/JRebel/lib/jrebel64.dll` on Windows x64. The Agent architecture must match the JDK used to run the service.

## Build on Windows

Install stable Rust with the WebAssembly component target, then run:

```powershell
rustup target add wasm32-wasip2
.\build.ps1
```

The script runs the Rust checks, builds `dist/plugin.wasm`, and creates `release/echo-jrebel-0.4.5.oiplugin`. Install the package through **Settings → Plugins → gear menu → Install Plugin from Disk…**. Review the requested process permissions before granting them.

## First use

1. Install the plugin and set the JRebel Agent library path in **Settings → Plugins → Echo JRebel**.
2. Open a Java project and run **Echo JRebel: Run Application**. The plugin detects modules and creates the selected module mappings.
3. Start the generated **JRebel: Run Application** configuration.
4. Enable automatic compilation from the Echo JRebel toolbar/status item or tool window. After editing selected module sources or resources, wait about five seconds for Save All and compilation.

Compile success means new class files were produced. Whether a particular change is reloadable depends on JRebel, the JDK, framework, and change type; this plugin does not verify that the running JVM applied every change.

## Permissions and safety

The plugin needs project read/write access to inspect modules and generate `rebel.xml`, storage for per-project settings, and process permissions for the selected Java/Maven/Gradle executables. It may request the IDE's Save All action for automatic compilation. Inspect the permission prompt and grant only the tools you plan to use. The plugin does not send project contents to a network service.

Generated `rebel.xml` files use local absolute paths. Existing files are backed up before replacement. Review the generated files before committing them to your project.

## Project status

This is a local development source repository, not an official JRebel or OpenIDEA plugin. The plugin ID remains `local.jrebel-integration` to preserve existing local plugin data. Before submitting it to the OpenIDEA community marketplace, choose a publisher ID, migrate the plugin ID deliberately, choose and add an open-source license, sign the package, and publish a release. A plugin requiring the new toolbar placement cannot pass marketplace compatibility checks until an OpenIDEA release supports that API.

Older implementation notes and the earlier 0.3.x guide are preserved in [`docs/LEGACY_GUIDE.md`](docs/LEGACY_GUIDE.md); follow the compatibility and build instructions on this page for version 0.4.5.
