package dev.echo.agent;

import java.io.IOException;
import java.io.Reader;
import java.lang.instrument.ClassDefinition;
import java.lang.instrument.Instrumentation;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.file.*;
import java.nio.file.attribute.BasicFileAttributes;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Instant;
import java.util.*;
import java.util.stream.Stream;

/** Method-body reload on an ordinary JVM. No framework or structural reload. */
public final class EchoAgent implements Runnable {
    private final Instrumentation instrumentation;
    private final List<Path> roots = new ArrayList<>();
    private final Map<Path, Stamp> files = new HashMap<>();
    private final Map<String, String> failures = new LinkedHashMap<>();
    private final Set<String> restartClasses = new HashSet<>();
    private final Path status;
    private final FileChannel lockChannel;
    private final FileLock lock;
    private final String session = UUID.randomUUID().toString();
    private long lastReload;
    private long reloaded;
    private boolean initialized;
    private volatile boolean stopped;

    private EchoAgent(String config, Instrumentation instrumentation) throws Exception {
        if (config == null || config.trim().isEmpty()) throw new IllegalArgumentException("Agent requires an absolute UTF-8 properties file path");
        this.instrumentation = instrumentation;
        if (!instrumentation.isRedefineClassesSupported()) throw new IllegalStateException("JVM does not support class redefinition");
        Path configPath = (config.startsWith("file:") ? Paths.get(java.net.URI.create(config)) : Paths.get(config)).toAbsolutePath().normalize();
        Properties p = new Properties();
        try (Reader reader = Files.newBufferedReader(configPath, StandardCharsets.UTF_8)) { p.load(reader); }
        for (int i = 0; p.containsKey("root." + i); i++) {
            Path root = Paths.get(p.getProperty("root." + i));
            if (!root.isAbsolute()) throw new IllegalArgumentException("Class roots must be absolute");
            root = root.normalize();
            if (!roots.contains(root)) roots.add(root);
        }
        if (roots.isEmpty()) throw new IllegalArgumentException("At least one class output root is required");
        status = configPath.resolveSibling("status.json");
        lockChannel = FileChannel.open(configPath.resolveSibling("agent.lock"), StandardOpenOption.CREATE, StandardOpenOption.WRITE);
        FileLock acquired;
        try { acquired = lockChannel.tryLock(); }
        catch (Exception e) { lockChannel.close(); throw e; }
        if (acquired == null) { lockChannel.close(); throw new IllegalStateException("Another Echo Agent already owns this project; use a separate configuration directory"); }
        lock = acquired;
    }

    public static void premain(String config, Instrumentation instrumentation) {
        try {
            final EchoAgent agent = new EchoAgent(config, instrumentation);
            Runtime.getRuntime().addShutdownHook(new Thread(() -> {
                agent.stopped = true;
                agent.report("stopped", "Application JVM stopped", "");
                try { agent.lock.release(); agent.lockChannel.close(); } catch (IOException ignored) { }
            }, "echo-agent-shutdown"));
            agent.scan(); // Baseline before main; do not reload every class at startup.
            agent.initialized = true;
            agent.report("ready", "Watching " + agent.roots.size() + " class output directories", "");
            Thread worker = new Thread(agent, "echo-agent-reloader");
            worker.setDaemon(true);
            worker.start();
        } catch (Exception e) {
            // Invalid Agent config must not terminate the application.
            System.err.println("[Echo Agent] START_FAILED: " + e);
        }
    }

    @Override public void run() {
        while (!stopped && !Thread.currentThread().isInterrupted()) {
            try { Thread.sleep(500); scan(); }
            catch (InterruptedException e) { Thread.currentThread().interrupt(); }
            catch (Exception e) { report("error", "Directory scan failed: " + e, ""); }
        }
    }

    private void scan() throws Exception {
        Set<Path> present = new HashSet<>();
        for (Path root : roots) {
            if (!Files.isDirectory(root, LinkOption.NOFOLLOW_LINKS)) continue;
            try (Stream<Path> paths = Files.walk(root)) {
                Iterator<Path> it = paths.filter(p -> p.toString().endsWith(".class") && Files.isRegularFile(p, LinkOption.NOFOLLOW_LINKS)).iterator();
                while (it.hasNext()) {
                    Path path = it.next();
                    if (!present.add(path)) continue;
                    try { inspect(root, path); }
                    catch (IOException e) { /* Compiler may be replacing a file; retry next scan. */ }
                }
            }
        }
        // Keep tombstones: a deleted/recreated class may still be loaded in the JVM.
    }

