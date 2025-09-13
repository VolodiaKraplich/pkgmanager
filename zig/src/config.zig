const std = @import("std");
const process = std.process;
const mem = std.mem;
const print = std.debug.print;

/// Available commands
pub const CommandType = enum {
    deps,
    build,
    artifacts,
    version,
    help,
};

/// Build-specific options
pub const BuildOptions = struct {
    clean: bool = false,
    sign: bool = false,
};

/// Main configuration structure
pub const Config = struct {
    command: CommandType,
    debug: bool = false,
    artifacts_dir: []const u8 = "artifacts",
    version_file: []const u8 = "version.env",
    build_options: BuildOptions = .{},
};

/// Parse command line arguments into configuration
pub fn parseArgs(allocator: std.mem.Allocator) !Config {
    var config = Config{ .command = .help };

    const args = try process.argsAlloc(allocator);
    defer process.argsFree(allocator, args);

    if (args.len < 2) {
        return config;
    }

    var i: usize = 1;
    while (i < args.len) : (i += 1) {
        const arg = args[i];

        if (mem.eql(u8, arg, "--debug")) {
            config.debug = true;
        } else if (mem.eql(u8, arg, "deps")) {
            config.command = .deps;
        } else if (mem.eql(u8, arg, "build")) {
            config.command = .build;
        } else if (mem.eql(u8, arg, "artifacts")) {
            config.command = .artifacts;
        } else if (mem.eql(u8, arg, "version")) {
            config.command = .version;
        } else if (mem.eql(u8, arg, "--clean")) {
            config.build_options.clean = true;
        } else if (mem.eql(u8, arg, "--sign")) {
            config.build_options.sign = true;
        } else if (mem.eql(u8, arg, "-o") or mem.eql(u8, arg, "--output-dir")) {
            if (i + 1 < args.len) {
                i += 1;
                config.artifacts_dir = args[i];
            }
        } else if (mem.eql(u8, arg, "--output-file")) {
            if (i + 1 < args.len) {
                i += 1;
                config.version_file = args[i];
            }
        } else if (mem.eql(u8, arg, "--help") or mem.eql(u8, arg, "-h")) {
            config.command = .help;
        }
    }

    return config;
}

/// Print help message
pub fn printHelp() void {
    print(
        \\builder - A reliable tool for building Arch Linux/PrismLinux packages in GitLab CI
        \\
        \\This tool replaces fragile shell scripts for dependency installation, package
        \\building, and artifact collection. It safely parses PKGBUILD files without sourcing them.
        \\
        \\USAGE:
        \\    builder [--debug] <COMMAND> [OPTIONS]
        \\
        \\COMMANDS:
        \\    deps        Parses PKGBUILD and installs dependencies using paru
        \\    build       Builds the package using paru
        \\                --clean    Clean previous build artifacts before building
        \\                --sign     Sign the package using GPG
        \\    artifacts   Collects build artifacts (packages, logs, etc.)
        \\                -o, --output-dir <DIR>    Directory to place artifacts in (default: artifacts)
        \\    version     Generates a .env file with version information for GitLab CI
        \\                --output-file <FILE>      The .env file to generate (default: version.env)
        \\    help        Show this help message
        \\
        \\GLOBAL OPTIONS:
        \\    --debug     Enable debug output
        \\
    , .{});
}
