const std = @import("std");
const fs = std.fs;
const mem = std.mem;
const process = std.process;
const print = std.debug.print;
const pkgbuild = @import("../pkgbuild.zig");

/// Handle the version generation command
pub fn handleVersionCommand(allocator: std.mem.Allocator, version_file: []const u8) !void {
    print("Generating version info file at {s}\n", .{version_file});

    var info = pkgbuild.parsePkgbuild(allocator, "PKGBUILD") catch |err| {
        print("Error parsing PKGBUILD: {}\n", .{err});
        return;
    };
    defer info.deinit(allocator);

    const version_info = try generateVersionInfo(allocator, &info);
    defer allocator.free(version_info);

    try writeVersionFile(version_file, version_info);
    print("Version info generated successfully:\n{s}", .{version_info});
}

/// Generate version information content
fn generateVersionInfo(allocator: std.mem.Allocator, info: *const pkgbuild.PkgbuildInfo) ![]u8 {
    const ci_commit_tag = process.getEnvVarOwned(allocator, "CI_COMMIT_TAG") catch try allocator.dupe(u8, info.pkg_ver);
    defer if (!mem.eql(u8, ci_commit_tag, info.pkg_ver)) allocator.free(ci_commit_tag);

    const ci_job_id = process.getEnvVarOwned(allocator, "CI_JOB_ID") catch try allocator.dupe(u8, "local");
    defer if (!mem.eql(u8, ci_job_id, "local")) allocator.free(ci_job_id);

    const timestamp = std.time.timestamp();

    const arch_str = try std.mem.join(allocator, " ", info.arch.items);
    defer allocator.free(arch_str);

    return try std.fmt.allocPrint(allocator,
        \\VERSION={s}
        \\PKG_RELEASE={s}
        \\FULL_VERSION={s}-{s}
        \\PACKAGE_NAME={s}
        \\TAG_VERSION={s}
        \\BUILD_JOB_ID={s}
        \\BUILD_DATE={d}
        \\ARCH="{s}"
        \\
    , .{ info.pkg_ver, info.pkg_rel, info.pkg_ver, info.pkg_rel, info.pkg_name, ci_commit_tag, ci_job_id, timestamp, arch_str });
}

/// Write version information to file
fn writeVersionFile(version_file: []const u8, content: []const u8) !void {
    const file = try fs.cwd().createFile(version_file, .{});
    defer file.close();
    try file.writeAll(content);
}
