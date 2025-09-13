package main

import (
	"fmt"
	"io"
	"log"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"sort"
	"strings"
	"time"

	"github.com/spf13/cobra"
)

var debugMode bool

// debugPrint prints debug messages only when debug mode is enabled
func debugPrint(format string, args ...any) {
	if debugMode {
		fmt.Printf("DEBUG: "+format+"\n", args...)
	}
}

// pkgbuildInfo holds the data extracted from a PKGBUILD file.
type pkgbuildInfo struct {
	PkgName      string
	PkgVer       string
	PkgRel       string
	Arch         []string
	Depends      []string
	MakeDepends  []string
	CheckDepends []string
}

// parsePKGBUILD safely reads a PKGBUILD file and extracts variables without executing it.
func parsePKGBUILD(path string) (*pkgbuildInfo, error) {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil, fmt.Errorf("could not read PKGBUILD file: %w", err)
	}
	info := &pkgbuildInfo{}
	sContent := string(content)

	lines := strings.Split(sContent, "\n")
	debugPrint("First 15 lines of PKGBUILD:")
	for i, line := range lines {
		if i >= 15 {
			break
		}
		debugPrint("%2d: %s", i+1, line)
	}

	// Single-line variable assignments with double quotes
	reDoubleQuoted := regexp.MustCompile(`(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*"([^"\n#]*?)"\s*(?:#.*)?$`)
	// Single-line variable assignments with single quotes
	reSingleQuoted := regexp.MustCompile(`(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*'([^'\n#]*?)'\s*(?:#.*)?$`)
	// Single-line variable assignments without quotes
	reUnquoted := regexp.MustCompile(`(?m)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*([^'"\n#]+?)\s*(?:#.*)?$`)

	// For array variables - handles multi-line arrays better
	reArray := regexp.MustCompile(`(?ms)^\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*\(\s*(.*?)\s*\)`)

	// Helper function to process matches
	processMatches := func(matches [][]string, valueIndex int) {
		debugPrint("Found %d variable matches", len(matches))
		for _, match := range matches {
			if len(match) < valueIndex+1 {
				continue
			}
			key := strings.TrimSpace(match[1])
			val := strings.TrimSpace(match[valueIndex])

			debugPrint("Found variable: %s = '%s'", key, val)

			switch key {
			case "pkgname":
				if info.PkgName == "" {
					info.PkgName = val
				}
			case "pkgver":
				if info.PkgVer == "" {
					info.PkgVer = val
				}
			case "pkgrel":
				if info.PkgRel == "" {
					info.PkgRel = val
				}
			}
		}
	}

	// Extract single-string variables with different quote types
	processMatches(reDoubleQuoted.FindAllStringSubmatch(sContent, -1), 2)
	processMatches(reSingleQuoted.FindAllStringSubmatch(sContent, -1), 2)
	processMatches(reUnquoted.FindAllStringSubmatch(sContent, -1), 2)

	// Extract array variables
	arrayMatches := reArray.FindAllStringSubmatch(sContent, -1)
	debugPrint("Found %d array matches", len(arrayMatches))

	for _, match := range arrayMatches {
		if len(match) < 3 {
			continue
		}
		key := strings.TrimSpace(match[1])
		val := match[2]

		// Clean up the array content more thoroughly
		// Remove comments first
		reComment := regexp.MustCompile(`(?m)#.*$`)
		val = reComment.ReplaceAllString(val, "")

		// Remove quotes and newlines, normalize whitespace
		cleanVal := strings.NewReplacer(
			"\n", " ",
			"\t", " ",
			"'", "",
			"\"", "",
			"  ", " ", // double spaces
		).Replace(val)

		// Split by whitespace and filter empty strings
		rawFields := strings.Fields(cleanVal)
		var fields []string
		for _, field := range rawFields {
			field = strings.TrimSpace(field)
			if field != "" {
				fields = append(fields, field)
			}
		}

		debugPrint("Found array: %s = %v", key, fields)

		switch key {
		case "arch":
			info.Arch = fields
		case "depends":
			info.Depends = fields
		case "makedepends":
			info.MakeDepends = fields
		case "checkdepends":
			info.CheckDepends = fields
		}
	}

	// Fallback: try to extract with simpler regex if nothing found
	if info.PkgName == "" || info.PkgVer == "" || info.PkgRel == "" {
		debugPrint("Primary parsing failed, trying fallback method...")

		// Very simple regex as fallback
		simpleRegex := regexp.MustCompile(`(?m)^([a-zA-Z_][a-zA-Z0-9_]*)\s*=\s*(.*)$`)
		simpleMatches := simpleRegex.FindAllStringSubmatch(sContent, -1)

		for _, match := range simpleMatches {
			if len(match) < 3 {
				continue
			}
			key := strings.TrimSpace(match[1])
			val := strings.TrimSpace(match[2])

			// Remove quotes and comments
			val = regexp.MustCompile(`\s*#.*$`).ReplaceAllString(val, "")
			val = strings.Trim(val, `"'`)

			debugPrint("Fallback found: %s = '%s'", key, val)

			switch key {
			case "pkgname":
				if info.PkgName == "" {
					info.PkgName = val
				}
			case "pkgver":
				if info.PkgVer == "" {
					info.PkgVer = val
				}
			case "pkgrel":
				if info.PkgRel == "" {
					info.PkgRel = val
				}
			}
		}
	}

	// Debug final parsed values
	debugPrint("Final parsed values - pkgname:'%s', pkgver:'%s', pkgrel:'%s'",
		info.PkgName, info.PkgVer, info.PkgRel)

	if info.PkgName == "" || info.PkgVer == "" || info.PkgRel == "" {
		return nil, fmt.Errorf("could not parse required variables from PKGBUILD. Found: pkgname='%s', pkgver='%s', pkgrel='%s'. This suggests the PKGBUILD format is unusual or contains complex variable assignments",
			info.PkgName, info.PkgVer, info.PkgRel)
	}

	return info, nil
}