    private void inspect(Path root, Path path) throws Exception {
        BasicFileAttributes a = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        Stamp previous = files.get(path);
        long modified = a.lastModifiedTime().toMillis();
        if (previous != null && previous.modified == modified && previous.size == a.size() && !previous.pending) return;
        if (a.size() > 16 * 1024 * 1024) return;
        byte[] bytes = Files.readAllBytes(path);
        BasicFileAttributes after = Files.readAttributes(path, BasicFileAttributes.class, LinkOption.NOFOLLOW_LINKS);
        if (after.size() != a.size() || !after.lastModifiedTime().equals(a.lastModifiedTime())) return;
        byte[] hash = MessageDigest.getInstance("SHA-256").digest(bytes);
        if (previous != null && Arrays.equals(previous.hash, hash) && !previous.pending) {
            files.put(path, new Stamp(modified, bytes.length, hash, false));
            return;
        }
        if (!initialized) { files.put(path, new Stamp(modified, bytes.length, hash, false)); return; }
        // Require two successive identical reads to avoid partially written compiler output.
        if (previous == null || !previous.pending || !Arrays.equals(previous.hash, hash)) {
            files.put(path, new Stamp(modified, bytes.length, hash, true));
            return;
        }
        files.put(path, new Stamp(modified, bytes.length, hash, false));
        String name = root.relativize(path).toString().replace('\\', '.').replace('/', '.');
        name = name.substring(0, name.length() - 6);
        reload(name, bytes);
    }

    private void reload(String name, byte[] bytes) {
        if (name.equals("module-info") || name.startsWith("dev.echo.agent.")) return;
        long candidates = roots.stream().filter(root -> Files.isRegularFile(root.resolve(name.replace('.', '/') + ".class"))).count();
        if (candidates > 1) { report("restart_required", "Duplicate class name across configured output roots", name); return; }
        Class<?> target = null;
        for (Class<?> type : instrumentation.getAllLoadedClasses()) {
            if (!type.getName().equals(name)) continue;
            if (target != null) { report("restart_required", "Duplicate class name across class loaders; refusing ambiguous replacement", name); return; }
            target = type;
        }
        if (target == null) {
            report("not_loaded", "Class is not loaded; normal classpath must contain new classes", name);
            return;
        }
        if (!instrumentation.isModifiableClass(target)) { report("restart_required", "JVM reports this class as unmodifiable", name); return; }
        try {
            instrumentation.redefineClasses(new ClassDefinition(target, bytes));
            lastReload = System.currentTimeMillis();
            reloaded++;
            report("reloaded", "Method bodies replaced successfully", name);
        } catch (UnsupportedOperationException e) {
            report("restart_required", "Structural change is unsupported by this JVM: " + e.getMessage(), name);
        } catch (Exception | LinkageError e) {
            report("reload_failed", "Replacement rejected; last working class retained: " + e, name);
        }
    }

    private synchronized void report(String state, String message, String className) {
        if (stopped && !state.equals("stopped")) return;
        if (state.equals("restart_required") || state.equals("reload_failed")) {
            failures.put(className, message);
            if (state.equals("restart_required")) restartClasses.add(className);
        } else if (state.equals("reloaded")) {
            failures.remove(className);
            restartClasses.remove(className);
        }
        StringJoiner errors = new StringJoiner(",", "{", "}");
        failures.forEach((name, reason) -> errors.add(quote(name) + ":" + quote(reason)));
        long now = System.currentTimeMillis();
        String pid = java.lang.management.ManagementFactory.getRuntimeMXBean().getName().split("@")[0];
        String json = "{\"engine\":\"echo\",\"session\":" + quote(session) + ",\"pid\":" + quote(pid)
            + ",\"state\":" + quote(state) + ",\"updated_at\":" + now + ",\"last_reload_at\":" + lastReload
            + ",\"reload_count\":" + reloaded + ",\"pending_failures\":" + failures.size() + ",\"pending_restart\":" + !restartClasses.isEmpty()
            + ",\"failures\":" + errors + ",\"class\":" + quote(className) + ",\"message\":" + quote(message) + "}\n";
        System.out.println("[Echo Agent] " + Instant.ofEpochMilli(now) + " " + state + " " + className + " " + message);
        Path temp = status.resolveSibling("status.json.tmp");
        try {
            Files.write(temp, json.getBytes(StandardCharsets.UTF_8));
            try { Files.move(temp, status, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE); }
            catch (AtomicMoveNotSupportedException e) { Files.move(temp, status, StandardCopyOption.REPLACE_EXISTING); }
        } catch (IOException e) { System.err.println("[Echo Agent] Cannot write status: " + e); }
    }

    private static String quote(String s) {
        StringBuilder out = new StringBuilder("\"");
        for (char c : s.toCharArray()) {
            if (c == '"' || c == '\\') out.append('\\').append(c);
            else if (c < 32) out.append(String.format("\\u%04x", (int) c));
            else out.append(c);
        }
        return out.append('"').toString();
    }

    private static final class Stamp {
        final long modified, size;
        final byte[] hash;
        final boolean pending;
        Stamp(long modified, long size, byte[] hash, boolean pending) {
            this.modified = modified; this.size = size; this.hash = hash; this.pending = pending;
        }
    }
}
