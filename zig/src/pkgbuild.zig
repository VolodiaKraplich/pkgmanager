const std = @import("std");
const Allocator = std.mem.Allocator;
const fs = std.fs;
const mem = std.mem;
const print = std.debug.print;
const debug = @import("utils/debug.zig");
const errors = @import("errors.zig");

/// PKGBUILD information structure
pub const PkgbuildInfo = struct {
    pkg_name: []const u8,
    pkg_ver: []const u8,
    pkg_rel: []const u8,
    arch: std.ArrayListUnmanaged([]const u8),
    depends: std.ArrayListUnmanaged([]const u8),
    make_depends: std.ArrayListUnmanaged([]const u8),
    check_depends: std.ArrayListUnmanaged([]const u8),

    const Self = @This();

    pub fn init() Self {
        return Self{
            .pkg_name = "",
            .pkg_ver = "",
            .pkg_rel = "",
            .arch = std.ArrayListUnmanaged([]const u8){},
            .depends = std.ArrayListUnmanaged([]const u8){},
            .make_depends = std.ArrayListUnmanaged([]const u8){},
            .check_depends = std.ArrayListUnmanaged([]const u8){},
        };
    }

    pub fn deinit(self: *Self, allocator: Allocator) void {
        if (self.pkg_name.len > 0) allocator.free(self.pkg_name);
        if (self.pkg_ver.len > 0) allocator.free(self.pkg_ver);
        if (self.pkg_rel.len > 0) allocator.free(self.pkg_rel);

        for (self.arch.items) |item| allocator.free(item);
        self.arch.deinit(allocator);

        for (self.depends.items) |item| allocator.free(item);
        self.depends.deinit(allocator);

        for (self.make_depends.items) |item| allocator.free(item);
        self.make_depends.deinit(allocator);

        for (self.check_depends.items) |item| allocator.free(item);
        self.check_depends.deinit(allocator);
    }

    /// Get all dependencies combined
    pub fn getAllDependencies(self: *const Self, allocator: Allocator) !std.ArrayListUnmanaged([]const u8) {
        var all_deps = std.ArrayListUnmanaged([]const u8){};
        try all_deps.appendSlice(allocator, self.depends.items);
        try all_deps.appendSlice(allocator, self.make_depends.items);
        try all_deps.appendSlice(allocator, self.check_depends.items);
        return all_deps;
    }

    /// Check if all required fields are present
    pub fn isValid(self: *const Self) bool {
        return self.pkg_name.len > 0 and self.pkg_ver.len > 0 and self.pkg_rel.len > 0;
    }
};

const ParseState = enum { idle, in_array };

/// Parse PKGBUILD file safely without executing it
pub fn parsePkgbuild(allocator: Allocator, path: []const u8) !PkgbuildInfo {
    const file = fs.cwd().openFile(path, .{}) catch |err| switch (err) {
        error.FileNotFound => {
            print("Error: Could not find PKGBUILD file at path: {s}\n", .{path});
            return errors.BuilderError.FileNotFound;
        },
        else => return err,
    };
    defer file.close();

    const content = try file.readToEndAlloc(allocator, 1024 * 1024); // 1MB max
    defer allocator.free(content);

    var info = PkgbuildInfo.init();
    errdefer info.deinit(allocator);

    var state: ParseState = .idle;
    var target_array: ?*std.ArrayListUnmanaged([]const u8) = null;
    var array_content = std.ArrayListUnmanaged(u8){};
    defer array_content.deinit(allocator);

    var lines = mem.splitSequence(u8, content, "\n");
    var line_count: u32 = 0;

    while (lines.next()) |line| {
        line_count += 1;
        if (line_count <= 15) {
            debug.debugPrint("{d:2}: {s}", .{ line_count, line });
        }

        const trimmed = cleanLine(line);
        if (trimmed.len == 0 or trimmed[0] == '#') continue;

        switch (state) {
            .idle => {
                if (try parseVariableAssignment(allocator, trimmed, &info, &state, &target_array, &array_content)) {
                    // Assignment was handled
                }
            },
            .in_array => {
                try array_content.appendSlice(allocator, trimmed);
                try array_content.append(allocator, ' ');
            },
        }

        // Check for the end of an array
        if (state == .in_array and mem.indexOf(u8, trimmed, ")") != null) {
            try finalizeArrayParsing(allocator, &array_content, target_array);
            state = .idle;
            target_array = null;
        }
    }

    debug.debugPrint("Final parsed values - pkgname:'{s}', pkgver:'{s}', pkgrel:'{s}'", .{ info.pkg_name, info.pkg_ver, info.pkg_rel });

    if (!info.isValid()) {
        print("Error: Could not parse required variables from PKGBUILD. Found: pkgname='{s}', pkgver='{s}', pkgrel='{s}'\n", .{ info.pkg_name, info.pkg_ver, info.pkg_rel });
        return errors.BuilderError.PkgbuildParseError;
    }

    return info;
}

