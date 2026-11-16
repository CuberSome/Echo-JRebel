# OpenIDEA JRebel integration

Approved direction: globally installed plugin, global Agent path, per-project opt-in.

0.2.0 approved extension: five-second idle Save All, project-scoped serialized compilation, one merged follow-up for edits during a build, failure notifications, and a pause switch. State and content fingerprints live separately from the existing launch JSON, preserving old configurations. Manual Compile Updates joins the same queue. The switch is off for projects without saved auto state.

Native scheduled events have a 60-second minimum and no window context. Use bounded live-buffer observation in a cancellable background task triggered by window editing events: poll at 100ms, yield after 5.5 seconds to queued commands/events, preserve last observed change time, and invoke file.save before run.start. This preserves the window context required by those APIs without a daemon/network dependency. A pause command during continuous edits can wait for the current batch; cancelling its task pauses directly. The host's Save All saves all editors; only configured module source/resource edits trigger compilation.

Use OpenIDEA's native WebAssembly SDK, pinned to v0.1.44. Reuse a locally installed and licensed native JRebel Agent; do not redistribute JRebel. Create plugin-owned run configurations because the inspected API cannot inject JVM options into arbitrary existing configurations.

First version: global Agent setting; per-project setup and module selection; backup-preserving rebel.xml generation; Maven Spring Boot or direct Java launch; Maven/Gradle compile configuration; Agent check; disable without deleting user files. Store project settings in plugin storage keyed by project root. Existing run configurations and build scripts remain untouched. Compile is explicit in this version; automatic save compilation, remote launch and Gradle bootRun integration are deferred.

Alternatives: modifying the IDE core provides closer run-button integration but requires maintaining an IDE fork; porting IntelliJ binaries is unsupported. Native plugin integration is the selected approach.

Verify XML escaping, path containment, backup preservation, project isolation, argument construction, and native/WASM compilation. Actual licensed hot reload requires a local JRebel installation and a representative application and is a separate unverified item until exercised.
