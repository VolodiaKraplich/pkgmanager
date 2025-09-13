// Error definitions
pub const BuilderError = error{
    PkgbuildParseError,
    FileNotFound,
    CommandFailed,
    NoPackagesGenerated,
    NoArtifactsFound,
    InvalidArguments,
};
