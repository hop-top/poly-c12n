package main

import (
	"context"
	"fmt"
	"path/filepath"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/cli"
	kitlog "hop.top/kit/go/console/log"
	"hop.top/kit/go/console/output"
	"hop.top/kit/go/core/config"
	"hop.top/kit/go/core/xdg"
)

// contextKey scopes cobra context values to avoid collisions.
type contextKey string

const (
	pipelineKey    contextKey = "pipeline"
	pipelineErrKey contextKey = "pipelineErr"
	configKey      contextKey = "config"
	configOptsKey  contextKey = "configOpts"
)

// PipelineFromContext retrieves the Pipeline stored in the cobra command context.
func PipelineFromContext(cmd *cobra.Command) *c12n.Pipeline {
	v, _ := cmd.Context().Value(pipelineKey).(*c12n.Pipeline)
	return v
}

// RequirePipeline returns the Pipeline for commands that cannot run
// without one, or an error explaining why it is unavailable.
//
// Pipeline construction is deliberately non-fatal in PersistentPreRunE
// so read-only commands still work in a stub build; this is the
// accessor that turns that soft failure back into a hard one, for the
// commands that genuinely need an engine. It surfaces the underlying
// cause (e.g. "native engine disabled") rather than a bare
// "pipeline not available", which told the user nothing actionable.
func RequirePipeline(cmd *cobra.Command) (*c12n.Pipeline, error) {
	if p := PipelineFromContext(cmd); p != nil {
		return p, nil
	}
	if err, _ := cmd.Context().Value(pipelineErrKey).(error); err != nil {
		return nil, fmt.Errorf("pipeline not available: %w", err)
	}
	return nil, fmt.Errorf("pipeline not available")
}

// ConfigFromContext retrieves the Config stored in the cobra command context.
func ConfigFromContext(cmd *cobra.Command) *c12n.Config {
	v, _ := cmd.Context().Value(configKey).(*c12n.Config)
	return v
}

// ConfigOptsFromContext retrieves the config.Options stored in the cobra
// command context.
func ConfigOptsFromContext(cmd *cobra.Command) config.Options {
	v, _ := cmd.Context().Value(configOptsKey).(config.Options)
	return v
}

// rootConfigOptions returns the layered config slots c12n loads from:
// system (/etc/c12n/config.yaml), user ($XDG_CONFIG_HOME/c12n/config.yaml),
// and project (./.c12n.yaml), plus the C12N_<KEY> env layer. Extracted from
// newRoot so tests can assert the layering without executing a command.
//
// A failure to resolve the XDG directory leaves UserConfigPath empty rather
// than aborting: the system and project layers still load, and kit's config
// loader silently skips empty conventional slots.
func rootConfigOptions() config.Options {
	opts := config.Options{
		SystemConfigPath:  filepath.Join("/etc", "c12n", "config.yaml"),
		ProjectConfigPath: ".c12n.yaml",
		EnvPrefix:         "C12N",
	}
	if cfgDir, err := xdg.ConfigDir("c12n"); err == nil {
		opts.UserConfigPath = filepath.Join(cfgDir, "config.yaml")
	}
	return opts
}

// newRoot builds the fully wired root command exactly as the binary does.
// Tests MUST use this constructor (not a hand-rolled cobra.Command) so the
// kit contract — global flag registration, PersistentPreRunE, hints — is
// exercised the same way it is at runtime.
func newRoot() (*cli.Root, error) {
	// WithStatus mounts kit's reserved `c12n status` subcommand. Kit's
	// validator requires the name to exist on the root; kit's own
	// implementation already carries the annotations and Long the
	// contract demands, so we mount it rather than hand-rolling one and
	// re-deriving the conformance surface.
	root := cli.New(cli.Config{
		Name:    "c12n",
		Version: version,
		Short:   "LLM request classification engine",
	}, cli.WithStatus(cli.StatusConfig{
		ExtraEnvKeys: []string{"C12N_*", "CGO_ENABLED"},
	}))

	// c12n-specific status sections. Priority 1000+ keeps them after
	// kit's own sections, per kit's band convention. Each provider must
	// work in BOTH build modes: in a stub build the engine section says
	// so plainly instead of erroring.
	registerStatusProviders(root)

	log := kitlog.New(root.Viper)

	opts := rootConfigOptions()
	opts.Viper = root.Viper

	// Hint registrations.
	var upgraded, updateAvail bool
	output.RegisterUpgradeHints(root.Hints, "c12n", &upgraded)
	output.RegisterVersionHints(root.Hints, "c12n", &updateAvail)

	root.Cmd.PersistentPreRunE = func(cmd *cobra.Command, _ []string) error {
		// Layer kit's -c/--config tokens on top of the discovered
		// files: bare paths append to ExtraConfigPaths, key=value
		// tokens become Overrides that win over every file layer.
		extraPaths, overrides, err := root.ConfigArgs()
		if err != nil {
			return err
		}
		opts := opts
		opts.ExtraConfigPaths = extraPaths
		opts.Overrides = overrides

		cfg, err := c12n.LoadConfig(opts)
		if err != nil {
			return fmt.Errorf("load config: %w", err)
		}
		log.Debug("config loaded",
			"signals", len(cfg.EnabledSignals()),
			"concurrency", cfg.MaxConcurrency)

		// Pipeline construction is best-effort, NOT a hard precondition.
		//
		// This hook runs for every command, but only classify and bench
		// actually evaluate anything. Failing the whole invocation here
		// meant that in a stub build (no -tags c12n_native, where
		// NewPipeline always errors) every single subcommand exited 1 —
		// including doctor, whose entire job is to diagnose a missing
		// native engine, and status, which must report the engine as
		// unavailable rather than die with it.
		//
		// The error is kept and surfaced by the commands that need a
		// pipeline; PipelineFromContext already returns nil for its
		// consumers, and they check for it.
		pipeline, perr := c12n.NewPipeline(cfg.ToPipelineConfig())
		if perr != nil {
			log.Debug("pipeline unavailable", "err", perr)
		}

		// Store config, opts, and pipeline in context for subcommands.
		newCtx := context.WithValue(cmd.Context(), configKey, cfg)
		newCtx = context.WithValue(newCtx, configOptsKey, opts)
		newCtx = context.WithValue(newCtx, pipelineKey, pipeline)
		newCtx = context.WithValue(newCtx, pipelineErrKey, perr)
		cmd.SetContext(newCtx)

		return nil
	}

	root.Cmd.AddCommand(benchCmd())
	root.Cmd.AddCommand(classifyCmd())
	root.Cmd.AddCommand(configCmd())
	root.Cmd.AddCommand(initCmd())
	root.Cmd.AddCommand(signalsCmd())
	root.Cmd.AddCommand(upgradeCmd())
	root.Cmd.AddCommand(doctorCmd())
	root.Cmd.AddCommand(tipCmd())
	root.Cmd.AddCommand(toolspecCmd())

	registerCompletions(root.Cmd)

	return root, nil
}

func run(ctx context.Context) error {
	root, err := newRoot()
	if err != nil {
		return err
	}
	return root.Execute(ctx)
}
