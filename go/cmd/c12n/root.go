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
	pipelineKey   contextKey = "pipeline"
	configKey     contextKey = "config"
	configOptsKey contextKey = "configOpts"
)

// PipelineFromContext retrieves the Pipeline stored in the cobra command context.
func PipelineFromContext(cmd *cobra.Command) *c12n.Pipeline {
	v, _ := cmd.Context().Value(pipelineKey).(*c12n.Pipeline)
	return v
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

func run(ctx context.Context) error {
	root := cli.New(cli.Config{
		Name:    "c12n",
		Version: version,
		Short:   "LLM request classification engine",
	})

	log := kitlog.New(root.Viper)

	// Resolve XDG config directory for layered config loading.
	cfgDir, err := xdg.ConfigDir("c12n")
	if err != nil {
		return fmt.Errorf("resolve config dir: %w", err)
	}

	opts := config.Options{
		UserConfigPath:    filepath.Join(cfgDir, "config.yaml"),
		ProjectConfigPath: ".c12n.yaml",
	}

	// Allow explicit --config flag to override all file paths.
	var cfgFlag string
	root.Cmd.PersistentFlags().StringVar(&cfgFlag, "config", "",
		"Path to config file (overrides default locations)")

	// Hint registrations.
	var upgraded, updateAvail bool
	output.RegisterUpgradeHints(root.Hints, "c12n", &upgraded)
	output.RegisterVersionHints(root.Hints, "c12n", &updateAvail)

	root.Cmd.PersistentPreRunE = func(cmd *cobra.Command, _ []string) error {
		// Override config paths when --config is provided.
		if cfgFlag != "" {
			opts = config.Options{ProjectConfigPath: cfgFlag}
		}

		cfg, err := c12n.LoadConfig(opts)
		if err != nil {
			return fmt.Errorf("load config: %w", err)
		}
		log.Debug("config loaded",
			"signals", len(cfg.EnabledSignals()),
			"concurrency", cfg.MaxConcurrency)

		pipeline, err := c12n.NewPipeline(cfg.ToPipelineConfig())
		if err != nil {
			return fmt.Errorf("create pipeline: %w", err)
		}

		// Store config, opts, and pipeline in context for subcommands.
		newCtx := context.WithValue(cmd.Context(), configKey, cfg)
		newCtx = context.WithValue(newCtx, configOptsKey, opts)
		newCtx = context.WithValue(newCtx, pipelineKey, pipeline)
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

	return root.Execute(ctx)
}
