package main

import (
	"os"
	"path/filepath"
	"testing"

	c12n "hop.top/c12n"
	"hop.top/kit/go/core/config"
	"hop.top/kit/go/core/uxp"
)

// configCheck mimics the Check 1 closure from doctor.go.
// Extracted for testability.
func configCheck(opts config.Options) uxp.Check {
	if _, err := c12n.LoadConfig(opts); err != nil {
		projectExists := opts.ProjectConfigPath != "" &&
			fileExists(opts.ProjectConfigPath)
		userExists := opts.UserConfigPath != "" &&
			fileExists(opts.UserConfigPath)

		switch {
		case projectExists || userExists:
			return uxp.Check{
				Name:    "config-file",
				Status:  uxp.StatusFail,
				Message: "config file failed to parse",
				Detail:  err.Error(),
			}
		default:
			return uxp.Check{
				Name:    "config-file",
				Status:  uxp.StatusWarn,
				Message: "config file not found",
				Detail:  formatConfigPaths(opts),
			}
		}
	}

	return uxp.Check{
		Name:    "config-file",
		Status:  uxp.StatusOK,
		Message: "config file valid",
		Detail:  formatConfigPaths(opts),
	}
}

// TestDoctorConfigCheck_UserConfigOnly verifies that when the project
// config is absent but a valid user config exists, the check reports OK.
// LoadConfig succeeds via the user config layer, so there is no warning.
//
// Regression for PR #4 review comment #5 (T-0071).
func TestDoctorConfigCheck_UserConfigOnly(t *testing.T) {
	tmpDir := t.TempDir()
	userCfg := filepath.Join(tmpDir, "config.yaml")
	if err := os.WriteFile(userCfg, []byte("keyword_enabled: true\n"), 0o644); err != nil {
		t.Fatalf("write user config: %v", err)
	}

	projectCfg := filepath.Join(tmpDir, "missing", ".c12n.yaml")

	opts := config.Options{
		UserConfigPath:    userCfg,
		ProjectConfigPath: projectCfg,
	}

	result := configCheck(opts)
	if result.Status != uxp.StatusOK {
		t.Errorf(
			"expected OK when user config valid (project absent), "+
				"got status=%v message=%q detail=%q",
			result.Status, result.Message, result.Detail,
		)
	}
}

// TestDoctorConfigCheck_BothMissing_LoadDefaults verifies that when
// neither config file exists, LoadConfig still succeeds (using defaults),
// so the check reports OK with defaults. The layered loader silently
// skips missing files and applies defaults.
func TestDoctorConfigCheck_BothMissing_LoadDefaults(t *testing.T) {
	tmpDir := t.TempDir()
	opts := config.Options{
		UserConfigPath:    filepath.Join(tmpDir, "missing-user.yaml"),
		ProjectConfigPath: filepath.Join(tmpDir, "missing-project.yaml"),
	}

	result := configCheck(opts)
	// LoadConfig succeeds with defaults when no files exist.
	if result.Status != uxp.StatusOK {
		t.Errorf("expected OK (defaults applied), got %v", result.Status)
	}
	// Detail should indicate no files were found.
	if result.Detail == "" {
		t.Error("expected non-empty detail showing path status")
	}
}

// TestDoctorConfigCheck_ProjectExists verifies OK when project config
// exists on disk and parses correctly.
func TestDoctorConfigCheck_ProjectExists(t *testing.T) {
	tmpDir := t.TempDir()
	projectCfg := filepath.Join(tmpDir, ".c12n.yaml")
	if err := os.WriteFile(projectCfg, []byte("keyword_enabled: false\n"), 0o644); err != nil {
		t.Fatalf("write project config: %v", err)
	}

	opts := config.Options{
		ProjectConfigPath: projectCfg,
		UserConfigPath:    filepath.Join(tmpDir, "missing-user.yaml"),
	}

	result := configCheck(opts)
	if result.Status != uxp.StatusOK {
		t.Errorf("expected OK when project config exists, got %v", result.Status)
	}
}

// TestDoctorConfigCheck_InvalidYAML verifies FAIL when a config file
// exists but contains invalid YAML.
func TestDoctorConfigCheck_InvalidYAML(t *testing.T) {
	tmpDir := t.TempDir()
	userCfg := filepath.Join(tmpDir, "config.yaml")
	if err := os.WriteFile(userCfg, []byte("{{invalid yaml\n"), 0o644); err != nil {
		t.Fatalf("write user config: %v", err)
	}

	opts := config.Options{
		UserConfigPath:    userCfg,
		ProjectConfigPath: "",
	}

	result := configCheck(opts)
	if result.Status != uxp.StatusFail {
		t.Errorf("expected FAIL for invalid YAML, got %v", result.Status)
	}
	if result.Message != "config file failed to parse" {
		t.Errorf("expected parse failure message, got %q", result.Message)
	}
}

// TestDoctorConfigCheck_NoPathsConfigured verifies that LoadConfig
// succeeds with defaults even when no paths are set.
func TestDoctorConfigCheck_NoPathsConfigured(t *testing.T) {
	opts := config.Options{}

	result := configCheck(opts)
	// No paths configured means LoadConfig uses defaults only — still OK.
	if result.Status != uxp.StatusOK {
		t.Errorf("expected OK (defaults), got %v", result.Status)
	}
}

// TestFormatConfigPaths verifies the path formatting helper.
func TestFormatConfigPaths(t *testing.T) {
	t.Run("both found", func(t *testing.T) {
		tmpDir := t.TempDir()
		p1 := filepath.Join(tmpDir, "a.yaml")
		p2 := filepath.Join(tmpDir, "b.yaml")
		os.WriteFile(p1, []byte(""), 0o644)
		os.WriteFile(p2, []byte(""), 0o644)

		opts := config.Options{
			ProjectConfigPath: p1,
			UserConfigPath:    p2,
		}
		detail := formatConfigPaths(opts)
		if detail == "" {
			t.Error("expected non-empty detail")
		}
	})

	t.Run("neither found", func(t *testing.T) {
		opts := config.Options{
			ProjectConfigPath: "/no/such/project.yaml",
			UserConfigPath:    "/no/such/user.yaml",
		}
		detail := formatConfigPaths(opts)
		if detail == "" {
			t.Error("expected non-empty detail")
		}
	})
}
