package main

import (
	"fmt"
	"os"
	"os/exec"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/core/config"
	"hop.top/kit/go/core/uxp"
)

func doctorCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "doctor",
		Short: "Run diagnostic checks on the c12n environment",
		RunE: func(cmd *cobra.Command, _ []string) error {
			cfg := ConfigFromContext(cmd)
			opts := ConfigOptsFromContext(cmd)
			w := cmd.OutOrStdout()

			doc := uxp.NewDoctor()

			// Check 1: config file exists and parses.
			doc.Add(func() uxp.Check {
				// Try the full layered load first — if it succeeds,
				// at least one config layer is present and valid.
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
			})

			// Check 2: model paths resolve (if relevant signals enabled).
			doc.Add(func() uxp.Check {
				if cfg == nil {
					return uxp.Check{
						Name:    "model-paths",
						Status:  uxp.StatusSkip,
						Message: "no config loaded",
					}
				}

				type mp struct {
					name string
					on   bool
					path *string
				}
				paths := []mp{
					{"embedding", cfg.EmbeddingEnabled, cfg.EmbeddingModelPath},
					{"domain", cfg.DomainEnabled, cfg.DomainModelPath},
					{"jailbreak", cfg.SafetyJailbreakEnabled, cfg.SafetyJailbreakModelPath},
					{"complexity", cfg.ComplexityEnabled, cfg.ComplexityModelPath},
				}

				var missing []string
				checked := 0
				for _, p := range paths {
					if !p.on {
						continue
					}
					if p.path == nil || *p.path == "" {
						missing = append(missing, p.name+": not set")
						checked++
						continue
					}
					if _, err := os.Stat(*p.path); err != nil {
						missing = append(missing, p.name+": "+*p.path)
					}
					checked++
				}

				if checked == 0 {
					return uxp.Check{
						Name:    "model-paths",
						Status:  uxp.StatusSkip,
						Message: "no model-based signals enabled",
					}
				}
				if len(missing) > 0 {
					return uxp.Check{
						Name:    "model-paths",
						Status:  uxp.StatusFail,
						Message: "model path(s) unresolved",
						Detail:  fmt.Sprintf("%v", missing),
					}
				}
				return uxp.Check{
					Name:    "model-paths",
					Status:  uxp.StatusOK,
					Message: "all model paths resolve",
				}
			})

			// Check 3: CGO available.
			doc.Add(func() uxp.Check {
				out, err := exec.Command("go", "env", "CGO_ENABLED").Output()
				if err != nil {
					return uxp.Check{
						Name:    "cgo",
						Status:  uxp.StatusWarn,
						Message: "could not determine CGO status",
						Detail:  err.Error(),
					}
				}
				val := string(out)
				if len(val) > 0 && val[0] == '1' {
					return uxp.Check{
						Name:    "cgo",
						Status:  uxp.StatusOK,
						Message: "CGO_ENABLED=1",
					}
				}
				return uxp.Check{
					Name:    "cgo",
					Status:  uxp.StatusWarn,
					Message: "CGO_ENABLED=0 (native signals unavailable)",
				}
			})

			results := doc.Run()

			icons := map[uxp.CheckStatus]string{
				uxp.StatusOK:   "PASS",
				uxp.StatusWarn: "WARN",
				uxp.StatusFail: "FAIL",
				uxp.StatusSkip: "SKIP",
			}

			for _, r := range results {
				icon := icons[r.Status]
				fmt.Fprintf(w, "[%s] %s: %s\n", icon, r.Name, r.Message)
				if r.Detail != "" {
					fmt.Fprintf(w, "       %s\n", r.Detail)
				}
			}

			return nil
		},
	}
}

// fileExists reports whether path points to an existing file.
func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

// formatConfigPaths returns a human-readable summary of configured paths.
func formatConfigPaths(opts config.Options) string {
	var parts []string
	if opts.ProjectConfigPath != "" {
		label := "project=" + opts.ProjectConfigPath
		if fileExists(opts.ProjectConfigPath) {
			label += " (found)"
		} else {
			label += " (not found)"
		}
		parts = append(parts, label)
	}
	if opts.UserConfigPath != "" {
		label := "user=" + opts.UserConfigPath
		if fileExists(opts.UserConfigPath) {
			label += " (found)"
		} else {
			label += " (not found)"
		}
		parts = append(parts, label)
	}
	if len(parts) == 0 {
		return "no config paths configured"
	}
	return fmt.Sprintf("%v", parts)
}