// copyFile copies a file from src to dst
func copyFile(src, dst string) error {
	sourceFile, err := os.Open(src)
	if err != nil {
		return err
	}
	defer sourceFile.Close()

	destFile, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer destFile.Close()

	_, err = io.Copy(destFile, sourceFile)
	if err != nil {
		return err
	}

	// Copy file permissions
	info, err := os.Stat(src)
	if err != nil {
		return err
	}
	return os.Chmod(dst, info.Mode())
}

// runCommand executes a command and streams its output to stdout/stderr.
func runCommand(name string, args ...string) error {
	cmd := exec.Command(name, args...)
	cmd.Stdout = os.Stdout
	cmd.Stderr = os.Stderr
	debugPrint("Running command: %s %s", name, strings.Join(args, " "))
	if !debugMode {
		fmt.Printf("+ Running command: %s %s\n", name, strings.Join(args, " "))
	}
	return cmd.Run()
}

// --- COBRA COMMANDS ---

func main() {
	var rootCmd = &cobra.Command{
		Use:   "builder",
		Short: "A reliable tool for building Arch Linux/PrismLinux packages in GitLab CI.",
		Long:  `This tool replaces fragile shell scripts for dependency installation, package building, and artifact collection. It safely parses PKGBUILD files without sourcing them.`,
	}
	rootCmd.CompletionOptions = cobra.CompletionOptions{DisableDefaultCmd: true}
	rootCmd.PersistentFlags().BoolVar(&debugMode, "debug", false, "Enable debug output")

	// --- 'deps' command ---
	var depsCmd = &cobra.Command{
		Use:   "deps",
		Short: "Parses PKGBUILD and installs dependencies using paru.",
		Run: func(cmd *cobra.Command, args []string) {
			log.Println("Installing PKGBUILD dependencies...")
			info, err := parsePKGBUILD("PKGBUILD")
			if err != nil {
				log.Fatalf("Error: %v", err)
			}

			allDeps := append(info.Depends, info.MakeDepends...)
			allDeps = append(allDeps, info.CheckDepends...)

			if len(allDeps) == 0 {
				log.Println("No dependencies found in PKGBUILD.")
				return
			}

			log.Printf("Found dependencies: %v\n", allDeps)

			// Check for rust/rustup conflict and handle it
			hasRust := false
			hasRustup := false
			filteredDeps := []string{}

			for _, dep := range allDeps {
				switch dep {
				case "rust":
					hasRust = true
				case "rustup":
					hasRustup = true
				default:
					filteredDeps = append(filteredDeps, dep)
				}
			}

			// Handle rust/rustup conflict
			if hasRust || hasRustup {
				// Check if rustup is already installed
				if err := runCommand("which", "rustup"); err == nil {
					log.Println("rustup is already available, skipping rust package")
					// Remove cargo from filtered deps if it exists since rustup includes it
					newFilteredDeps := []string{}
					for _, dep := range filteredDeps {
						if dep != "cargo" {
							newFilteredDeps = append(newFilteredDeps, dep)
						}
					}
					filteredDeps = newFilteredDeps
				} else {
					// Neither rust nor rustup available, try to install rustup
					log.Println("Installing rustup for Rust toolchain...")
					filteredDeps = append(filteredDeps, "rustup")
				}
			}

			if len(filteredDeps) == 0 {
				log.Println("All dependencies are already satisfied.")
				return
			}

			// Try paru first
			paruArgs := []string{"-S", "--noconfirm", "--needed", "--asdeps"}
			paruArgs = append(paruArgs, filteredDeps...)

			if err := runCommand("paru", paruArgs...); err != nil {
				log.Printf("Paru failed, trying with sudo pacman: %v", err)
				// Try pacman with sudo
				pacmanArgs := []string{"-S", "--noconfirm", "--needed", "--asdeps"}
				pacmanArgs = append(pacmanArgs, filteredDeps...)
				if err := runCommand("sudo", append([]string{"pacman"}, pacmanArgs...)...); err != nil {
					log.Printf("Warning: Some dependencies might not be available: %v", err)
				}
			}
			log.Println("Dependencies installation attempted!")
		},
	}

	// --- 'build' command ---
	var cleanBuild bool
	var signPackage bool
	var buildCmd = &cobra.Command{
		Use:   "build",
		Short: "Builds the package using paru.",
		Run: func(cmd *cobra.Command, args []string) {
			if cleanBuild {
				log.Println("Cleaning previous builds...")
				files, _ := filepath.Glob("*.pkg.tar.*")
				for _, f := range files {
					os.Remove(f)
				}
				for _, dir := range []string{"src", "pkg"} {
					os.RemoveAll(dir)
				}
			}

			log.Println("Building package with paru...")
			buildArgs := []string{"-B", "--noconfirm", "./"}
			if signPackage {
				buildArgs = append(buildArgs, "--sign")
			}

			paruCmd := exec.Command("paru", buildArgs...)
			paruCmd.Env = append(os.Environ(), "CCACHE_DIR=/home/builder/.ccache")
			paruCmd.Stdout = os.Stdout
			paruCmd.Stderr = os.Stderr
			debugPrint("Running command: CCACHE_DIR=/home/builder/.ccache paru %s", strings.Join(buildArgs, " "))
			if !debugMode {
				fmt.Printf("+ Running command: CCACHE_DIR=/home/builder/.ccache paru %s\n", strings.Join(buildArgs, " "))
			}

			if err := paruCmd.Run(); err != nil {
				log.Fatalf("Package build failed: %v", err)
			}

			log.Println("Build completed successfully!")
			packageFiles, err := filepath.Glob("*.pkg.tar.*")
			if err != nil {
				log.Fatalf("Failed to search for package files: %v", err)
			}
			if len(packageFiles) == 0 {
				log.Fatalf(`No package file (*.pkg.tar.*) was generated by paru.

This usually means:
• The build was skipped (e.g. due to existing src/ or pkg/ directories)
• The PKGBUILD has a conditional 'exit 0'
• paru failed silently (check logs above)
• Dynamic pkgver/pkgrel caused unexpected naming

Please review the build output carefully for warnings or skipped steps.
`)
			}

			sort.Strings(packageFiles)

			log.Printf("Successfully built %d package(s): %v", len(packageFiles), packageFiles)

			lsArgs := append([]string{"-la"}, packageFiles...)
			if err := runCommand("ls", lsArgs...); err != nil {
				log.Printf("Warning: could not run 'ls' on generated packages: %v", err)
			}
		},
	}
	buildCmd.Flags().BoolVar(&cleanBuild, "clean", false, "Clean previous build artifacts and directories before building")
	buildCmd.Flags().BoolVar(&signPackage, "sign", false, "Sign the package using GPG")

	// --- 'artifacts' command ---
	var artifactsDir string
	var artifactsCmd = &cobra.Command{
		Use:   "artifacts",
		Short: "Collects build artifacts (packages, logs, etc.).",
		Run: func(cmd *cobra.Command, args []string) {
			log.Printf("Collecting build artifacts into directory: %s\n", artifactsDir)
			if err := os.MkdirAll(artifactsDir, 0755); err != nil {
				log.Fatalf("Could not create artifacts directory: %v", err)
			}

			foundPackages := false
			patterns := []string{"*.pkg.tar.*", "*.log", "PKGBUILD", ".SRCINFO"}
			for _, pattern := range patterns {
				files, _ := filepath.Glob(pattern)
				for _, f := range files {
					dest := filepath.Join(artifactsDir, filepath.Base(f))

					if filepath.Base(f) == "PKGBUILD" {
						if err := copyFile(f, dest); err != nil {
							log.Printf("Warning: could not copy artifact %s: %v", f, err)
						} else {
							log.Printf("  Copied: %s", dest)
						}
					} else {
						if err := os.Rename(f, dest); err != nil {
							log.Printf("Warning: could not move artifact %s: %v", f, err)
						} else {
							log.Printf("  Collected: %s", dest)
							if strings.Contains(pattern, ".pkg.tar.") {
								foundPackages = true
							}
						}
					}
				}
			}

			if !foundPackages {
				log.Fatalf("Error: No package files (*.pkg.tar.*) were found to collect.")
			}
			log.Println("Artifacts collected successfully.")
		},
	}
	artifactsCmd.Flags().StringVarP(&artifactsDir, "output-dir", "o", "artifacts", "The directory to place artifacts in")

	// --- 'version' command ---
	var versionFile string
	var versionCmd = &cobra.Command{
		Use:   "version",
		Short: "Generates a .env file with version information for GitLab CI.",
		Run: func(cmd *cobra.Command, args []string) {
			log.Printf("Generating version info file at %s\n", versionFile)
			info, err := parsePKGBUILD("PKGBUILD")
			if err != nil {
				log.Fatalf("Error: %v", err)
			}

			ciCommitTag := os.Getenv("CI_COMMIT_TAG")
			if ciCommitTag == "" {
				ciCommitTag = info.PkgVer
			}
			ciJobID := os.Getenv("CI_JOB_ID")
			if ciJobID == "" {
				ciJobID = "local"
			}

			content := fmt.Sprintf(
				"VERSION=%s\nPKG_RELEASE=%s\nFULL_VERSION=%s-%s\nPACKAGE_NAME=%s\nTAG_VERSION=%s\nBUILD_JOB_ID=%s\nBUILD_DATE=%s\nARCH=\"%s\"\n",
				info.PkgVer,
				info.PkgRel,
				info.PkgVer, info.PkgRel,
				info.PkgName,
				ciCommitTag,
				ciJobID,
				time.Now().UTC().Format(time.RFC3339),
				strings.Join(info.Arch, " "),
			)

			if err := os.WriteFile(versionFile, []byte(content), 0644); err != nil {
				log.Fatalf("Failed to write version file: %v", err)
			}
			log.Println("Version info generated successfully:")
			fmt.Println(content)
		},
	}
	versionCmd.Flags().StringVarP(&versionFile, "output-file", "o", "version.env", "The .env file to generate")

	rootCmd.AddCommand(depsCmd, buildCmd, artifactsCmd, versionCmd)
	if err := rootCmd.Execute(); err != nil {
		os.Exit(1)
	}
}
