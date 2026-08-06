package main

import (
	"context"
	"os"
	"sort"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/cli"
)

// Priority band for c12n's own status sections. Kit reserves 100-600
// for its shipped providers and asks adopters to start at 1000 so the
// kit-side ordering stays stable as sections are added.
const (
	priorityEngine  = 1000
	priorityConfig  = 1010
	prioritySignals = 1020
)

// defaultStatusFormat makes bare `c12n status` render as yaml.
//
// Kit's status renders a StatusOutput whose sections carry nested,
// heterogeneous payloads. The table renderer cannot flatten that shape
// and emits NOTHING for it, so with c12n's root default of
// --format=table, `c12n status` exited 0 having printed zero bytes.
//
// Overriding the flag's default value (not its value) keeps
// `--format json|table|...` working exactly as the user typed it; it
// only changes what happens when the flag is not passed at all. The
// override is scoped to the status command's own flag set, so every
// other subcommand keeps the table default.
func defaultStatusFormat(root *cli.Root) {
	if root == nil || root.Cmd == nil {
		return
	}
	for _, c := range root.Cmd.Commands() {
		if c.Name() != "status" {
			continue
		}
		// Shadow the inherited persistent flag with a local one that
		// differs only in default. Cobra prefers the local flag during
		// parse, so an explicit --format still wins.
		c.Flags().String("format", "yaml",
			"Output format (json|yaml|table)")
		return
	}
}

// engineStatus is the payload of the "engine" status section. Reports
// how this binary was built and whether the native classification
// engine is actually usable, which is the single most common reason
// classify returns nothing useful.
type engineStatus struct {
	// BuildMode is "c12n_native" or "stub".
	BuildMode string `json:"build_mode" yaml:"build_mode"`
	// NativeEngine reports whether the Rust cdylib is linked in.
	NativeEngine bool `json:"native_engine" yaml:"native_engine"`
	// PipelineLoadable reports whether a pipeline could actually be
	// constructed. False in a stub build, and also false in a native
	// build whose config the engine rejects.
	PipelineLoadable bool `json:"pipeline_loadable" yaml:"pipeline_loadable"`
	// Detail explains a false PipelineLoadable in prose.
	Detail string `json:"detail,omitempty" yaml:"detail,omitempty"`
}

// configStatus is the payload of the "config-paths" status section:
// the layered slots c12n reads and which of them exist on disk.
type configStatus struct {
	System  string `json:"system" yaml:"system"`
	User    string `json:"user" yaml:"user"`
	Project string `json:"project" yaml:"project"`
	// Found lists the slots that exist on disk, in precedence order.
	Found []string `json:"found" yaml:"found"`
	// EnvPrefix is the environment-variable layer's prefix.
	EnvPrefix string `json:"env_prefix" yaml:"env_prefix"`
}

// signalsStatus is the payload of the "signals" status section: how
// many signal types exist versus how many config actually enables.
type signalsStatus struct {
	Total   int      `json:"total" yaml:"total"`
	Enabled int      `json:"enabled" yaml:"enabled"`
	Names   []string `json:"enabled_names,omitempty" yaml:"enabled_names,omitempty"`
}

// registerStatusProviders attaches c12n's status sections to root.
//
// Providers deliberately re-resolve config themselves rather than
// reading it off the cobra context: `status` is reachable even when
// PersistentPreRunE could not build a pipeline (which is always the
// case in a stub build), and a status command that cannot run when the
// engine is broken is useless exactly when it is needed most.
func registerStatusProviders(root *cli.Root) {
	if root == nil {
		return
	}
	opts := rootConfigOptions()
	opts.Viper = root.Viper

	defaultStatusFormat(root)

	root.RegisterStatusProvider("engine", func(_ context.Context) (cli.StatusSection, error) {
		sec := cli.StatusSection{
			Title:    "engine",
			Priority: priorityEngine,
			Status:   cli.StatusOK,
		}
		st := engineStatus{
			BuildMode:    "stub",
			NativeEngine: nativeBuild,
		}
		if nativeBuild {
			st.BuildMode = "c12n_native"
		}

		// Probe the engine for real rather than inferring from the
		// build tag: a native build can still fail to construct a
		// pipeline (bad config, unloadable model), and reporting that
		// is the point of the section.
		cfg, err := c12n.LoadConfig(opts)
		switch {
		case err != nil:
			st.Detail = "config did not load: " + err.Error()
		default:
			pipeline, perr := c12n.NewPipeline(cfg.ToPipelineConfig())
			switch {
			case perr != nil:
				st.Detail = perr.Error()
			default:
				st.PipelineLoadable = true
				pipeline.Close()
			}
		}

		if !st.PipelineLoadable && !nativeBuild {
			// Expected state for a stub build, not a fault. Report it
			// plainly so the section does not read as an error.
			sec.Status = cli.StatusUnavailable
			st.Detail = "stub build: native signals unavailable " +
				"(rebuild with -tags c12n_native)"
		} else if !st.PipelineLoadable {
			sec.Status = cli.StatusError
			sec.ErrorMessage = st.Detail
		}

		sec.Data = st
		return sec, nil
	})

	root.RegisterStatusProvider("config-paths", func(_ context.Context) (cli.StatusSection, error) {
		sec := cli.StatusSection{
			Title:    "config-paths",
			Priority: priorityConfig,
			Status:   cli.StatusOK,
		}
		st := configStatus{
			System:    opts.SystemConfigPath,
			User:      opts.UserConfigPath,
			Project:   opts.ProjectConfigPath,
			EnvPrefix: opts.EnvPrefix,
		}
		for _, slot := range []struct{ label, path string }{
			{"system", opts.SystemConfigPath},
			{"user", opts.UserConfigPath},
			{"project", opts.ProjectConfigPath},
		} {
			if slot.path == "" {
				continue
			}
			if _, err := os.Stat(slot.path); err == nil {
				st.Found = append(st.Found, slot.label)
			}
		}
		if len(st.Found) == 0 {
			// No file anywhere: c12n runs on built-in defaults. Empty,
			// not an error.
			sec.Status = cli.StatusEmpty
		}
		sec.Data = st
		return sec, nil
	})

	root.RegisterStatusProvider("signals", func(_ context.Context) (cli.StatusSection, error) {
		sec := cli.StatusSection{
			Title:    "signals",
			Priority: prioritySignals,
			Status:   cli.StatusOK,
		}
		st := signalsStatus{Total: len(c12n.AllSignalTypes())}

		cfg, err := c12n.LoadConfig(opts)
		if err != nil {
			sec.Status = cli.StatusUnavailable
			sec.Data = st
			return sec, nil
		}
		for _, s := range cfg.EnabledSignals() {
			st.Names = append(st.Names, string(s))
		}
		sort.Strings(st.Names)
		st.Enabled = len(st.Names)
		if st.Enabled == 0 {
			sec.Status = cli.StatusEmpty
		}
		sec.Data = st
		return sec, nil
	})
}