/// Clean a line by trimming and removing comments
fn cleanLine(line: []const u8) []const u8 {
    var trimmed = mem.trim(u8, line, " \t\r");

    // Remove comments from the current line before processing
    if (mem.indexOf(u8, trimmed, "#")) |comment_pos| {
        trimmed = mem.trimRight(u8, trimmed[0..comment_pos], " \t");
    }

    return trimmed;
}

/// Parse a variable assignment line
fn parseVariableAssignment(allocator: Allocator, line: []const u8, info: *PkgbuildInfo, state: *ParseState, target_array: *?*std.ArrayListUnmanaged([]const u8), array_content: *std.ArrayListUnmanaged(u8)) !bool {
    const eq_pos = mem.indexOf(u8, line, "=") orelse return false;

    var var_name = mem.trim(u8, line[0..eq_pos], " \t");
    // Handle the += operator by stripping the '+'
    if (var_name.len > 0 and var_name[var_name.len - 1] == '+') {
        var_name = var_name[0 .. var_name.len - 1];
    }

    var value = mem.trim(u8, line[eq_pos + 1 ..], " \t");

    // Check for the start of a multi-line or single-line array
    if (mem.startsWith(u8, value, "(")) {
        target_array.* = getTargetArray(var_name, info);

        if (target_array.* != null) {
            state.* = .in_array;
            try array_content.appendSlice(allocator, value[1..]);
            try array_content.append(allocator, ' ');
        }
    } else {
        // Handle single string values
        const clean_value = removeQuotes(value);
        try assignSingleValue(allocator, var_name, clean_value, info);
        debug.debugPrint("Found variable: {s} = '{s}'", .{ var_name, clean_value });
    }

    return true;
}

/// Get the target array for a given variable name
fn getTargetArray(var_name: []const u8, info: *PkgbuildInfo) ?*std.ArrayListUnmanaged([]const u8) {
    if (mem.eql(u8, var_name, "arch")) {
        return &info.arch;
    } else if (mem.eql(u8, var_name, "depends")) {
        return &info.depends;
    } else if (mem.eql(u8, var_name, "makedepends")) {
        return &info.make_depends;
    } else if (mem.eql(u8, var_name, "checkdepends")) {
        return &info.check_depends;
    }
    return null;
}

/// Remove quotes from a value string
fn removeQuotes(value: []const u8) []const u8 {
    if (value.len >= 2) {
        if ((value[0] == '"' and value[value.len - 1] == '"') or
            (value[0] == '\'' and value[value.len - 1] == '\''))
        {
            return value[1 .. value.len - 1];
        }
    }
    return value;
}

/// Assign a single value to the appropriate field
fn assignSingleValue(allocator: Allocator, var_name: []const u8, value: []const u8, info: *PkgbuildInfo) !void {
    if (mem.eql(u8, var_name, "pkgname")) {
        if (info.pkg_name.len == 0) info.pkg_name = try allocator.dupe(u8, value);
    } else if (mem.eql(u8, var_name, "pkgver")) {
        if (info.pkg_ver.len == 0) info.pkg_ver = try allocator.dupe(u8, value);
    } else if (mem.eql(u8, var_name, "pkgrel")) {
        if (info.pkg_rel.len == 0) info.pkg_rel = try allocator.dupe(u8, value);
    }
}

/// Finalize array parsing by extracting items
fn finalizeArrayParsing(allocator: Allocator, array_content: *std.ArrayListUnmanaged(u8), target_array: ?*std.ArrayListUnmanaged([]const u8)) !void {
    if (target_array) |arr| {
        // Find the closing paren and truncate the string there to remove junk
        const end_paren_pos = mem.indexOf(u8, array_content.items, ")") orelse array_content.items.len;
        const final_array_str = array_content.items[0..end_paren_pos];

        var array_items = mem.splitAny(u8, final_array_str, " \t\n\r");
        while (array_items.next()) |item| {
            const clean_item = mem.trim(u8, item, " \t'\"");
            if (clean_item.len > 0) {
                try arr.append(allocator, try allocator.dupe(u8, clean_item));
            }
        }
    }
    array_content.clearRetainingCapacity();
}
