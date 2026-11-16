# JRebel integration implementation plan

Goal: deliver a globally installable OpenIDEA plugin with project opt-in.
Architecture: Rust component using pinned OpenIDEA SDK, global setting and project storage; native Agent runs inside application JVM.
Spec: design.md

- [x] Implement and test safe project paths, XML mappings, backups and launch argument construction.
- [x] Add global Agent setting, project wizard, editable stored project configuration, Run/Compile/Check/Disable commands, and configuration provider.
- [x] Compile wasm32-wasip2, inspect manifest and exports, package source and installable plugin, document install and limits. Full official host validation and IDE runtime testing remain unverified.

Review focus: paths containing spaces/XML characters; failed configuration must not enable project; existing rebel.xml preserved; compile failures must remain visible; disabled project must publish no launch configuration. No commits or publishing.
